use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use nfs4_client::{CompoundBuilder, Nfs4Client, NfsError};
use nfs4_types::*;
use onc_rpc::RpcTransport;

use crate::attr_cache::AttrCache;
use crate::config::MountConfig;
use crate::dir_cache::DirCache;
use crate::error::MountError;
use crate::handle::HandleTable;
use crate::path;
use crate::read_buf::ReadAheadBuf;
use crate::write_buf::WriteBuf;

pub struct MountHandle<T: RpcTransport> {
    client: Arc<Mutex<Nfs4Client<T>>>,
    config: MountConfig,
    attr_cache: Mutex<AttrCache>,
    dir_cache: Mutex<DirCache>,
    handles: Mutex<HandleTable>,
    read_bufs: Mutex<HashMap<u64, ReadAheadBuf>>,
    write_bufs: Mutex<HashMap<u64, WriteBuf>>,
}

impl<T: RpcTransport> MountHandle<T> {
    /// Create a new MountHandle wrapping an already-connected client.
    ///
    /// Cache TTLs and buffer sizes are derived from the supplied config.
    pub fn new(client: Nfs4Client<T>, config: MountConfig) -> Self {
        let attr_cache = AttrCache::new(
            config.acregmin,
            config.acregmax,
            config.acdirmin,
            config.acdirmax,
        );
        let dir_cache = DirCache::new(config.acdirmin);

        Self {
            client: Arc::new(Mutex::new(client)),
            config,
            attr_cache: Mutex::new(attr_cache),
            dir_cache: Mutex::new(dir_cache),
            handles: Mutex::new(HandleTable::new()),
            read_bufs: Mutex::new(HashMap::new()),
            write_bufs: Mutex::new(HashMap::new()),
        }
    }

    /// Return a reference to the mount configuration.
    pub fn config(&self) -> &MountConfig {
        &self.config
    }

