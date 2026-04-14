use bytes::{Bytes, BytesMut};
use xdr_codec::{XdrDecode, XdrEncode, XdrError};

use crate::base::{NfsFh4, StateId4};
use crate::bitmap::{Bitmap4, Fattr4};
use crate::status::NfsStat4;

// ---------------------------------------------------------------------------
// PUTROOTFH (op 24) — no args, result is just NfsStat4
// PUTFH (op 22)
// ---------------------------------------------------------------------------

/// PUTFH arguments — set current filehandle (RFC 8881 section 18.19).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct PutFh4args {
    pub object: NfsFh4,
}

// ---------------------------------------------------------------------------
// LOOKUP (op 15)
// ---------------------------------------------------------------------------

/// LOOKUP arguments — look up a name in the current directory
/// (RFC 8881 section 18.14).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Lookup4args {
    pub objname: String,
}

// ---------------------------------------------------------------------------
// GETFH (op 10)
// ---------------------------------------------------------------------------

// GETFH has no arguments — compound dispatch handles it.

/// GETFH result — returns the current filehandle.
#[derive(Debug, Clone, PartialEq)]
pub struct GetFh4res {
    pub status: NfsStat4,
    pub object: Option<NfsFh4>,
}

impl XdrEncode for GetFh4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref fh) = self.object {
            fh.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for GetFh4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let object = if status.is_ok() {
            Some(NfsFh4::decode(buf)?)
        } else {
            None
        };
        Ok(GetFh4res { status, object })
    }
}

// ---------------------------------------------------------------------------
// GETATTR (op 9)
// ---------------------------------------------------------------------------

/// GETATTR arguments — request specific attributes (RFC 8881 section 18.7).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct GetAttr4args {
    pub attr_request: Bitmap4,
}

/// Successful GETATTR result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct GetAttr4resok {
    pub obj_attributes: Fattr4,
}

/// GETATTR result.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAttr4res {
    pub status: NfsStat4,
    pub resok: Option<GetAttr4resok>,
}

impl XdrEncode for GetAttr4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for GetAttr4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(GetAttr4resok::decode(buf)?)
        } else {
            None
        };
        Ok(GetAttr4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// SETATTR (op 34)
// ---------------------------------------------------------------------------

/// SETATTR arguments (RFC 8881 section 18.30).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct SetAttr4args {
    pub sa_stateid: StateId4,
    pub sa_fattr: Fattr4,
}

/// Successful SETATTR result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct SetAttr4resok {
    pub attrsset: Bitmap4,
}

/// SETATTR result.
#[derive(Debug, Clone, PartialEq)]
pub struct SetAttr4res {
    pub status: NfsStat4,
    pub resok: Option<SetAttr4resok>,
}

impl XdrEncode for SetAttr4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for SetAttr4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(SetAttr4resok::decode(buf)?)
        } else {
            None
        };
        Ok(SetAttr4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::StateIdOther;

    fn round_trip<T: XdrEncode + XdrDecode + PartialEq + std::fmt::Debug>(val: &T) {
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = T::decode(&mut bytes).unwrap();
        assert_eq!(*val, decoded);
        assert_eq!(bytes.len(), 0, "leftover bytes after decode");
    }

    #[test]
    fn putfh4args_round_trip() {
        let args = PutFh4args {
            object: NfsFh4(vec![0xDE, 0xAD]),
        };
        round_trip(&args);
    }

    #[test]
    fn lookup4args_round_trip() {
        let args = Lookup4args {
            objname: "some-file.txt".into(),
        };
        round_trip(&args);
    }

    #[test]
    fn getattr4args_round_trip() {
        let mut bm = Bitmap4::new();
        bm.set(1); // FATTR4_TYPE
        bm.set(4); // FATTR4_SIZE
        let args = GetAttr4args { attr_request: bm };
        round_trip(&args);
    }

    #[test]
    fn getfh4res_ok_round_trip() {
        let res = GetFh4res {
            status: NfsStat4::Ok,
            object: Some(NfsFh4(vec![1, 2, 3])),
        };
        round_trip(&res);
    }

    #[test]
    fn getfh4res_err_round_trip() {
        let res = GetFh4res {
            status: NfsStat4::Nofilehandle,
            object: None,
        };
        round_trip(&res);
    }

    #[test]
    fn getattr4res_ok_round_trip() {
        let mut mask = Bitmap4::new();
        mask.set(4);
        let res = GetAttr4res {
            status: NfsStat4::Ok,
            resok: Some(GetAttr4resok {
                obj_attributes: Fattr4 {
                    attrmask: mask,
                    attr_vals: vec![0, 0, 0, 0, 0, 0, 0, 100],
                },
            }),
        };
        round_trip(&res);
    }

    #[test]
    fn setattr4args_round_trip() {
        let mut mask = Bitmap4::new();
        mask.set(33); // FATTR4_MODE
        let args = SetAttr4args {
            sa_stateid: StateId4 {
                seqid: 0,
                other: StateIdOther([0; 12]),
            },
            sa_fattr: Fattr4 {
                attrmask: mask,
                attr_vals: vec![0, 0, 0x01, 0xA4], // 0644
            },
        };
        round_trip(&args);
    }

    #[test]
    fn setattr4res_ok_round_trip() {
        let mut bm = Bitmap4::new();
        bm.set(33);
        let res = SetAttr4res {
            status: NfsStat4::Ok,
            resok: Some(SetAttr4resok { attrsset: bm }),
        };
        round_trip(&res);
    }
}
