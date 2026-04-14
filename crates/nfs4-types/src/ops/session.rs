use bytes::{Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

use crate::base::{
    ClientId4, NfsTime4, SequenceId4, SessionId4, SlotId4, Verifier4,
};
use crate::status::NfsStat4;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Client owner — verifier + opaque owner string (RFC 8881 section 2.4).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ClientOwner4 {
    pub co_verifier: Verifier4,
    pub co_ownerid: Opaque,
}

/// Implementation identifier advertised during EXCHANGE_ID.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct NfsImplId4 {
    pub nii_domain: String,
    pub nii_name: String,
    pub nii_date: NfsTime4,
}

/// State protection selection sent by the client.
///
/// Only SP_NONE (discriminant 0) is supported. The other forms
/// (SP_MACH_CRED, SP_SSV) require RPCSEC_GSS machinery that this
/// minimal client does not implement.
#[derive(Debug, Clone, PartialEq)]
pub enum StateProtect4a {
    SpNone,
}

impl XdrEncode for StateProtect4a {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            StateProtect4a::SpNone => 0u32.encode(buf),
        }
    }
}

impl XdrDecode for StateProtect4a {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => Ok(StateProtect4a::SpNone),
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "StateProtect4a",
            }),
        }
    }
}

/// State protection result returned by the server.
///
/// Only SP_NONE (discriminant 0) is supported — see `StateProtect4a`.
#[derive(Debug, Clone, PartialEq)]
pub enum StateProtect4r {
    SpNone,
}

impl XdrEncode for StateProtect4r {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            StateProtect4r::SpNone => 0u32.encode(buf),
        }
    }
}

impl XdrDecode for StateProtect4r {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => Ok(StateProtect4r::SpNone),
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "StateProtect4r",
            }),
        }
    }
}

/// Server owner returned by EXCHANGE_ID.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ServerOwner4 {
    pub so_minor_id: u64,
    pub so_major_id: Opaque,
}

/// Channel attributes for fore/back channels (RFC 8881 section 18.36).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ChannelAttrs4 {
    pub ca_headerpadsize: u32,
    pub ca_maxrequestsize: u32,
    pub ca_maxresponsesize: u32,
    pub ca_maxresponsesize_cached: u32,
    pub ca_maxoperations: u32,
    pub ca_maxrequests: u32,
    pub ca_rdma_ird: Vec<u32>,
}

/// Callback security parameters.
///
/// Only AUTH_NONE (flavor 0) is supported. AUTH_SYS would require
/// encoding uid/gid/groups which this client does not need.
#[derive(Debug, Clone, PartialEq)]
pub enum CallbackSecParms4 {
    AuthNone,
}

impl XdrEncode for CallbackSecParms4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            CallbackSecParms4::AuthNone => {
                0u32.encode(buf)?; // flavor = AUTH_NONE
                0u32.encode(buf)?; // body length = 0
                Ok(())
            }
        }
    }
}

impl XdrDecode for CallbackSecParms4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let flavor = u32::decode(buf)?;
        match flavor {
            0 => {
                let _body_len = u32::decode(buf)?;
                Ok(CallbackSecParms4::AuthNone)
            }
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "CallbackSecParms4",
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// EXCHANGE_ID (op 42)
// ---------------------------------------------------------------------------

/// EXCHANGE_ID arguments (RFC 8881 section 18.35).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ExchangeId4args {
    pub eia_clientowner: ClientOwner4,
    pub eia_flags: u32,
    pub eia_state_protect: StateProtect4a,
    pub eia_client_impl_id: Vec<NfsImplId4>,
}

/// Successful EXCHANGE_ID result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ExchangeId4resok {
    pub eir_clientid: ClientId4,
    pub eir_sequenceid: SequenceId4,
    pub eir_flags: u32,
    pub eir_state_protect: StateProtect4r,
    pub eir_server_owner: ServerOwner4,
    pub eir_server_scope: Opaque,
    pub eir_server_impl_id: Vec<NfsImplId4>,
}

/// EXCHANGE_ID result — status + optional success data.
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeId4res {
    pub status: NfsStat4,
    pub resok: Option<ExchangeId4resok>,
}

impl XdrEncode for ExchangeId4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for ExchangeId4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(ExchangeId4resok::decode(buf)?)
        } else {
            None
        };
        Ok(ExchangeId4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// CREATE_SESSION (op 43)
// ---------------------------------------------------------------------------

/// CREATE_SESSION arguments (RFC 8881 section 18.36).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct CreateSession4args {
    pub csa_clientid: ClientId4,
    pub csa_sequence: SequenceId4,
    pub csa_flags: u32,
    pub csa_fore_chan_attrs: ChannelAttrs4,
    pub csa_back_chan_attrs: ChannelAttrs4,
    pub csa_cb_program: u32,
    pub csa_sec_parms: Vec<CallbackSecParms4>,
}

/// Successful CREATE_SESSION result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct CreateSession4resok {
    pub csr_sessionid: SessionId4,
    pub csr_sequence: SequenceId4,
    pub csr_flags: u32,
    pub csr_fore_chan_attrs: ChannelAttrs4,
    pub csr_back_chan_attrs: ChannelAttrs4,
}

/// CREATE_SESSION result.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSession4res {
    pub status: NfsStat4,
    pub resok: Option<CreateSession4resok>,
}

