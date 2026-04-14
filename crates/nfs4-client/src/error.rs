use thiserror::Error;

use nfs4_types::NfsStat4;

#[derive(Debug, Error)]
pub enum NfsError {
    #[error("NFS error: {0:?}")]
    Nfs(NfsStat4),

    #[error("RPC error: {0}")]
    Rpc(#[from] onc_rpc::RpcError),

    #[error("XDR error: {0}")]
    Xdr(#[from] xdr_codec::XdrError),

    #[error("no session established")]
    NoSession,

    #[error("session expired")]
    SessionExpired,

    #[error("operation at index {index} failed: {status:?}")]
    OperationFailed { index: usize, status: NfsStat4 },

    #[error("unexpected response: expected {expected}, got {actual}")]
    UnexpectedResponse {
        expected: &'static str,
        actual: String,
    },
}
