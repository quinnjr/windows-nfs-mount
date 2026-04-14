use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::XdrError;
use crate::traits::{XdrDecode, XdrEncode};

impl XdrEncode for u32 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_u32(*self);
        Ok(())
    }
}

impl XdrDecode for u32 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 4 {
            return Err(XdrError::BufferTooShort {
                needed: 4,
                available: buf.remaining(),
            });
        }
        Ok(buf.get_u32())
    }
}

impl XdrEncode for i32 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_i32(*self);
        Ok(())
    }
}

impl XdrDecode for i32 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 4 {
            return Err(XdrError::BufferTooShort {
                needed: 4,
                available: buf.remaining(),
            });
        }
        Ok(buf.get_i32())
    }
}

impl XdrEncode for u64 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_u64(*self);
        Ok(())
    }
}

impl XdrDecode for u64 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 8 {
            return Err(XdrError::BufferTooShort {
                needed: 8,
                available: buf.remaining(),
            });
        }
        Ok(buf.get_u64())
    }
}

impl XdrEncode for i64 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_i64(*self);
        Ok(())
    }
}

impl XdrDecode for i64 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 8 {
            return Err(XdrError::BufferTooShort {
                needed: 8,
                available: buf.remaining(),
            });
        }
        Ok(buf.get_i64())
    }
}

impl XdrEncode for bool {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_u32(u32::from(*self));
        Ok(())
    }
}

impl XdrDecode for bool {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let val = u32::decode(buf)?;
        match val {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(XdrError::InvalidBool(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_round_trip() {
        let mut buf = BytesMut::new();
        42u32.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);
        let mut bytes = buf.freeze();
        assert_eq!(u32::decode(&mut bytes).unwrap(), 42);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn u32_big_endian() {
        let mut buf = BytesMut::new();
        0x01020304u32.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn u32_decode_buffer_too_short() {
        let mut bytes = Bytes::from_static(&[0x00, 0x01]);
        let err = u32::decode(&mut bytes).unwrap_err();
        assert!(matches!(
            err,
            XdrError::BufferTooShort {
                needed: 4,
                available: 2
            }
        ));
    }

    #[test]
    fn i32_round_trip() {
        let mut buf = BytesMut::new();
        (-1i32).encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0xff, 0xff, 0xff, 0xff]);
        let mut bytes = buf.freeze();
        assert_eq!(i32::decode(&mut bytes).unwrap(), -1);
    }

    #[test]
    fn u64_round_trip() {
        let mut buf = BytesMut::new();
        0xDEADBEEFCAFEBABEu64.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
        let mut bytes = buf.freeze();
        assert_eq!(u64::decode(&mut bytes).unwrap(), 0xDEADBEEFCAFEBABE);
    }

    #[test]
    fn i64_round_trip() {
        let mut buf = BytesMut::new();
        (-42i64).encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
        let mut bytes = buf.freeze();
        assert_eq!(i64::decode(&mut bytes).unwrap(), -42);
    }

    #[test]
    fn bool_true_round_trip() {
        let mut buf = BytesMut::new();
        true.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 1]);
        let mut bytes = buf.freeze();
        assert_eq!(bool::decode(&mut bytes).unwrap(), true);
    }

    #[test]
    fn bool_false_round_trip() {
        let mut buf = BytesMut::new();
        false.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 0]);
        let mut bytes = buf.freeze();
        assert_eq!(bool::decode(&mut bytes).unwrap(), false);
    }

    #[test]
    fn bool_invalid_value() {
        let mut bytes = Bytes::from_static(&[0, 0, 0, 2]);
        let err = bool::decode(&mut bytes).unwrap_err();
        assert!(matches!(err, XdrError::InvalidBool(2)));
    }
}
