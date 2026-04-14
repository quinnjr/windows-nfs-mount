use bytes::{Buf, BufMut, Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

use crate::auth::AuthFlavor;

// RFC 5531 section 9: message type discriminants
pub const MSG_TYPE_CALL: u32 = 0;
pub const MSG_TYPE_REPLY: u32 = 1;

// RPC version for this implementation (RFC 5531)
pub const RPC_VERSION: u32 = 2;

// Reply status discriminants
pub const MSG_ACCEPTED: u32 = 0;
pub const MSG_DENIED: u32 = 1;

// Accept status codes (RFC 5531 section 9)
pub const ACCEPT_SUCCESS: u32 = 0;
pub const ACCEPT_PROG_UNAVAIL: u32 = 1;
pub const ACCEPT_PROG_MISMATCH: u32 = 2;
pub const ACCEPT_PROC_UNAVAIL: u32 = 3;
pub const ACCEPT_GARBAGE_ARGS: u32 = 4;
pub const ACCEPT_SYSTEM_ERR: u32 = 5;

/// An RPC call message (RFC 5531 section 9).
///
/// The `body` field holds the raw opaque bytes of the call arguments.
/// The RPC layer does not interpret them — the program layer above
/// (e.g. NFS) is responsible for encoding/decoding the procedure
/// arguments within the body.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcCall {
    pub xid: u32,
    pub program: u32,
    pub version: u32,
    pub procedure: u32,
    pub cred: AuthFlavor,
    pub verf: AuthFlavor,
    pub body: Opaque,
}

impl XdrEncode for RpcCall {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.xid.encode(buf)?;
        MSG_TYPE_CALL.encode(buf)?;
        RPC_VERSION.encode(buf)?;
        self.program.encode(buf)?;
        self.version.encode(buf)?;
        self.procedure.encode(buf)?;
        self.cred.encode(buf)?;
        self.verf.encode(buf)?;
        // Body is appended raw — not length-prefixed — because the
        // RPC record framing already provides the overall length.
        buf.put_slice(&self.body.0);
        Ok(())
    }
}

impl XdrDecode for RpcCall {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let xid = u32::decode(buf)?;
        let msg_type = u32::decode(buf)?;
        if msg_type != MSG_TYPE_CALL {
            return Err(XdrError::InvalidEnum {
                discriminant: msg_type,
                type_name: "MsgType (expected CALL=0)",
            });
        }
        let rpc_vers = u32::decode(buf)?;
        if rpc_vers != RPC_VERSION {
            return Err(XdrError::InvalidEnum {
                discriminant: rpc_vers,
                type_name: "RpcVersion (expected 2)",
            });
        }
        let program = u32::decode(buf)?;
        let version = u32::decode(buf)?;
        let procedure = u32::decode(buf)?;
        let cred = AuthFlavor::decode(buf)?;
        let verf = AuthFlavor::decode(buf)?;
        // All remaining bytes are the opaque call body.
        let body = Opaque(buf.to_vec());
        buf.advance(buf.remaining());
        Ok(RpcCall {
            xid,
            program,
            version,
            procedure,
            cred,
            verf,
            body,
        })
    }
}

/// Accept status for an RPC reply (RFC 5531 section 9).
#[derive(Debug, Clone, PartialEq)]
pub enum AcceptStatus {
    /// The call completed successfully; body contains the result.
    Success(Opaque),
    /// The remote system does not export the requested program.
    ProgramUnavailable,
    /// The remote system exports the program but not the requested
    /// version. `low` and `high` indicate the supported range.
    ProgramMismatch { low: u32, high: u32 },
    /// The program does not recognize the requested procedure number.
    ProcedureUnavailable,
    /// The call arguments could not be decoded (garbage).
    GarbageArgs,
    /// A generic system error on the server side.
    SystemError,
}

impl XdrEncode for AcceptStatus {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            AcceptStatus::Success(body) => {
                ACCEPT_SUCCESS.encode(buf)?;
                buf.put_slice(&body.0);
            }
            AcceptStatus::ProgramUnavailable => {
                ACCEPT_PROG_UNAVAIL.encode(buf)?;
            }
            AcceptStatus::ProgramMismatch { low, high } => {
                ACCEPT_PROG_MISMATCH.encode(buf)?;
                low.encode(buf)?;
                high.encode(buf)?;
            }
            AcceptStatus::ProcedureUnavailable => {
                ACCEPT_PROC_UNAVAIL.encode(buf)?;
            }
            AcceptStatus::GarbageArgs => {
                ACCEPT_GARBAGE_ARGS.encode(buf)?;
            }
            AcceptStatus::SystemError => {
                ACCEPT_SYSTEM_ERR.encode(buf)?;
            }
        }
        Ok(())
    }
}

