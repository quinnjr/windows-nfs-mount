use xdr_codec::{XdrDecode, XdrEncode};

/// NFSv4.1 status codes from RFC 8881 section 15.1.
///
/// Every NFS operation returns one of these codes to indicate success or
/// the specific failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, XdrEncode, XdrDecode)]
pub enum NfsStat4 {
    #[xdr(discriminant = 0)]
    Ok,
    #[xdr(discriminant = 1)]
    Perm,
    #[xdr(discriminant = 2)]
    Noent,
    #[xdr(discriminant = 5)]
    Io,
    #[xdr(discriminant = 6)]
    Nxio,
    #[xdr(discriminant = 13)]
    Access,
    #[xdr(discriminant = 17)]
    Exist,
    #[xdr(discriminant = 18)]
    Xdev,
    #[xdr(discriminant = 20)]
    Notdir,
    #[xdr(discriminant = 21)]
    Isdir,
    #[xdr(discriminant = 22)]
    Inval,
    #[xdr(discriminant = 27)]
    Fbig,
    #[xdr(discriminant = 28)]
    Nospc,
    #[xdr(discriminant = 30)]
    Rofs,
    #[xdr(discriminant = 31)]
    Mlink,
    #[xdr(discriminant = 63)]
    Nametoolong,
    #[xdr(discriminant = 66)]
    Notempty,
    #[xdr(discriminant = 69)]
    Dquot,
    #[xdr(discriminant = 70)]
    Stale,
    #[xdr(discriminant = 10001)]
    BadHandle,
    #[xdr(discriminant = 10003)]
    BadCookie,
    #[xdr(discriminant = 10004)]
    Notsupp,
    #[xdr(discriminant = 10005)]
    Toosmall,
    #[xdr(discriminant = 10006)]
    Serverfault,
    #[xdr(discriminant = 10007)]
    Badtype,
    #[xdr(discriminant = 10008)]
    Delay,
    #[xdr(discriminant = 10009)]
    Same,
    #[xdr(discriminant = 10010)]
    Denied,
    #[xdr(discriminant = 10011)]
    Expired,
    #[xdr(discriminant = 10012)]
    Locked,
    #[xdr(discriminant = 10013)]
    Grace,
    #[xdr(discriminant = 10014)]
    FhExpired,
    #[xdr(discriminant = 10015)]
    ShareDenied,
    #[xdr(discriminant = 10016)]
    WrongSec,
    #[xdr(discriminant = 10019)]
    Moved,
    #[xdr(discriminant = 10020)]
    Nofilehandle,
    #[xdr(discriminant = 10021)]
    MinorVersMismatch,
    #[xdr(discriminant = 10022)]
    StaleClientId,
    #[xdr(discriminant = 10023)]
    StaleStateid,
    #[xdr(discriminant = 10024)]
    OldStateid,
    #[xdr(discriminant = 10025)]
    BadStateid,
    #[xdr(discriminant = 10026)]
    BadSeqid,
    #[xdr(discriminant = 10029)]
    Symlink,
    #[xdr(discriminant = 10032)]
    AttrNotsupp,
    #[xdr(discriminant = 10036)]
    BadOwner,
    #[xdr(discriminant = 10038)]
    BadName,
    #[xdr(discriminant = 10044)]
    OpIllegal,
    #[xdr(discriminant = 10047)]
    LocksHeld,
    #[xdr(discriminant = 10048)]
    Openmode,
    #[xdr(discriminant = 10052)]
    BadSession,
    #[xdr(discriminant = 10053)]
    BadSlot,
    #[xdr(discriminant = 10054)]
    CompleteAlready,
    #[xdr(discriminant = 10055)]
    ConnNotBoundToSession,
    #[xdr(discriminant = 10063)]
    SeqMisordered,
    #[xdr(discriminant = 10064)]
    SequencePos,
    #[xdr(discriminant = 10065)]
    ReqTooBig,
    #[xdr(discriminant = 10066)]
    RepTooBig,
    #[xdr(discriminant = 10067)]
    RepTooBigToCache,
    #[xdr(discriminant = 10068)]
    RetryUncachedRep,
    #[xdr(discriminant = 10069)]
    UnsafeCompound,
    #[xdr(discriminant = 10070)]
    TooManyOps,
    #[xdr(discriminant = 10071)]
    OpNotInSession,
    #[xdr(discriminant = 10076)]
    SeqFalseRetry,
    #[xdr(discriminant = 10077)]
    BadHighSlot,
    #[xdr(discriminant = 10078)]
    DeadSession,
}

impl NfsStat4 {
    pub fn is_ok(&self) -> bool {
        matches!(self, NfsStat4::Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn ok_round_trip() {
        let s = NfsStat4::Ok;
        let mut buf = BytesMut::new();
        s.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);
        let mut bytes = buf.freeze();
        assert_eq!(NfsStat4::decode(&mut bytes).unwrap(), NfsStat4::Ok);
    }

    #[test]
    fn stale_round_trip() {
        let s = NfsStat4::Stale;
        let mut buf = BytesMut::new();
        s.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(NfsStat4::decode(&mut bytes).unwrap(), NfsStat4::Stale);
    }

    #[test]
    fn noent_wire_format() {
        let s = NfsStat4::Noent;
        let mut buf = BytesMut::new();
        s.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 2]);
    }

    #[test]
    fn is_ok() {
        assert!(NfsStat4::Ok.is_ok());
        assert!(!NfsStat4::Noent.is_ok());
    }
}
