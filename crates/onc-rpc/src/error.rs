use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("XDR codec error: {0}")]
    Xdr(#[from] xdr_codec::XdrError),

    #[error("RPC program mismatch: expected {expected}, got {actual}")]
    ProgramMismatch { expected: u32, actual: u32 },

    #[error("RPC version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    #[error("RPC call rejected: {0}")]
    Rejected(String),

    #[error("RPC accept error: {0}")]
    AcceptError(String),

    #[error("unexpected XID: expected {expected}, got {actual}")]
    XidMismatch { expected: u32, actual: u32 },

    #[error("incomplete record: expected {expected} bytes, got {actual}")]
    IncompleteRecord { expected: usize, actual: usize },

    #[error("transport error: {0}")]
    Transport(#[from] std::io::Error),

    #[error("record too large: {size} bytes (max {max})")]
    RecordTooLarge { size: usize, max: usize },
}
