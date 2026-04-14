use bytes::{Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

/// Attribute bitmap — variable-length array of u32 words.
/// Each bit position identifies an attribute.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct Bitmap4(pub Vec<u32>);

impl Bitmap4 {
    pub fn new() -> Self {
        Self(vec![])
    }

    /// Set a bit in the bitmap, extending if necessary.
    pub fn set(&mut self, bit: u32) {
        let word = (bit / 32) as usize;
        let pos = bit % 32;
        while self.0.len() <= word {
            self.0.push(0);
        }
        self.0[word] |= 1 << pos;
    }

    /// Check if a bit is set.
    pub fn is_set(&self, bit: u32) -> bool {
        let word = (bit / 32) as usize;
        let pos = bit % 32;
        self.0.get(word).map_or(false, |w| (w >> pos) & 1 == 1)
    }
}

impl Default for Bitmap4 {
    fn default() -> Self {
        Self::new()
    }
}

// NFSv4.1 attribute numbers (RFC 8881 section 5.8)
// Word 0 (bits 0-31): mandatory/recommended attributes
pub const FATTR4_SUPPORTED_ATTRS: u32 = 0;
pub const FATTR4_TYPE: u32 = 1;
pub const FATTR4_FH_EXPIRE_TYPE: u32 = 2;
pub const FATTR4_CHANGE: u32 = 3;
pub const FATTR4_SIZE: u32 = 4;
pub const FATTR4_LINK_SUPPORT: u32 = 5;
pub const FATTR4_SYMLINK_SUPPORT: u32 = 6;
pub const FATTR4_NAMED_ATTR: u32 = 7;
pub const FATTR4_FSID: u32 = 8;
pub const FATTR4_UNIQUE_HANDLES: u32 = 9;
pub const FATTR4_LEASE_TIME: u32 = 10;
pub const FATTR4_RDATTR_ERROR: u32 = 11;
pub const FATTR4_FILEHANDLE: u32 = 19;

// Word 1 (bits 32-63): recommended attributes
pub const FATTR4_MODE: u32 = 33;
pub const FATTR4_NUMLINKS: u32 = 35;
pub const FATTR4_OWNER: u32 = 36;
pub const FATTR4_OWNER_GROUP: u32 = 37;
pub const FATTR4_RAWDEV: u32 = 41;
pub const FATTR4_SPACE_USED: u32 = 45;
pub const FATTR4_TIME_ACCESS: u32 = 47;
pub const FATTR4_TIME_METADATA: u32 = 52;
pub const FATTR4_TIME_MODIFY: u32 = 53;

/// NFS file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, XdrEncode, XdrDecode)]
pub enum NfsFType4 {
    #[xdr(discriminant = 1)] Reg,
    #[xdr(discriminant = 2)] Dir,
    #[xdr(discriminant = 3)] Blk,
    #[xdr(discriminant = 4)] Chr,
    #[xdr(discriminant = 5)] Lnk,
    #[xdr(discriminant = 6)] Sock,
    #[xdr(discriminant = 7)] Fifo,
    #[xdr(discriminant = 8)] AttrDir,
    #[xdr(discriminant = 9)] NamedAttr,
}

/// FSID — filesystem identifier.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct FsId4 {
    pub major: u64,
    pub minor: u64,
}

/// File attributes — bitmap + opaque encoded values.
///
/// The attr_vals contains the XDR-encoded attribute values in bitmap
/// order. The caller is responsible for decoding them based on which
/// bits are set in attrmask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fattr4 {
    pub attrmask: Bitmap4,
    pub attr_vals: Vec<u8>,
}

impl XdrEncode for Fattr4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.attrmask.encode(buf)?;
        Opaque(self.attr_vals.clone()).encode(buf)
    }
}

impl XdrDecode for Fattr4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let attrmask = Bitmap4::decode(buf)?;
        let attr_vals = Opaque::decode(buf)?;
        Ok(Fattr4 { attrmask, attr_vals: attr_vals.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_set_and_check() {
        let mut bm = Bitmap4::new();
        bm.set(FATTR4_TYPE);
        bm.set(FATTR4_SIZE);
        bm.set(FATTR4_MODE);

        assert!(bm.is_set(FATTR4_TYPE));
        assert!(bm.is_set(FATTR4_SIZE));
        assert!(bm.is_set(FATTR4_MODE));
        assert!(!bm.is_set(FATTR4_CHANGE));
        assert_eq!(bm.0.len(), 2);
    }

    #[test]
    fn bitmap_round_trip() {
        let mut bm = Bitmap4::new();
        bm.set(0);
        bm.set(31);
        bm.set(32);
        let mut buf = BytesMut::new();
        bm.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(Bitmap4::decode(&mut bytes).unwrap(), bm);
    }

    #[test]
    fn fattr4_round_trip() {
        let mut mask = Bitmap4::new();
        mask.set(FATTR4_SIZE);
        let fa = Fattr4 {
            attrmask: mask,
            attr_vals: vec![0, 0, 0, 0, 0, 0, 0, 42],
        };
        let mut buf = BytesMut::new();
        fa.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(Fattr4::decode(&mut bytes).unwrap(), fa);
    }

    #[test]
    fn nfs_ftype4_round_trip() {
        let ft = NfsFType4::Dir;
        let mut buf = BytesMut::new();
        ft.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 2]);
        let mut bytes = buf.freeze();
        assert_eq!(NfsFType4::decode(&mut bytes).unwrap(), NfsFType4::Dir);
    }
}
