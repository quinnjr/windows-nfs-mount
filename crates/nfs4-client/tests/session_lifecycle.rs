//! Integration test: full session lifecycle through a mock NFS transport.
//!
//! Validates the connect → compound → disconnect path by wiring up a
//! `MockNfsTransport` that decodes the tag from each incoming COMPOUND
//! request and returns the appropriate scripted response. This exercises
//! the full stack: CompoundBuilder → XDR encoding → RPC dispatch →
//! response decoding → SessionManager state changes.

use std::sync::{Arc, Mutex as StdMutex};

use bytes::{Bytes, BytesMut};
use onc_rpc::{AcceptStatus, AuthFlavor, RpcError, RpcResponse, RpcTransport};
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

use nfs4_types::bitmap::{Bitmap4, Fattr4};
use nfs4_types::ops::filehandle::*;
use nfs4_types::ops::session::*;
use nfs4_types::*;

use nfs4_client::{CompoundBuilder, Nfs4Client};

// ---------------------------------------------------------------------------
// Mock transport
// ---------------------------------------------------------------------------

/// Mock NFS transport that decodes the tag from each incoming COMPOUND
/// request, records it, and dispatches a pre-built response based on the
/// tag string.
///
/// This is more realistic than a simple response-queue mock because it
/// verifies the client is sending the tags we expect in the right order,
/// while still being deterministic and free of real network I/O.
struct MockNfsTransport {
    /// Tags received, in order.
    received_tags: StdMutex<Vec<String>>,
}

impl MockNfsTransport {
    fn new() -> Self {
        Self {
            received_tags: StdMutex::new(Vec::new()),
        }
    }

    fn received_tags(&self) -> Vec<String> {
        self.received_tags.lock().unwrap().clone()
    }

    /// Build a Compound4res for a given tag, encode it, and wrap it in
    /// an RpcResponse.
    fn make_rpc_response(compound_res: Compound4res) -> Result<RpcResponse, RpcError> {
        let mut buf = BytesMut::new();
        compound_res
            .encode(&mut buf)
            .map_err(|e| RpcError::Xdr(e))?;

        Ok(RpcResponse {
            xid: 0,
            verf: AuthFlavor::None,
            status: AcceptStatus::Success(Opaque(buf.to_vec())),
        })
    }

    /// Return the scripted response for a given tag.
    fn response_for_tag(tag: &str) -> Compound4res {
        match tag {
            "exchange_id" => Compound4res {
                status: NfsStat4::Ok,
                tag: tag.into(),
                resarray: vec![NfsResOp4::ExchangeId(ExchangeId4res {
                    status: NfsStat4::Ok,
                    resok: Some(ExchangeId4resok {
                        eir_clientid: ClientId4(0x1234),
                        eir_sequenceid: 1,
                        eir_flags: 0,
                        eir_state_protect: StateProtect4r::SpNone,
                        eir_server_owner: ServerOwner4 {
                            so_minor_id: 0,
                            so_major_id: Opaque(b"mock-server".to_vec()),
                        },
                        eir_server_scope: Opaque(b"mock".to_vec()),
                        eir_server_impl_id: vec![],
                    }),
                })],
            },

            "create_session" => Compound4res {
                status: NfsStat4::Ok,
                tag: tag.into(),
                resarray: vec![NfsResOp4::CreateSession(CreateSession4res {
                    status: NfsStat4::Ok,
                    resok: Some(CreateSession4resok {
                        csr_sessionid: SessionId4([0x42; 16]),
                        csr_sequence: 1,
                        csr_flags: 0,
                        csr_fore_chan_attrs: ChannelAttrs4 {
                            ca_headerpadsize: 0,
                            ca_maxrequestsize: 1_048_576,
                            ca_maxresponsesize: 1_048_576,
                            ca_maxresponsesize_cached: 65_536,
                            ca_maxoperations: 64,
                            ca_maxrequests: 8,
                            ca_rdma_ird: vec![],
                        },
                        csr_back_chan_attrs: ChannelAttrs4 {
                            ca_headerpadsize: 0,
                            ca_maxrequestsize: 4096,
                            ca_maxresponsesize: 4096,
                            ca_maxresponsesize_cached: 4096,
                            ca_maxoperations: 2,
                            ca_maxrequests: 2,
                            ca_rdma_ird: vec![],
                        },
                    }),
                })],
            },

            "reclaim_complete" => Compound4res {
                status: NfsStat4::Ok,
                tag: tag.into(),
                resarray: vec![
                    NfsResOp4::Sequence(Sequence4res {
                        status: NfsStat4::Ok,
                        resok: Some(Sequence4resok {
                            sr_sessionid: SessionId4([0x42; 16]),
                            sr_sequenceid: 1,
                            sr_slotid: 0,
                            sr_highest_slotid: 7,
                            sr_target_highest_slotid: 7,
                            sr_status_flags: 0,
                        }),
                    }),
                    NfsResOp4::ReclaimComplete(NfsStat4::Ok),
                ],
            },

            "getattr" => Compound4res {
                status: NfsStat4::Ok,
                tag: tag.into(),
                resarray: vec![
                    NfsResOp4::Sequence(Sequence4res {
                        status: NfsStat4::Ok,
                        resok: Some(Sequence4resok {
                            sr_sessionid: SessionId4([0x42; 16]),
                            sr_sequenceid: 2,
                            sr_slotid: 0,
                            sr_highest_slotid: 7,
                            sr_target_highest_slotid: 7,
                            sr_status_flags: 0,
                        }),
                    }),
                    NfsResOp4::PutRootFh(NfsStat4::Ok),
                    NfsResOp4::GetAttr(GetAttr4res {
                        status: NfsStat4::Ok,
                        resok: Some(GetAttr4resok {
                            obj_attributes: Fattr4 {
                                attrmask: Bitmap4::new(),
                                attr_vals: vec![],
                            },
                        }),
                    }),
                ],
            },

            "destroy_session" => Compound4res {
                status: NfsStat4::Ok,
                tag: tag.into(),
                resarray: vec![NfsResOp4::DestroySession(NfsStat4::Ok)],
            },

            other => panic!("MockNfsTransport: unhandled tag {:?}", other),
        }
    }
}

