use bytes::{Bytes, BytesMut};
use xdr_codec::{XdrDecode, XdrEncode, XdrError};

use crate::ops::data::*;
use crate::ops::filehandle::*;
use crate::ops::session::*;
use crate::ops::*;
use crate::status::NfsStat4;

// ---------------------------------------------------------------------------
// NfsArgOp4 — client-side operation arguments
// ---------------------------------------------------------------------------

/// A single operation in a COMPOUND request.
///
/// Each variant carries the arguments for one NFSv4.1 operation.
/// Operations without arguments (PUTROOTFH, GETFH) are unit variants.
#[derive(Debug, Clone, PartialEq)]
pub enum NfsArgOp4 {
    PutRootFh,
    PutFh(PutFh4args),
    Lookup(Lookup4args),
    GetFh,
    GetAttr(GetAttr4args),
    SetAttr(SetAttr4args),
    Open(Open4args),
    Close(Close4args),
    Read(Read4args),
    Write(Write4args),
    ReadDir(ReadDir4args),
    Remove(Remove4args),
    Rename(Rename4args),
    ExchangeId(ExchangeId4args),
    CreateSession(CreateSession4args),
    DestroySession(DestroySession4args),
    Sequence(Sequence4args),
    ReclaimComplete(ReclaimComplete4args),
}

