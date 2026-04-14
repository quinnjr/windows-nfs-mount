use bytes::{Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

use crate::base::{ChangeInfo4, ClientId4, StateId4, Verifier4};
use crate::bitmap::{Bitmap4, Fattr4};
use crate::status::NfsStat4;

// ---------------------------------------------------------------------------
// OPEN constants and supporting types
// ---------------------------------------------------------------------------

pub const OPEN4_SHARE_ACCESS_READ: u32 = 0x00000001;
pub const OPEN4_SHARE_ACCESS_WRITE: u32 = 0x00000002;
pub const OPEN4_SHARE_ACCESS_BOTH: u32 = 0x00000003;
pub const OPEN4_SHARE_DENY_NONE: u32 = 0x00000000;

// createhow4 discriminants
const UNCHECKED4: u32 = 0;

// open_claim_type4 discriminants
const CLAIM_NULL: u32 = 0;
const CLAIM_FH: u32 = 4;

// open_delegation_type4 discriminants
const OPEN_DELEGATE_NONE: u32 = 0;

/// Open owner — identifies the open-owner for lock state.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct OpenOwner4 {
    pub clientid: ClientId4,
    pub owner: Opaque,
}

/// How to open a file — create or no-create.
///
/// OPEN4_NOCREATE (disc 0) carries no payload; OPEN4_CREATE (disc 1)
/// carries a createhow4 which for UNCHECKED4 is just an Fattr4.
#[derive(Debug, Clone, PartialEq)]
pub enum OpenFlag4 {
    NoCreate,
    CreateUnchecked(Fattr4),
}

impl XdrEncode for OpenFlag4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            OpenFlag4::NoCreate => 0u32.encode(buf),
            OpenFlag4::CreateUnchecked(attrs) => {
                1u32.encode(buf)?; // OPEN4_CREATE
                UNCHECKED4.encode(buf)?;
                attrs.encode(buf)
            }
        }
    }
}

impl XdrDecode for OpenFlag4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => Ok(OpenFlag4::NoCreate),
            1 => {
                let how = u32::decode(buf)?;
                match how {
                    0 => {
                        // UNCHECKED4
                        let attrs = Fattr4::decode(buf)?;
                        Ok(OpenFlag4::CreateUnchecked(attrs))
                    }
                    other => Err(XdrError::InvalidEnum {
                        discriminant: other,
                        type_name: "createhow4",
                    }),
                }
            }
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "OpenFlag4",
            }),
        }
    }
}

/// What to open — claim type.
///
/// CLAIM_NULL (disc 0) opens by filename relative to current FH.
/// CLAIM_FH (disc 4) opens the current FH directly (no name needed).
#[derive(Debug, Clone, PartialEq)]
pub enum OpenClaim4 {
    ClaimNull(String),
    ClaimFh,
}

impl XdrEncode for OpenClaim4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            OpenClaim4::ClaimNull(name) => {
                CLAIM_NULL.encode(buf)?;
                name.encode(buf)
            }
            OpenClaim4::ClaimFh => CLAIM_FH.encode(buf),
        }
    }
}

impl XdrDecode for OpenClaim4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => {
                let name = String::decode(buf)?;
                Ok(OpenClaim4::ClaimNull(name))
            }
            4 => Ok(OpenClaim4::ClaimFh),
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "OpenClaim4",
            }),
        }
    }
}

/// Open delegation type returned to the client.
///
/// Only OPEN_DELEGATE_NONE (disc 0) is supported — this client does
/// not request or handle delegations.
#[derive(Debug, Clone, PartialEq)]
pub enum OpenDelegation4 {
    None,
}

impl XdrEncode for OpenDelegation4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            OpenDelegation4::None => OPEN_DELEGATE_NONE.encode(buf),
        }
    }
}

impl XdrDecode for OpenDelegation4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => Ok(OpenDelegation4::None),
            other => Err(XdrError::InvalidEnum {
                discriminant: other,
                type_name: "OpenDelegation4",
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// OPEN (op 18)
// ---------------------------------------------------------------------------

/// OPEN arguments (RFC 8881 section 18.16).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Open4args {
    pub seqid: u32,
    pub share_access: u32,
    pub share_deny: u32,
    pub owner: OpenOwner4,
    pub openhow: OpenFlag4,
    pub claim: OpenClaim4,
}

/// Successful OPEN result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Open4resok {
    pub stateid: StateId4,
    pub cinfo: ChangeInfo4,
    pub rflags: u32,
    pub attrset: Bitmap4,
    pub delegation: OpenDelegation4,
}

/// OPEN result.
#[derive(Debug, Clone, PartialEq)]
pub struct Open4res {
    pub status: NfsStat4,
    pub resok: Option<Open4resok>,
}

impl XdrEncode for Open4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Open4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Open4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Open4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// CLOSE (op 4)
// ---------------------------------------------------------------------------

