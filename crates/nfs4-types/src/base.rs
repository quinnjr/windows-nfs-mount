use bytes::{Buf, BufMut, Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

/// NFS filehandle — variable-length opaque, max 128 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NfsFh4(pub Vec<u8>);

impl XdrEncode for NfsFh4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        Opaque(self.0.clone()).encode(buf)
    }
}

impl XdrDecode for NfsFh4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let opaque = Opaque::decode(buf)?;
        if opaque.0.len() > 128 {
            return Err(XdrError::LengthExceeded { max: 128, actual: opaque.0.len() as u32 });
        }
        Ok(NfsFh4(opaque.0))
    }
}

/// Client identifier — u64 assigned by server during EXCHANGE_ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, XdrEncode, XdrDecode)]
pub struct ClientId4(pub u64);

/// Verifier — fixed 8-byte opaque used for reboot detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verifier4(pub [u8; 8]);

impl XdrEncode for Verifier4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for Verifier4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 8 {
            return Err(XdrError::BufferTooShort { needed: 8, available: buf.remaining() });
        }
        let mut arr = [0u8; 8];
        buf.copy_to_slice(&mut arr);
        Ok(Verifier4(arr))
    }
}

/// Session identifier — fixed 16-byte opaque.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId4(pub [u8; 16]);

impl XdrEncode for SessionId4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for SessionId4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 16 {
            return Err(XdrError::BufferTooShort { needed: 16, available: buf.remaining() });
        }
        let mut arr = [0u8; 16];
        buf.copy_to_slice(&mut arr);
        Ok(SessionId4(arr))
    }
}

/// Sequence ID for session slot tracking.
pub type SequenceId4 = u32;

/// Slot ID within a session.
pub type SlotId4 = u32;

/// State ID — identifies open/lock state on the server.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct StateId4 {
    pub seqid: u32,
    pub other: StateIdOther,
}

/// The 12-byte "other" field of a stateid4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateIdOther(pub [u8; 12]);

impl XdrEncode for StateIdOther {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for StateIdOther {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 12 {
            return Err(XdrError::BufferTooShort { needed: 12, available: buf.remaining() });
        }
        let mut arr = [0u8; 12];
        buf.copy_to_slice(&mut arr);
        Ok(StateIdOther(arr))
    }
}

impl StateId4 {
    /// Special stateid meaning "anonymous" / "current stateid".
    pub fn anonymous() -> Self {
        Self { seqid: 0, other: StateIdOther([0; 12]) }
    }
}

/// NFS time — seconds + nanoseconds.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct NfsTime4 {
    pub seconds: i64,
    pub nseconds: u32,
}

/// Change info returned by mutating operations.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct ChangeInfo4 {
    pub atomic: bool,
    pub before: u64,
    pub after: u64,
}

/// NFS program number and version.
pub const NFS4_PROGRAM: u32 = 100003;
pub const NFS4_VERSION: u32 = 4;
pub const NFS4_MINOR_VERSION: u32 = 1;

/// COMPOUND procedure number (always 1 for NFSv4).
pub const NFSPROC4_COMPOUND: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfs_fh4_round_trip() {
        let fh = NfsFh4(vec![1, 2, 3, 4, 5]);
        let mut buf = BytesMut::new();
        fh.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(NfsFh4::decode(&mut bytes).unwrap(), fh);
    }

    #[test]
    fn verifier4_round_trip() {
        let v = Verifier4([0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
        let mut buf = BytesMut::new();
        v.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
        let mut bytes = buf.freeze();
        assert_eq!(Verifier4::decode(&mut bytes).unwrap(), v);
    }

    #[test]
    fn session_id4_round_trip() {
        let sid = SessionId4([1; 16]);
        let mut buf = BytesMut::new();
        sid.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 16);
        let mut bytes = buf.freeze();
        assert_eq!(SessionId4::decode(&mut bytes).unwrap(), sid);
    }

    #[test]
    fn stateid4_round_trip() {
        let st = StateId4 { seqid: 42, other: StateIdOther([0xAA; 12]) };
        let mut buf = BytesMut::new();
        st.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 16);
        let mut bytes = buf.freeze();
        assert_eq!(StateId4::decode(&mut bytes).unwrap(), st);
    }

    #[test]
    fn client_id4_round_trip() {
        let cid = ClientId4(0x123456789ABCDEF0);
        let mut buf = BytesMut::new();
        cid.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(ClientId4::decode(&mut bytes).unwrap(), cid);
    }

    #[test]
    fn nfs_time4_round_trip() {
        let t = NfsTime4 { seconds: 1700000000, nseconds: 500_000_000 };
        let mut buf = BytesMut::new();
        t.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(NfsTime4::decode(&mut bytes).unwrap(), t);
    }

    #[test]
    fn change_info4_round_trip() {
        let ci = ChangeInfo4 { atomic: true, before: 100, after: 101 };
        let mut buf = BytesMut::new();
        ci.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(ChangeInfo4::decode(&mut bytes).unwrap(), ci);
    }
}