impl XdrEncode for NfsArgOp4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            NfsArgOp4::PutRootFh => OP_PUTROOTFH.encode(buf),
            NfsArgOp4::PutFh(a) => {
                OP_PUTFH.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Lookup(a) => {
                OP_LOOKUP.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::GetFh => OP_GETFH.encode(buf),
            NfsArgOp4::GetAttr(a) => {
                OP_GETATTR.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::SetAttr(a) => {
                OP_SETATTR.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Open(a) => {
                OP_OPEN.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Close(a) => {
                OP_CLOSE.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Read(a) => {
                OP_READ.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Write(a) => {
                OP_WRITE.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::ReadDir(a) => {
                OP_READDIR.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Remove(a) => {
                OP_REMOVE.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Rename(a) => {
                OP_RENAME.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::ExchangeId(a) => {
                OP_EXCHANGE_ID.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::CreateSession(a) => {
                OP_CREATE_SESSION.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::DestroySession(a) => {
                OP_DESTROY_SESSION.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::Sequence(a) => {
                OP_SEQUENCE.encode(buf)?;
                a.encode(buf)
            }
            NfsArgOp4::ReclaimComplete(a) => {
                OP_RECLAIM_COMPLETE.encode(buf)?;
                a.encode(buf)
            }
        }
    }
}

// NfsArgOp4 is not decoded from the wire (the client sends it, not receives).
// We skip XdrDecode for it.

// ---------------------------------------------------------------------------
// NfsResOp4 — server-side operation results
// ---------------------------------------------------------------------------

/// A single operation result in a COMPOUND response.
///
/// Each variant carries the decoded result for one NFSv4.1 operation.
/// Operations whose only result is NfsStat4 store just the status.
#[derive(Debug, Clone, PartialEq)]
pub enum NfsResOp4 {
    PutRootFh(NfsStat4),
    PutFh(NfsStat4),
    Lookup(NfsStat4),
    GetFh(GetFh4res),
    GetAttr(GetAttr4res),
    SetAttr(SetAttr4res),
    Open(Open4res),
    Close(Close4res),
    Read(Read4res),
    Write(Write4res),
    ReadDir(ReadDir4res),
    Remove(Remove4res),
    Rename(Rename4res),
    ExchangeId(ExchangeId4res),
    CreateSession(CreateSession4res),
    DestroySession(NfsStat4),
    Sequence(Sequence4res),
    ReclaimComplete(NfsStat4),
}

impl XdrEncode for NfsResOp4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            NfsResOp4::PutRootFh(s) => {
                OP_PUTROOTFH.encode(buf)?;
                s.encode(buf)
            }
            NfsResOp4::PutFh(s) => {
                OP_PUTFH.encode(buf)?;
                s.encode(buf)
            }
            NfsResOp4::Lookup(s) => {
                OP_LOOKUP.encode(buf)?;
                s.encode(buf)
            }
            NfsResOp4::GetFh(r) => {
                OP_GETFH.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::GetAttr(r) => {
                OP_GETATTR.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::SetAttr(r) => {
                OP_SETATTR.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Open(r) => {
                OP_OPEN.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Close(r) => {
                OP_CLOSE.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Read(r) => {
                OP_READ.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Write(r) => {
                OP_WRITE.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::ReadDir(r) => {
                OP_READDIR.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Remove(r) => {
                OP_REMOVE.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::Rename(r) => {
                OP_RENAME.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::ExchangeId(r) => {
                OP_EXCHANGE_ID.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::CreateSession(r) => {
                OP_CREATE_SESSION.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::DestroySession(s) => {
                OP_DESTROY_SESSION.encode(buf)?;
                s.encode(buf)
            }
            NfsResOp4::Sequence(r) => {
                OP_SEQUENCE.encode(buf)?;
                r.encode(buf)
            }
            NfsResOp4::ReclaimComplete(s) => {
                OP_RECLAIM_COMPLETE.encode(buf)?;
                s.encode(buf)
            }
        }
    }
}

impl XdrDecode for NfsResOp4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let opnum = u32::decode(buf)?;
        match opnum {
            OP_PUTROOTFH => Ok(NfsResOp4::PutRootFh(NfsStat4::decode(buf)?)),
            OP_PUTFH => Ok(NfsResOp4::PutFh(NfsStat4::decode(buf)?)),
            OP_LOOKUP => Ok(NfsResOp4::Lookup(NfsStat4::decode(buf)?)),
            OP_GETFH => Ok(NfsResOp4::GetFh(GetFh4res::decode(buf)?)),
            OP_GETATTR => Ok(NfsResOp4::GetAttr(GetAttr4res::decode(buf)?)),
            OP_SETATTR => Ok(NfsResOp4::SetAttr(SetAttr4res::decode(buf)?)),
            OP_OPEN => Ok(NfsResOp4::Open(Open4res::decode(buf)?)),
            OP_CLOSE => Ok(NfsResOp4::Close(Close4res::decode(buf)?)),
            OP_READ => Ok(NfsResOp4::Read(Read4res::decode(buf)?)),
            OP_WRITE => Ok(NfsResOp4::Write(Write4res::decode(buf)?)),
            OP_READDIR => Ok(NfsResOp4::ReadDir(ReadDir4res::decode(buf)?)),
            OP_REMOVE => Ok(NfsResOp4::Remove(Remove4res::decode(buf)?)),
            OP_RENAME => Ok(NfsResOp4::Rename(Rename4res::decode(buf)?)),
            OP_EXCHANGE_ID => Ok(NfsResOp4::ExchangeId(ExchangeId4res::decode(buf)?)),
            OP_CREATE_SESSION => {
                Ok(NfsResOp4::CreateSession(CreateSession4res::decode(buf)?))
            }
            OP_DESTROY_SESSION => {
                Ok(NfsResOp4::DestroySession(NfsStat4::decode(buf)?))
            }
            OP_SEQUENCE => Ok(NfsResOp4::Sequence(Sequence4res::decode(buf)?)),
            OP_RECLAIM_COMPLETE => {
                Ok(NfsResOp4::ReclaimComplete(NfsStat4::decode(buf)?))
            }
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "NfsResOp4",
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// COMPOUND4args / COMPOUND4res
// ---------------------------------------------------------------------------

/// COMPOUND request (RFC 8881 section 18.1).
///
/// Groups one or more operations to be executed in order by the server.
#[derive(Debug, Clone, PartialEq, XdrEncode)]
pub struct Compound4args {
    pub tag: String,
    pub minorversion: u32,
    pub argarray: Vec<NfsArgOp4>,
}

/// COMPOUND response.
///
/// Contains the overall status plus per-operation results. If an
/// operation fails, the server returns results up to and including the
/// failing operation; later operations are not attempted.
#[derive(Debug, Clone, PartialEq)]
pub struct Compound4res {
    pub status: NfsStat4,
    pub tag: String,
    pub resarray: Vec<NfsResOp4>,
}

impl XdrEncode for Compound4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        self.tag.encode(buf)?;
        self.resarray.encode(buf)
    }
}

impl XdrDecode for Compound4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let tag = String::decode(buf)?;
        let resarray = Vec::<NfsResOp4>::decode(buf)?;
        Ok(Compound4res {
            status,
            tag,
            resarray,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{NfsTime4, SessionId4};
    use crate::bitmap::Bitmap4;
    use xdr_codec::Opaque;

    #[test]
    fn compound4args_sequence_putrootfh_getattr() {
        let mut bm = Bitmap4::new();
        bm.set(1); // FATTR4_TYPE
        bm.set(4); // FATTR4_SIZE

        let args = Compound4args {
            tag: String::new(),
            minorversion: 1,
            argarray: vec![
                NfsArgOp4::Sequence(Sequence4args {
                    sa_sessionid: SessionId4([0xAA; 16]),
                    sa_sequenceid: 1,
                    sa_slotid: 0,
                    sa_highest_slotid: 0,
                    sa_cachethis: false,
                }),
                NfsArgOp4::PutRootFh,
                NfsArgOp4::GetAttr(GetAttr4args { attr_request: bm }),
            ],
        };

        let mut buf = BytesMut::new();
        args.encode(&mut buf).unwrap();

        // Verify it's valid XDR: tag(4) + minorversion(4) + count(4)
        // + op53 header + sequence args
        // + op24 header
        // + op9 header + getattr args
        assert!(buf.len() > 12);

        // Verify the operation count is 3
        let bytes = buf.freeze();
        // tag length(4) + "" (0 data) + minorversion(4) + array count(4)
        // = offset 12 for the count
        assert_eq!(&bytes[8..12], &[0, 0, 0, 3]);
    }

    #[test]
    fn compound4res_round_trip() {
        let res = Compound4res {
            status: NfsStat4::Ok,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0xAA; 16]),
                        sr_sequenceid: 1,
                        sr_slotid: 0,
                        sr_highest_slotid: 3,
                        sr_target_highest_slotid: 3,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::PutRootFh(NfsStat4::Ok),
                NfsResOp4::GetAttr(GetAttr4res {
                    status: NfsStat4::Ok,
                    resok: Some(GetAttr4resok {
                        obj_attributes: crate::bitmap::Fattr4 {
                            attrmask: Bitmap4::new(),
                            attr_vals: vec![],
                        },
                    }),
                }),
            ],
        };

        let mut buf = BytesMut::new();
        res.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = Compound4res::decode(&mut bytes).unwrap();
        assert_eq!(res, decoded);
        assert_eq!(bytes.len(), 0, "leftover bytes after decode");
    }

    #[test]
    fn compound4res_error_stops_early() {
        // Simulate a response where LOOKUP fails — only two results returned.
        let res = Compound4res {
            status: NfsStat4::Noent,
            tag: String::new(),
            resarray: vec![
                NfsResOp4::Sequence(Sequence4res {
                    status: NfsStat4::Ok,
                    resok: Some(Sequence4resok {
                        sr_sessionid: SessionId4([0; 16]),
                        sr_sequenceid: 1,
                        sr_slotid: 0,
                        sr_highest_slotid: 0,
                        sr_target_highest_slotid: 0,
                        sr_status_flags: 0,
                    }),
                }),
                NfsResOp4::Lookup(NfsStat4::Noent),
            ],
        };

        let mut buf = BytesMut::new();
        res.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = Compound4res::decode(&mut bytes).unwrap();
        assert_eq!(decoded.status, NfsStat4::Noent);
        assert_eq!(decoded.resarray.len(), 2);
    }

    #[test]
    fn nfsresop4_exchange_id_round_trip() {
        let res = NfsResOp4::ExchangeId(ExchangeId4res {
            status: NfsStat4::Ok,
            resok: Some(ExchangeId4resok {
                eir_clientid: crate::base::ClientId4(0x1234),
                eir_sequenceid: 1,
                eir_flags: 0,
                eir_state_protect: StateProtect4r::SpNone,
                eir_server_owner: ServerOwner4 {
                    so_minor_id: 0,
                    so_major_id: Opaque(b"srv".to_vec()),
                },
                eir_server_scope: Opaque(b"scope".to_vec()),
                eir_server_impl_id: vec![NfsImplId4 {
                    nii_domain: "kernel.org".into(),
                    nii_name: "linux-nfsd".into(),
                    nii_date: NfsTime4 {
                        seconds: 0,
                        nseconds: 0,
                    },
                }],
            }),
        });

        let mut buf = BytesMut::new();
        res.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = NfsResOp4::decode(&mut bytes).unwrap();
        assert_eq!(res, decoded);
    }
}
