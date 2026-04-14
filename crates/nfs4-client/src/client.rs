use std::sync::Arc;

use bytes::BytesMut;
use onc_rpc::{AcceptStatus, AuthFlavor, RpcTransport};
use xdr_codec::{XdrDecode, XdrEncode};

use nfs4_types::ops::session::*;
use nfs4_types::*;

use crate::compound::CompoundBuilder;
use crate::error::NfsError;
use crate::session::SessionManager;

pub struct Nfs4Client<T: RpcTransport> {
    transport: Arc<T>,
    auth: AuthFlavor,
    session: Option<SessionManager>,
}

impl<T: RpcTransport> Nfs4Client<T> {
    pub fn new(transport: Arc<T>, auth: AuthFlavor) -> Self {
        Self {
            transport,
            auth,
            session: None,
        }
    }

    /// Get a reference to the session manager (if connected).
    pub fn session(&self) -> Option<&SessionManager> {
        self.session.as_ref()
    }

    /// Send a COMPOUND request and decode the response.
    pub async fn compound(&self, args: Compound4args) -> Result<Compound4res, NfsError> {
        let mut buf = BytesMut::new();
        args.encode(&mut buf)?;

        let response = self
            .transport
            .call(
                NFS4_PROGRAM,
                NFS4_VERSION,
                NFSPROC4_COMPOUND,
                &buf,
                &self.auth,
                &AuthFlavor::None,
            )
            .await?;

        match response.status {
            AcceptStatus::Success(data) => {
                let mut bytes = data.0.into();
                Ok(Compound4res::decode(&mut bytes)?)
            }
            other => Err(NfsError::Rpc(onc_rpc::RpcError::AcceptError(format!(
                "{:?}",
                other
            )))),
        }
    }

    /// Establish a session: EXCHANGE_ID + CREATE_SESSION + RECLAIM_COMPLETE.
    pub async fn connect(&mut self, client_owner: ClientOwner4) -> Result<(), NfsError> {
        // Step 1: EXCHANGE_ID (no session needed)
        let exchange_args = CompoundBuilder::new()
            .tag("exchange_id")
            .exchange_id(client_owner, 0)
            .build();

        let res = self.compound(exchange_args).await?;
        if !res.status.is_ok() {
            return Err(NfsError::Nfs(res.status));
        }

        let exchange_res = match &res.resarray[0] {
            NfsResOp4::ExchangeId(r) => r,
            other => {
                return Err(NfsError::UnexpectedResponse {
                    expected: "ExchangeId",
                    actual: format!("{:?}", other),
                })
            }
        };

        if !exchange_res.status.is_ok() {
            return Err(NfsError::Nfs(exchange_res.status));
        }

        let resok = exchange_res
            .resok
            .as_ref()
            .ok_or(NfsError::UnexpectedResponse {
                expected: "ExchangeId4resok",
                actual: "None".into(),
            })?;

        let client_id = resok.eir_clientid;
        let sequence_id = resok.eir_sequenceid;

        // Step 2: CREATE_SESSION
        let default_chan = ChannelAttrs4 {
            ca_headerpadsize: 0,
            ca_maxrequestsize: 1_048_576,
            ca_maxresponsesize: 1_048_576,
            ca_maxresponsesize_cached: 65_536,
            ca_maxoperations: 64,
            ca_maxrequests: 32,
            ca_rdma_ird: vec![],
        };

        let create_args = CompoundBuilder::new()
            .tag("create_session")
            .create_session(
                client_id,
                sequence_id,
                0,
                default_chan.clone(),
                default_chan,
            )
            .build();

        let res = self.compound(create_args).await?;
        if !res.status.is_ok() {
            return Err(NfsError::Nfs(res.status));
        }

        let create_res = match &res.resarray[0] {
            NfsResOp4::CreateSession(r) => r,
            other => {
                return Err(NfsError::UnexpectedResponse {
                    expected: "CreateSession",
                    actual: format!("{:?}", other),
                })
            }
        };

        if !create_res.status.is_ok() {
            return Err(NfsError::Nfs(create_res.status));
        }

        let csresok = create_res
            .resok
            .as_ref()
            .ok_or(NfsError::UnexpectedResponse {
                expected: "CreateSession4resok",
                actual: "None".into(),
            })?;

        let session_id = csresok.csr_sessionid.clone();
        let max_slots = csresok.csr_fore_chan_attrs.ca_maxrequests;

        self.session = Some(SessionManager::new(
            session_id.clone(),
            client_id,
            max_slots,
            90,
        ));

        // Step 3: SEQUENCE + RECLAIM_COMPLETE
        let session = self.session.as_ref().unwrap();
        let (slot, seq, highest) = session
            .try_alloc_slot()
            .await
            .ok_or(NfsError::NoSession)?;

        let reclaim_args = CompoundBuilder::new()
            .tag("reclaim_complete")
            .sequence(session_id, seq, slot, highest)
            .reclaim_complete(false)
            .build();

        let res = self.compound(reclaim_args).await?;
        session.release_slot(slot).await;

        if !res.status.is_ok() {
            return Err(NfsError::Nfs(res.status));
        }

        Ok(())
    }

