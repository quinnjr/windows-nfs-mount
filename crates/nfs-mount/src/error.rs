use thiserror::Error;
use nfs4_client::NfsError;

#[derive(Debug, Error)]
pub enum MountError {
    #[error("NFS error: {0}")]
    Nfs(#[from] NfsError),

    #[error("path error: {0}")]
    InvalidPath(String),

    #[error("file not open: handle {0}")]
    NotOpen(u64),

    #[error("stale file handle")]
    StaleHandle,

    #[error("not a directory")]
    NotDirectory,

    #[error("is a directory")]
    IsDirectory,

    #[error("I/O error: {0}")]
    Io(String),
}