    /// Translate a Windows-style path to an NFS filehandle.
    ///
    /// Builds a single COMPOUND containing SEQUENCE + PUTROOTFH +
    /// LOOKUP (one per path component) + GETFH, sends it, and extracts
    /// the resulting filehandle. An empty path (e.g. `\`) resolves to
    /// the root filehandle.
    pub async fn resolve_path(&self, windows_path: &str) -> Result<NfsFh4, MountError> {
        let components = path::to_nfs_components(windows_path);

        let client = self.client.lock().await;

        let session = client
            .session()
            .ok_or_else(|| MountError::Nfs(NfsError::NoSession))?;

        let (slot, seq, highest) = session
            .try_alloc_slot()
            .await
            .ok_or_else(|| MountError::Nfs(NfsError::NoSession))?;

        let session_id = session.session_id().clone();

        let mut builder = CompoundBuilder::new()
            .tag("resolve_path")
            .sequence(session_id, seq, slot, highest)
            .putrootfh();

        for component in &components {
            builder = builder.lookup(*component);
        }
        builder = builder.getfh();

        let args = builder.build();
        let res = client.compound(args).await;

        session.release_slot(slot).await;

        let res = res?;

        if !res.status.is_ok() {
            return Err(MountError::Nfs(NfsError::Nfs(res.status)));
        }

        // The GETFH result is the last element in the response array.
        // Layout: SEQUENCE + PUTROOTFH + N*LOOKUP + GETFH
        let getfh_res = res
            .resarray
            .last()
            .ok_or_else(|| {
                MountError::Nfs(NfsError::UnexpectedResponse {
                    expected: "GetFh",
                    actual: "empty resarray".into(),
                })
            })?;

        match getfh_res {
            NfsResOp4::GetFh(fh_res) => {
                if !fh_res.status.is_ok() {
                    return Err(MountError::Nfs(NfsError::Nfs(fh_res.status)));
                }
                fh_res
                    .object
                    .clone()
                    .ok_or_else(|| {
                        MountError::Nfs(NfsError::UnexpectedResponse {
                            expected: "GetFh4res with object",
                            actual: "None".into(),
                        })
                    })
            }
            other => Err(MountError::Nfs(NfsError::UnexpectedResponse {
                expected: "GetFh",
                actual: format!("{:?}", other),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use nfs4_types::ops::filehandle::GetFh4res;
    use nfs4_types::ops::session::*;
    use onc_rpc::{AcceptStatus, AuthFlavor, RpcResponse, RpcTransport};
    use std::sync::Mutex as StdMutex;
    use xdr_codec::{Opaque, XdrEncode};

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

    /// Helper: build the three responses needed by Nfs4Client::connect().
    fn connect_responses() -> Vec<Compound4res> {
        vec![
            // EXCHANGE_ID
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
            },
            // CREATE_SESSION
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
            },
            // SEQUENCE + RECLAIM_COMPLETE
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
            },
        ]
    }

    /// Create a connected client backed by a MockTransport.
    async fn make_connected_client(
        extra_responses: Vec<Compound4res>,
    ) -> Nfs4Client<MockTransport> {
        let mut responses = connect_responses();
        responses.extend(extra_responses);

        let transport = Arc::new(MockTransport::new(responses));
        let mut client = Nfs4Client::new(transport, AuthFlavor::None);

        let owner = ClientOwner4 {
            co_verifier: Verifier4([0; 8]),
            co_ownerid: Opaque(b"test-mount".to_vec()),
        };
        client.connect(owner).await.unwrap();
        client
    }

    #[test]
    fn mount_handle_creation() {
        let transport = Arc::new(MockTransport::new(vec![]));
        let client = Nfs4Client::new(transport, AuthFlavor::None);

        let config = MountConfig {
            rsize: 65536,
            wsize: 65536,
            ..MountConfig::default()
        };

        let handle = MountHandle::new(client, config.clone());

        assert_eq!(handle.config().rsize, 65536);
        assert_eq!(handle.config().wsize, 65536);
        assert_eq!(handle.config().uid, MountConfig::default().uid);
    }

    #[tokio::test]
    async fn resolve_root_path() {
        let root_fh = NfsFh4(vec![0xDE, 0xAD, 0xBE, 0xEF]);

        let resolve_res = Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0xBB; 16]),
                        sr_sequenceid: 2,
                        sr_slotid: 0,
                        sr_highest_slotid: 7,
                        sr_target_highest_slotid: 7,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::PutRootFh(NfsStat4::Ok),
                NfsResOp4::GetFh(GetFh4res {
                    status: NfsStat4::Ok,
                    object: Some(root_fh.clone()),
                }),
            ],
        };

        let client = make_connected_client(vec![resolve_res]).await;
        let handle = MountHandle::new(client, MountConfig::default());

        let fh = handle.resolve_path("\\").await.unwrap();
        assert_eq!(fh, root_fh);
    }

    #[tokio::test]
    async fn resolve_nested_path() {
        let expected_fh = NfsFh4(vec![0xCA, 0xFE]);

        let resolve_res = Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0xBB; 16]),
                        sr_sequenceid: 2,
                        sr_slotid: 0,
                        sr_highest_slotid: 7,
                        sr_target_highest_slotid: 7,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::PutRootFh(NfsStat4::Ok),
                NfsResOp4::Lookup(NfsStat4::Ok),
                NfsResOp4::Lookup(NfsStat4::Ok),
                NfsResOp4::GetFh(GetFh4res {
                    status: NfsStat4::Ok,
                    object: Some(expected_fh.clone()),
                }),
            ],
        };

        let client = make_connected_client(vec![resolve_res]).await;
        let handle = MountHandle::new(client, MountConfig::default());

        let fh = handle.resolve_path("\\folder\\file.txt").await.unwrap();
        assert_eq!(fh, expected_fh);
    }

    #[tokio::test]
    async fn resolve_path_not_found() {
        let resolve_res = Compound4res {
            status: NfsStat4::Noent,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0xBB; 16]),
                        sr_sequenceid: 2,
                        sr_slotid: 0,
                        sr_highest_slotid: 7,
                        sr_target_highest_slotid: 7,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::PutRootFh(NfsStat4::Ok),
                NfsResOp4::Lookup(NfsStat4::Noent),
            ],
        };

        let client = make_connected_client(vec![resolve_res]).await;
        let handle = MountHandle::new(client, MountConfig::default());

        let err = handle.resolve_path("\\nonexistent").await.unwrap_err();
        assert!(matches!(err, MountError::Nfs(NfsError::Nfs(NfsStat4::Noent))));
    }
}
