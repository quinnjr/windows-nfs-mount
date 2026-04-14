use crate::auth::AuthFlavor;
use crate::error::RpcError;
use crate::message::AcceptStatus;

#[derive(Debug)]
pub struct RpcResponse {
    pub xid: u32,
    pub verf: AuthFlavor,
    pub status: AcceptStatus,
}

pub trait RpcTransport: Send + Sync {
    fn call(
        &self,
        program: u32,
        version: u32,
        procedure: u32,
        args: &[u8],
        cred: &AuthFlavor,
        verf: &AuthFlavor,
    ) -> impl std::future::Future<Output = Result<RpcResponse, RpcError>> + Send;
}