impl RpcTransport for MockNfsTransport {
    fn call(
        &self,
        _program: u32,
        _version: u32,
        _procedure: u32,
        args: &[u8],
        _cred: &AuthFlavor,
        _verf: &AuthFlavor,
    ) -> impl std::future::Future<Output = Result<RpcResponse, RpcError>> + Send {
        // Decode just the tag from the incoming Compound4args. The
        // encoding order is: tag (XDR string), minorversion (u32),
        // argarray (variable). We only need the tag to dispatch.
        let mut bytes: Bytes = args.to_vec().into();
        let tag = String::decode(&mut bytes).expect("failed to decode compound tag");

        self.received_tags.lock().unwrap().push(tag.clone());

        let response = Self::response_for_tag(&tag);
        let rpc_response = Self::make_rpc_response(response).expect("failed to encode response");

        async move { Ok(rpc_response) }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full session lifecycle: connect → user compound → disconnect.
///
/// Verifies that connect sends the three expected compounds
/// (EXCHANGE_ID, CREATE_SESSION, RECLAIM_COMPLETE with SEQUENCE),
/// that the session is populated with the right IDs, that a user
/// compound round-trips correctly, and that disconnect tears down
/// the session.
#[tokio::test]
async fn full_session_lifecycle() {
    let transport = Arc::new(MockNfsTransport::new());
    let mut client = Nfs4Client::new(transport.clone(), AuthFlavor::None);

    // -- Connect: EXCHANGE_ID → CREATE_SESSION → SEQUENCE+RECLAIM_COMPLETE
    let owner = ClientOwner4 {
        co_verifier: Verifier4([0; 8]),
        co_ownerid: Opaque(b"test-client".to_vec()),
    };
    client.connect(owner).await.unwrap();

    // Session should now be established with the mock's values.
    let session = client.session().unwrap();
    assert_eq!(session.session_id(), &SessionId4([0x42; 16]));
    assert_eq!(session.client_id(), ClientId4(0x1234));

    // Verify connect sent the right compounds in order.
    let tags = transport.received_tags();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "exchange_id");
    assert_eq!(tags[1], "create_session");
    assert_eq!(tags[2], "reclaim_complete");

    // -- Send a user compound: SEQUENCE + PUTROOTFH + GETATTR
    let session = client.session().unwrap();
    let (slot, seq, highest) = session.try_alloc_slot().await.unwrap();

    let args = CompoundBuilder::new()
        .tag("getattr")
        .sequence(session.session_id().clone(), seq, slot, highest)
        .putrootfh()
        .getattr(Bitmap4::new())
        .build();

    let res = client.compound(args).await.unwrap();
    session.release_slot(slot).await;

    assert!(res.status.is_ok());
    assert_eq!(res.resarray.len(), 3);
    assert!(matches!(res.resarray[0], NfsResOp4::Sequence(_)));
    assert!(matches!(res.resarray[1], NfsResOp4::PutRootFh(NfsStat4::Ok)));
    assert!(matches!(res.resarray[2], NfsResOp4::GetAttr(_)));

    // -- Disconnect: DESTROY_SESSION
    client.disconnect().await.unwrap();
    assert!(client.session().is_none());

    // Verify the full sequence of tags.
    let tags = transport.received_tags();
    assert_eq!(tags.len(), 5);
    assert_eq!(tags[3], "getattr");
    assert_eq!(tags[4], "destroy_session");
}

/// Verify that connect populates the session manager with the
/// correct client ID and session ID from the mock server.
#[tokio::test]
async fn connect_populates_session_manager() {
    let transport = Arc::new(MockNfsTransport::new());
    let mut client = Nfs4Client::new(transport, AuthFlavor::None);

    assert!(client.session().is_none());

    let owner = ClientOwner4 {
        co_verifier: Verifier4([0; 8]),
        co_ownerid: Opaque(b"test".to_vec()),
    };
    client.connect(owner).await.unwrap();

    let session = client.session().unwrap();
    assert_eq!(session.client_id(), ClientId4(0x1234));
    assert_eq!(session.session_id(), &SessionId4([0x42; 16]));

    // max_slots should be 8 (ca_maxrequests from the mock), meaning
    // slots 0..7 are available and we can allocate 8 before exhaustion.
    for i in 0..8 {
        let alloc = session.try_alloc_slot().await;
        assert!(alloc.is_some(), "slot {} should be available", i);
    }
    // The 9th should fail.
    assert!(session.try_alloc_slot().await.is_none());
}

/// Verify that disconnect without a session returns NoSession.
#[tokio::test]
async fn disconnect_without_session_errors() {
    let transport = Arc::new(MockNfsTransport::new());
    let mut client = Nfs4Client::new(transport, AuthFlavor::None);

    let err = client.disconnect().await.unwrap_err();
    assert!(
        matches!(err, nfs4_client::NfsError::NoSession),
        "expected NoSession, got {:?}",
        err
    );
}