impl XdrDecode for AcceptStatus {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let stat = u32::decode(buf)?;
        match stat {
            ACCEPT_SUCCESS => {
                let body = Opaque(buf.to_vec());
                buf.advance(buf.remaining());
                Ok(AcceptStatus::Success(body))
            }
            ACCEPT_PROG_UNAVAIL => Ok(AcceptStatus::ProgramUnavailable),
            ACCEPT_PROG_MISMATCH => {
                let low = u32::decode(buf)?;
                let high = u32::decode(buf)?;
                Ok(AcceptStatus::ProgramMismatch { low, high })
            }
            ACCEPT_PROC_UNAVAIL => Ok(AcceptStatus::ProcedureUnavailable),
            ACCEPT_GARBAGE_ARGS => Ok(AcceptStatus::GarbageArgs),
            ACCEPT_SYSTEM_ERR => Ok(AcceptStatus::SystemError),
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "AcceptStatus",
            }),
        }
    }
}

/// An RPC reply message (RFC 5531 section 9).
///
/// Currently only accepted replies are decoded into structured form.
/// Rejected replies (auth errors, RPC version mismatch) produce an
/// `InvalidEnum` error containing the discriminant for debugging.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcReply {
    pub xid: u32,
    pub verf: AuthFlavor,
    pub status: AcceptStatus,
}

impl XdrEncode for RpcReply {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.xid.encode(buf)?;
        MSG_TYPE_REPLY.encode(buf)?;
        MSG_ACCEPTED.encode(buf)?;
        self.verf.encode(buf)?;
        self.status.encode(buf)?;
        Ok(())
    }
}

impl XdrDecode for RpcReply {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let xid = u32::decode(buf)?;
        let msg_type = u32::decode(buf)?;
        if msg_type != MSG_TYPE_REPLY {
            return Err(XdrError::InvalidEnum {
                discriminant: msg_type,
                type_name: "MsgType (expected REPLY=1)",
            });
        }
        let reply_stat = u32::decode(buf)?;
        if reply_stat != MSG_ACCEPTED {
            return Err(XdrError::InvalidEnum {
                discriminant: reply_stat,
                type_name: "ReplyStat (expected ACCEPTED=0)",
            });
        }
        let verf = AuthFlavor::decode(buf)?;
        let status = AcceptStatus::decode(buf)?;
        Ok(RpcReply { xid, verf, status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthSys;

    #[test]
    fn rpc_call_round_trip() {
        let call = RpcCall {
            xid: 0x12345678,
            program: 100003,
            version: 4,
            procedure: 1,
            cred: AuthFlavor::Sys(AuthSys {
                stamp: 0,
                machine_name: String::from("test"),
                uid: 1000,
                gid: 1000,
                gids: vec![1000],
            }),
            verf: AuthFlavor::None,
            body: Opaque(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let mut buf = BytesMut::new();
        call.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = RpcCall::decode(&mut bytes).unwrap();
        assert_eq!(decoded, call);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn rpc_call_null_procedure() {
        let call = RpcCall {
            xid: 1,
            program: 100003,
            version: 4,
            procedure: 0,
            cred: AuthFlavor::None,
            verf: AuthFlavor::None,
            body: Opaque(vec![]),
        };
        let mut buf = BytesMut::new();
        call.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = RpcCall::decode(&mut bytes).unwrap();
        assert_eq!(decoded, call);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn rpc_reply_success_round_trip() {
        let reply = RpcReply {
            xid: 0xAABBCCDD,
            verf: AuthFlavor::None,
            status: AcceptStatus::Success(Opaque(vec![1, 2, 3, 4])),
        };
        let mut buf = BytesMut::new();
        reply.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = RpcReply::decode(&mut bytes).unwrap();
        assert_eq!(decoded, reply);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn rpc_reply_program_mismatch() {
        let reply = RpcReply {
            xid: 42,
            verf: AuthFlavor::None,
            status: AcceptStatus::ProgramMismatch { low: 3, high: 4 },
        };
        let mut buf = BytesMut::new();
        reply.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = RpcReply::decode(&mut bytes).unwrap();
        assert_eq!(decoded, reply);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn rpc_call_wire_format_header() {
        let call = RpcCall {
            xid: 0x00000001,
            program: 100003,
            version: 4,
            procedure: 0,
            cred: AuthFlavor::None,
            verf: AuthFlavor::None,
            body: Opaque(vec![]),
        };
        let mut buf = BytesMut::new();
        call.encode(&mut buf).unwrap();
        // First 12 bytes: xid (4), MSG_TYPE_CALL=0 (4), RPC_VERSION=2 (4)
        assert_eq!(&buf[0..4], &[0, 0, 0, 1]); // xid = 1
        assert_eq!(&buf[4..8], &[0, 0, 0, 0]); // CALL = 0
        assert_eq!(&buf[8..12], &[0, 0, 0, 2]); // rpc_vers = 2
    }
}
