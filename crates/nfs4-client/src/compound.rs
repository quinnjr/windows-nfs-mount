use xdr_codec::Opaque;

use nfs4_types::ops::data::*;
use nfs4_types::ops::filehandle::*;
use nfs4_types::ops::session::*;
use nfs4_types::*;

pub struct CompoundBuilder {
    tag: String,
    ops: Vec<NfsArgOp4>,
}

impl CompoundBuilder {
    pub fn new() -> Self {
        Self {
            tag: String::new(),
            ops: Vec::new(),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    pub fn sequence(
        mut self,
        session_id: SessionId4,
        sequence_id: SequenceId4,
        slot_id: SlotId4,
        highest_slot_id: SlotId4,
    ) -> Self {
        self.ops.push(NfsArgOp4::Sequence(Sequence4args {
            sa_sessionid: session_id,
            sa_sequenceid: sequence_id,
            sa_slotid: slot_id,
            sa_highest_slotid: highest_slot_id,
            sa_cachethis: false,
        }));
        self
    }

    pub fn putrootfh(mut self) -> Self {
        self.ops.push(NfsArgOp4::PutRootFh);
        self
    }

    pub fn putfh(mut self, fh: NfsFh4) -> Self {
        self.ops.push(NfsArgOp4::PutFh(PutFh4args { object: fh }));
        self
    }

    pub fn lookup(mut self, name: impl Into<String>) -> Self {
        self.ops
            .push(NfsArgOp4::Lookup(Lookup4args { objname: name.into() }));
        self
    }

    pub fn getfh(mut self) -> Self {
        self.ops.push(NfsArgOp4::GetFh);
        self
    }

    pub fn getattr(mut self, attr_request: Bitmap4) -> Self {
        self.ops
            .push(NfsArgOp4::GetAttr(GetAttr4args { attr_request }));
        self
    }

    pub fn setattr(mut self, stateid: StateId4, fattr: Fattr4) -> Self {
        self.ops.push(NfsArgOp4::SetAttr(SetAttr4args {
            sa_stateid: stateid,
            sa_fattr: fattr,
        }));
        self
    }

    pub fn open_by_name(
        mut self,
        name: impl Into<String>,
        share_access: u32,
        owner: Opaque,
    ) -> Self {
        self.ops.push(NfsArgOp4::Open(Open4args {
            seqid: 0, // deprecated in v4.1
            share_access,
            share_deny: OPEN4_SHARE_DENY_NONE,
            owner: OpenOwner4 {
                clientid: ClientId4(0),
                owner,
            },
            openhow: OpenFlag4::NoCreate,
            claim: OpenClaim4::ClaimNull(name.into()),
        }));
        self
    }

    pub fn close(mut self, stateid: StateId4) -> Self {
        self.ops.push(NfsArgOp4::Close(Close4args {
            seqid: 0,
            open_stateid: stateid,
        }));
        self
    }

    pub fn read(mut self, stateid: StateId4, offset: u64, count: u32) -> Self {
        self.ops.push(NfsArgOp4::Read(Read4args {
            stateid,
            offset,
            count,
        }));
        self
    }

    pub fn write(
        mut self,
        stateid: StateId4,
        offset: u64,
        data: Vec<u8>,
        stable: u32,
    ) -> Self {
        self.ops.push(NfsArgOp4::Write(Write4args {
            stateid,
            offset,
            stable,
            data: Opaque(data),
        }));
        self
    }

    pub fn readdir(
        mut self,
        cookie: u64,
        cookieverf: Verifier4,
        dircount: u32,
        maxcount: u32,
        attr_request: Bitmap4,
    ) -> Self {
        self.ops.push(NfsArgOp4::ReadDir(ReadDir4args {
            cookie,
            cookieverf,
            dircount,
            maxcount,
            attr_request,
        }));
        self
    }

    pub fn remove(mut self, target: impl Into<String>) -> Self {
        self.ops
            .push(NfsArgOp4::Remove(Remove4args { target: target.into() }));
        self
    }

    pub fn rename(mut self, oldname: impl Into<String>, newname: impl Into<String>) -> Self {
        self.ops.push(NfsArgOp4::Rename(Rename4args {
            oldname: oldname.into(),
            newname: newname.into(),
        }));
        self
    }

    pub fn exchange_id(mut self, client_owner: ClientOwner4, flags: u32) -> Self {
        self.ops.push(NfsArgOp4::ExchangeId(ExchangeId4args {
            eia_clientowner: client_owner,
            eia_flags: flags,
            eia_state_protect: StateProtect4a::SpNone,
            eia_client_impl_id: vec![],
        }));
        self
    }

    pub fn create_session(
        mut self,
        client_id: ClientId4,
        sequence: SequenceId4,
        flags: u32,
        fore_chan: ChannelAttrs4,
        back_chan: ChannelAttrs4,
    ) -> Self {
        self.ops
            .push(NfsArgOp4::CreateSession(CreateSession4args {
                csa_clientid: client_id,
                csa_sequence: sequence,
                csa_flags: flags,
                csa_fore_chan_attrs: fore_chan,
                csa_back_chan_attrs: back_chan,
                csa_cb_program: 0,
                csa_sec_parms: vec![CallbackSecParms4::AuthNone],
            }));
        self
    }

    pub fn destroy_session(mut self, session_id: SessionId4) -> Self {
        self.ops
            .push(NfsArgOp4::DestroySession(DestroySession4args {
                dsa_sessionid: session_id,
            }));
        self
    }

    pub fn reclaim_complete(mut self, one_fs: bool) -> Self {
        self.ops
            .push(NfsArgOp4::ReclaimComplete(ReclaimComplete4args {
                rca_one_fs: one_fs,
            }));
        self
    }

    pub fn build(self) -> Compound4args {
        Compound4args {
            tag: self.tag,
            minorversion: NFS4_MINOR_VERSION,
            argarray: self.ops,
        }
    }
}

impl Default for CompoundBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_compound() {
        let args = CompoundBuilder::new()
            .tag("test")
            .putrootfh()
            .getfh()
            .build();

        assert_eq!(args.tag, "test");
        assert_eq!(args.minorversion, 1);
        assert_eq!(args.argarray.len(), 2);
        assert!(matches!(args.argarray[0], NfsArgOp4::PutRootFh));
        assert!(matches!(args.argarray[1], NfsArgOp4::GetFh));
    }

    #[test]
    fn build_with_sequence() {
        let sid = SessionId4([0xAA; 16]);
        let args = CompoundBuilder::new()
            .sequence(sid, 1, 0, 0)
            .putrootfh()
            .getattr(Bitmap4::new())
            .build();

        assert_eq!(args.argarray.len(), 3);
        assert!(matches!(args.argarray[0], NfsArgOp4::Sequence(_)));
    }

    #[test]
    fn build_lookup_chain() {
        let args = CompoundBuilder::new()
            .putrootfh()
            .lookup("usr")
            .lookup("local")
            .lookup("bin")
            .getfh()
            .build();

        assert_eq!(args.argarray.len(), 5);
    }

    #[test]
    fn compound_encodes() {
        use bytes::BytesMut;
        use xdr_codec::XdrEncode;

        let args = CompoundBuilder::new().putrootfh().getfh().build();

        let mut buf = BytesMut::new();
        args.encode(&mut buf).unwrap();
        assert!(!buf.is_empty());
    }
}