/// CLOSE arguments (RFC 8881 section 18.2).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Close4args {
    pub seqid: u32,
    pub open_stateid: StateId4,
}

/// Successful CLOSE result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Close4resok {
    pub open_stateid: StateId4,
}

/// CLOSE result.
#[derive(Debug, Clone, PartialEq)]
pub struct Close4res {
    pub status: NfsStat4,
    pub resok: Option<Close4resok>,
}

impl XdrEncode for Close4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Close4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Close4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Close4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// READ (op 25)
// ---------------------------------------------------------------------------

/// READ arguments (RFC 8881 section 18.22).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Read4args {
    pub stateid: StateId4,
    pub offset: u64,
    pub count: u32,
}

/// Successful READ result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Read4resok {
    pub eof: bool,
    pub data: Opaque,
}

/// READ result.
#[derive(Debug, Clone, PartialEq)]
pub struct Read4res {
    pub status: NfsStat4,
    pub resok: Option<Read4resok>,
}

impl XdrEncode for Read4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Read4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Read4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Read4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// WRITE (op 38)
// ---------------------------------------------------------------------------

/// Write stability levels.
pub const UNSTABLE4: u32 = 0;
pub const DATA_SYNC4: u32 = 1;
pub const FILE_SYNC4: u32 = 2;

/// WRITE arguments (RFC 8881 section 18.32).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Write4args {
    pub stateid: StateId4,
    pub offset: u64,
    pub stable: u32,
    pub data: Opaque,
}

/// Successful WRITE result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Write4resok {
    pub count: u32,
    pub committed: u32,
    pub writeverf: Verifier4,
}

/// WRITE result.
#[derive(Debug, Clone, PartialEq)]
pub struct Write4res {
    pub status: NfsStat4,
    pub resok: Option<Write4resok>,
}

impl XdrEncode for Write4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Write4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Write4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Write4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// READDIR (op 26)
// ---------------------------------------------------------------------------

/// A single directory entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry4 {
    pub cookie: u64,
    pub name: String,
    pub attrs: Fattr4,
}

/// Directory listing — entries + end-of-directory flag.
#[derive(Debug, Clone, PartialEq)]
pub struct DirList4 {
    pub entries: Vec<Entry4>,
    pub eof: bool,
}

/// READDIR arguments (RFC 8881 section 18.23).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ReadDir4args {
    pub cookie: u64,
    pub cookieverf: Verifier4,
    pub dircount: u32,
    pub maxcount: u32,
    pub attr_request: Bitmap4,
}

/// Successful READDIR result data.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadDir4resok {
    pub cookieverf: Verifier4,
    pub reply: DirList4,
}

impl XdrEncode for ReadDir4resok {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.cookieverf.encode(buf)?;
        // Entries are a chain: true + entry, ..., false (end of chain).
        for entry in &self.reply.entries {
            true.encode(buf)?;
            entry.cookie.encode(buf)?;
            entry.name.encode(buf)?;
            entry.attrs.encode(buf)?;
        }
        false.encode(buf)?; // no more entries
        self.reply.eof.encode(buf)?;
        Ok(())
    }
}

impl XdrDecode for ReadDir4resok {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let cookieverf = Verifier4::decode(buf)?;
        let mut entries = Vec::new();
        loop {
            let has_entry = bool::decode(buf)?;
            if !has_entry {
                break;
            }
            let cookie = u64::decode(buf)?;
            let name = String::decode(buf)?;
            let attrs = Fattr4::decode(buf)?;
            entries.push(Entry4 { cookie, name, attrs });
        }
        let eof = bool::decode(buf)?;
        Ok(ReadDir4resok {
            cookieverf,
            reply: DirList4 { entries, eof },
        })
    }
}

/// READDIR result.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadDir4res {
    pub status: NfsStat4,
    pub resok: Option<ReadDir4resok>,
}

impl XdrEncode for ReadDir4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for ReadDir4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(ReadDir4resok::decode(buf)?)
        } else {
            None
        };
        Ok(ReadDir4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// REMOVE (op 28)
// ---------------------------------------------------------------------------

/// REMOVE arguments (RFC 8881 section 18.25).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Remove4args {
    pub target: String,
}

/// Successful REMOVE result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Remove4resok {
    pub cinfo: ChangeInfo4,
}

/// REMOVE result.
#[derive(Debug, Clone, PartialEq)]
pub struct Remove4res {
    pub status: NfsStat4,
    pub resok: Option<Remove4resok>,
}

impl XdrEncode for Remove4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Remove4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Remove4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Remove4res { status, resok })
    }
}

// ---------------------------------------------------------------------------
// RENAME (op 29)
// ---------------------------------------------------------------------------

/// RENAME arguments (RFC 8881 section 18.26).
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Rename4args {
    pub oldname: String,
    pub newname: String,
}

/// Successful RENAME result data.
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Rename4resok {
    pub source_cinfo: ChangeInfo4,
    pub target_cinfo: ChangeInfo4,
}