    /// Destroy the active session.
    pub async fn disconnect(&mut self) -> Result<(), NfsError> {
        let session = self.session.as_ref().ok_or(NfsError::NoSession)?;
        let session_id = session.session_id().clone();

        let args = CompoundBuilder::new()
            .tag("destroy_session")
            .destroy_session(session_id)
            .build();

        let res = self.compound(args).await?;
        self.session = None;

        if !res.status.is_ok() {
            return Err(NfsError::Nfs(res.status));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use onc_rpc::{AuthFlavor, RpcResponse, RpcTransport};
    use std::sync::Mutex as StdMutex;
    use xdr_codec::Opaque;

    /// Mock transport that returns pre-scripted Compound4res responses.
    struct MockTransport {
        responses: StdMutex<Vec<Compound4res>>,
    }

    impl MockTransport {
        fn new(responses: Vec<Compound4res>) -> Self {
            Self {
                responses: StdMutex::new(responses),
            }
        }
    }

    impl RpcTransport for MockTransport {
        fn call(
            &self,
            _program: u32,
            _version: u32,
            _procedure: u32,
            _args: &[u8],
            _cred: &AuthFlavor,
            _verf: &AuthFlavor,
        ) -> impl std::future::Future<Output = Result<RpcResponse, onc_rpc::RpcError>> + Send
        {
            let res = self.responses.lock().unwrap().remove(0);
            let mut buf = BytesMut::new();
            res.encode(&mut buf).unwrap();
            async move {
                Ok(RpcResponse {
                    xid: 0,
                    verf: AuthFlavor::None,
                    status: AcceptStatus::Success(Opaque(buf.to_vec())),
                })
            }
        }
    }

    fn make_exchange_id_res() -> Compound4res {
        Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![NfsResOp4::ExchangeId(ExchangeId4res {
                status: NfsStat4::Ok,
                resok: Some(ExchangeId4resok {
                    eir_clientid: ClientId4(0x1234),
                    eir_sequenceid: 1,
                    eir_flags: 0,
                    eir_state_protect: StateProtect4r::SpNone,
                    eir_server_owner: ServerOwner4 {
                        so_minor_id: 0,
                        so_major_id: Opaque(b"server".to_vec()),
                    },
                    eir_server_scope: Opaque(b"scope".to_vec()),
                    eir_server_impl_id: vec![],
                }),
            })],
        }
    }

    fn make_create_session_res() -> Compound4res {
        Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![NfsResOp4::CreateSession(CreateSession4res {
                status: NfsStat4::Ok,
                resok: Some(CreateSession4resok {
                    csr_sessionid: SessionId4([0xBB; 16]),
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
                        ca_maxresponsesize_cached: 0,
                        ca_maxoperations: 2,
                        ca_maxrequests: 1,
                        ca_rdma_ird: vec![],
                    },
                }),
            })],
        }
    }

    fn make_reclaim_complete_res() -> Compound4res {
        Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0xBB; 16]),
                        sr_sequenceid: 1,
                        sr_slotid: 0,
                        sr_highest_slotid: 7,
                        sr_target_highest_slotid: 7,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::ReclaimComplete(NfsStat4::Ok),
            ],
        }
    }

    fn make_destroy_session_res() -> Compound4res {
        Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![NfsResOp4::DestroySession(NfsStat4::Ok)],
        }
    }

    #[tokio::test]
    async fn compound_sends_and_receives() {
        let expected = Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![NfsResOp4::PutRootFh(NfsStat4::Ok)],
        };
        let transport = Arc::new(MockTransport::new(vec![expected.clone()]));
        let client = Nfs4Client::new(transport, AuthFlavor::None);

        let args = CompoundBuilder::new().tag("test").putrootfh().build();
        let res = client.compound(args).await.unwrap();

        assert_eq!(res.status, NfsStat4::Ok);
        assert_eq!(res.resarray.len(), 1);
        assert!(matches!(res.resarray[0], NfsResOp4::PutRootFh(NfsStat4::Ok)));
    }

    #[tokio::test]
    async fn connect_lifecycle() {
        let transport = Arc::new(MockTransport::new(vec![
            make_exchange_id_res(),
            make_create_session_res(),
            make_reclaim_complete_res(),
        ]));

        let mut client = Nfs4Client::new(transport, AuthFlavor::None);
        assert!(client.session().is_none());

        let owner = ClientOwner4 {
            co_verifier: Verifier4([0; 8]),
            co_ownerid: Opaque(b"test-client".to_vec()),
        };

        client.connect(owner).await.unwrap();
        assert!(client.session().is_some());
        assert_eq!(client.session().unwrap().session_id(), &SessionId4([0xBB; 16]));
        assert_eq!(client.session().unwrap().client_id(), ClientId4(0x1234));
    }

    #[tokio::test]
    async fn disconnect_clears_session() {
        let transport = Arc::new(MockTransport::new(vec![
            make_exchange_id_res(),
            make_create_session_res(),
            make_reclaim_complete_res(),
            make_destroy_session_res(),
        ]));

        let mut client = Nfs4Client::new(transport, AuthFlavor::None);

        let owner = ClientOwner4 {
            co_verifier: Verifier4([0; 8]),
            co_ownerid: Opaque(b"test-client".to_vec()),
        };

        client.connect(owner).await.unwrap();
        assert!(client.session().is_some());

        client.disconnect().await.unwrap();
        assert!(client.session().is_none());
    }

    #[tokio::test]
    async fn disconnect_without_session_errors() {
        let transport = Arc::new(MockTransport::new(vec![]));
        let mut client = Nfs4Client::new(transport, AuthFlavor::None);

        let err = client.disconnect().await.unwrap_err();
        assert!(matches!(err, NfsError::NoSession));
    }
}