impl XdrEncode for CreateSession4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for CreateSession4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(CreateSession4resok::decode(buf)?)
        } else {
            None
        };
        Ok(CreateSession4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// SEQUENCE (op 53)
// ---------------------------------------------------------------------------

/// SEQUENCE arguments (RFC 8881 section 18.46).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Sequence4args {
    pub sa_sessionid: SessionId4,
    pub sa_sequenceid: SequenceId4,
    pub sa_slotid: SlotId4,
    pub sa_highest_slotid: SlotId4,
    pub sa_cachethis: bool,
}

/// Successful SEQUENCE result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Sequence4resok {
    pub sr_sessionid: SessionId4,
    pub sr_sequenceid: SequenceId4,
    pub sr_slotid: SlotId4,
    pub sr_highest_slotid: SlotId4,
    pub sr_target_highest_slotid: SlotId4,
    pub sr_status_flags: u32,
}

/// SEQUENCE result.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence4res {
    pub status: NfsStat4,
    pub resok: Option<Sequence4resok>,
}

impl XdrEncode for Sequence4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Sequence4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Sequence4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Sequence4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// DESTROY_SESSION (op 44)
// ---------------------------------------------------------------------------

/// DESTROY_SESSION arguments (RFC 8881 section 18.37).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct DestroySession4args {
    pub dsa_sessionid: SessionId4,
}

// Result is just NfsStat4, handled by compound dispatch.

// ---------------------------------------------------------------------------
// RECLAIM_COMPLETE (op 58)
// ---------------------------------------------------------------------------

/// RECLAIM_COMPLETE arguments (RFC 8881 section 18.51).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ReclaimComplete4args {
    pub rca_one_fs: bool,
}

// Result is just NfsStat4, handled by compound dispatch.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: XdrEncode + XdrDecode + PartialEq + std::fmt::Debug>(val: &T) {
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = T::decode(&mut bytes).unwrap();
        assert_eq!(*val, decoded);
        assert_eq!(bytes.len(), 0, "leftover bytes after decode");
    }

    #[test]
    fn sequence4args_round_trip() {
        let args = Sequence4args {
            sa_sessionid: SessionId4([0xAB; 16]),
            sa_sequenceid: 1,
            sa_slotid: 0,
            sa_highest_slotid: 0,
            sa_cachethis: false,
        };
        round_trip(&args);
    }

    #[test]
    fn exchange_id4args_round_trip() {
        let args = ExchangeId4args {
            eia_clientowner: ClientOwner4 {
                co_verifier: Verifier4([1; 8]),
                co_ownerid: Opaque(b"test-client".to_vec()),
            },
            eia_flags: 0x00000001,
            eia_state_protect: StateProtect4a::SpNone,
            eia_client_impl_id: vec![NfsImplId4 {
                nii_domain: "example.com".into(),
                nii_name: "test-nfs".into(),
                nii_date: NfsTime4 { seconds: 0, nseconds: 0 },
            }],
        };
        round_trip(&args);
    }

    #[test]
    fn exchange_id4res_ok_round_trip() {
        let res = ExchangeId4res {
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
        };
        round_trip(&res);
    }

    #[test]
    fn exchange_id4res_err_round_trip() {
        let res = ExchangeId4res {
            status: NfsStat4::Serverfault,
            resok: None,
        };
        round_trip(&res);
    }

    #[test]
    fn create_session4args_round_trip() {
        let args = CreateSession4args {
            csa_clientid: ClientId4(42),
            csa_sequence: 1,
            csa_flags: 0,
            csa_fore_chan_attrs: ChannelAttrs4 {
                ca_headerpadsize: 0,
                ca_maxrequestsize: 1048576,
                ca_maxresponsesize: 1048576,
                ca_maxresponsesize_cached: 4096,
                ca_maxoperations: 8,
                ca_maxrequests: 4,
                ca_rdma_ird: vec![],
            },
            csa_back_chan_attrs: ChannelAttrs4 {
                ca_headerpadsize: 0,
                ca_maxrequestsize: 4096,
                ca_maxresponsesize: 4096,
                ca_maxresponsesize_cached: 0,
                ca_maxoperations: 2,
                ca_maxrequests: 1,
                ca_rdma_ird: vec![],
            },
            csa_cb_program: 0,
            csa_sec_parms: vec![CallbackSecParms4::AuthNone],
        };
        round_trip(&args);
    }

    #[test]
    fn sequence4res_ok_round_trip() {
        let res = Sequence4res {
            status: NfsStat4::Ok,
            resok: Some(Sequence4resok {
                sr_sessionid: SessionId4([0xBB; 16]),
                sr_sequenceid: 1,
                sr_slotid: 0,
                sr_highest_slotid: 3,
                sr_target_highest_slotid: 3,
                sr_status_flags: 0,
            }),
        };
        round_trip(&res);
    }

    #[test]
    fn destroy_session4args_round_trip() {
        let args = DestroySession4args {
            dsa_sessionid: SessionId4([0xCC; 16]),
        };
        round_trip(&args);
    }

    #[test]
    fn reclaim_complete4args_round_trip() {
        let args = ReclaimComplete4args { rca_one_fs: false };
        round_trip(&args);
    }
}