/// RENAME result.
#[derive(Debug, Clone, PartialEq)]
pub struct Rename4res {
    pub status: NfsStat4,
    pub resok: Option<Rename4resok>,
}

impl XdrEncode for Rename4res {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.status.encode(buf)?;
        if let Some(ref ok) = self.resok {
            ok.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for Rename4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let resok = if status.is_ok() {
            Some(Rename4resok::decode(buf)?)
        } else {
            None
        };
        Ok(Rename4res { status, resok })
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
    fn read4args_round_trip() {
        let args = Read4args {
            stateid: StateId4::anonymous(),
            offset: 0,
            count: 65536,
        };
        round_trip(&args);
    }

    #[test]
    fn write4args_round_trip() {
        let args = Write4args {
            stateid: StateId4::anonymous(),
            offset: 1024,
            stable: FILE_SYNC4,
            data: Opaque(b"hello world".to_vec()),
        };
        round_trip(&args);
    }

    #[test]
    fn close4args_round_trip() {
        let args = Close4args {
            seqid: 1,
            open_stateid: StateId4 {
                seqid: 2,
                other: StateIdOther([0xAA; 12]),
            },
        };
        round_trip(&args);
    }

    #[test]
    fn close4res_ok_round_trip() {
        let res = Close4res {
            status: NfsStat4::Ok,
            resok: Some(Close4resok {
                open_stateid: StateId4 {
                    seqid: 3,
                    other: StateIdOther([0xBB; 12]),
                },
            }),
        };
        round_trip(&res);
    }

    #[test]
    fn open4args_nocreate_round_trip() {
        let args = Open4args {
            seqid: 0,
            share_access: OPEN4_SHARE_ACCESS_READ,
            share_deny: OPEN4_SHARE_DENY_NONE,
            owner: OpenOwner4 {
                clientid: ClientId4(42),
                owner: Opaque(b"owner".to_vec()),
            },
            openhow: OpenFlag4::NoCreate,
            claim: OpenClaim4::ClaimNull("test.txt".into()),
        };
        round_trip(&args);
    }

    #[test]
    fn open4args_create_round_trip() {
        let args = Open4args {
            seqid: 0,
            share_access: OPEN4_SHARE_ACCESS_BOTH,
            share_deny: OPEN4_SHARE_DENY_NONE,
            owner: OpenOwner4 {
                clientid: ClientId4(42),
                owner: Opaque(b"owner".to_vec()),
            },
            openhow: OpenFlag4::CreateUnchecked(Fattr4 {
                attrmask: Bitmap4::new(),
                attr_vals: vec![],
            }),
            claim: OpenClaim4::ClaimFh,
        };
        round_trip(&args);
    }

    #[test]
    fn remove4args_round_trip() {
        let args = Remove4args {
            target: "old-file.txt".into(),
        };
        round_trip(&args);
    }

    #[test]
    fn rename4args_round_trip() {
        let args = Rename4args {
            oldname: "old.txt".into(),
            newname: "new.txt".into(),
        };
        round_trip(&args);
    }

    #[test]
    fn readdir4args_round_trip() {
        let args = ReadDir4args {
            cookie: 0,
            cookieverf: Verifier4([0; 8]),
            dircount: 8192,
            maxcount: 32768,
            attr_request: Bitmap4::new(),
        };
        round_trip(&args);
    }

    #[test]
    fn readdir4resok_round_trip() {
        let resok = ReadDir4resok {
            cookieverf: Verifier4([0xFF; 8]),
            reply: DirList4 {
                entries: vec![
                    Entry4 {
                        cookie: 1,
                        name: "file1.txt".into(),
                        attrs: Fattr4 {
                            attrmask: Bitmap4::new(),
                            attr_vals: vec![],
                        },
                    },
                    Entry4 {
                        cookie: 2,
                        name: "file2.txt".into(),
                        attrs: Fattr4 {
                            attrmask: Bitmap4::new(),
                            attr_vals: vec![],
                        },
                    },
                ],
                eof: true,
            },
        };
        round_trip(&resok);
    }

    #[test]
    fn readdir4resok_empty_round_trip() {
        let resok = ReadDir4resok {
            cookieverf: Verifier4([0; 8]),
            reply: DirList4 {
                entries: vec![],
                eof: true,
            },
        };
        round_trip(&resok);
    }

    #[test]
    fn read4res_ok_round_trip() {
        let res = Read4res {
            status: NfsStat4::Ok,
            resok: Some(Read4resok {
                eof: false,
                data: Opaque(b"file content".to_vec()),
            }),
        };
        round_trip(&res);
    }

    #[test]
    fn write4res_ok_round_trip() {
        let res = Write4res {
            status: NfsStat4::Ok,
            resok: Some(Write4resok {
                count: 11,
                committed: FILE_SYNC4,
                writeverf: Verifier4([0xAA; 8]),
            }),
        };
        round_trip(&res);
    }
}
