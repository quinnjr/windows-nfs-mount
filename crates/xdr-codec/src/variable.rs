use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::XdrError;
use crate::traits::{XdrDecode, XdrEncode};

/// Calculate the number of padding bytes needed to align to a 4-byte boundary.
fn xdr_padding(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

/// Variable-length opaque data (RFC 4506 section 4.10).
///
/// Wraps `Vec<u8>`. On the wire: length (u32) + data bytes + zero-padding
/// to 4-byte boundary. Distinct from `Vec<u8>` which would encode as a
/// variable-length array of individually-encoded u32 values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opaque(pub Vec<u8>);

impl From<Vec<u8>> for Opaque {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<Opaque> for Vec<u8> {
    fn from(o: Opaque) -> Self {
        o.0
    }
}

impl AsRef<[u8]> for Opaque {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn encode_opaque_bytes(data: &[u8], buf: &mut BytesMut) -> Result<(), XdrError> {
    let len = u32::try_from(data.len())
        .map_err(|_| XdrError::LengthExceeded { max: u32::MAX, actual: 0 })?;
    len.encode(buf)?;
    buf.put_slice(data);
    let pad = xdr_padding(data.len());
    if pad > 0 {
        buf.put_bytes(0, pad);
    }
    Ok(())
}

fn decode_opaque_bytes(buf: &mut Bytes) -> Result<Vec<u8>, XdrError> {
    let len = u32::decode(buf)? as usize;
    let padded = len + xdr_padding(len);
    if buf.remaining() < padded {
        return Err(XdrError::BufferTooShort {
            needed: padded,
            available: buf.remaining(),
        });
    }
    let data = buf.split_to(len).to_vec();
    let pad = xdr_padding(len);
    if pad > 0 {
        buf.advance(pad);
    }
    Ok(data)
}

impl XdrEncode for Opaque {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        encode_opaque_bytes(&self.0, buf)
    }
}

impl XdrDecode for Opaque {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        Ok(Opaque(decode_opaque_bytes(buf)?))
    }
}

impl XdrEncode for String {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        encode_opaque_bytes(self.as_bytes(), buf)
    }
}

impl XdrDecode for String {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let raw = decode_opaque_bytes(buf)?;
        Ok(String::from_utf8(raw)?)
    }
}

/// Variable-length array (RFC 4506 section 4.13).
impl<T: XdrEncode> XdrEncode for Vec<T> {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        let len = u32::try_from(self.len())
            .map_err(|_| XdrError::LengthExceeded { max: u32::MAX, actual: 0 })?;
        len.encode(buf)?;
        for item in self {
            item.encode(buf)?;
        }
        Ok(())
    }
}

impl<T: XdrDecode> XdrDecode for Vec<T> {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let len = u32::decode(buf)? as usize;
        let mut result = Vec::with_capacity(len);
        for _ in 0..len {
            result.push(T::decode(buf)?);
        }
        Ok(result)
    }
}

/// XDR optional-data (RFC 4506 section 4.19).
impl<T: XdrEncode> XdrEncode for Option<T> {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            Some(val) => {
                1u32.encode(buf)?;
                val.encode(buf)?;
            }
            None => {
                0u32.encode(buf)?;
            }
        }
        Ok(())
    }
}

impl<T: XdrDecode> XdrDecode for Option<T> {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let disc = u32::decode(buf)?;
        match disc {
            0 => Ok(None),
            1 => Ok(Some(T::decode(buf)?)),
            other => Err(XdrError::InvalidBool(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_calculation() {
        assert_eq!(xdr_padding(0), 0);
        assert_eq!(xdr_padding(1), 3);
        assert_eq!(xdr_padding(2), 2);
        assert_eq!(xdr_padding(3), 1);
        assert_eq!(xdr_padding(4), 0);
        assert_eq!(xdr_padding(5), 3);
    }

    #[test]
    fn opaque_round_trip_aligned() {
        let data = Opaque(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let mut buf = BytesMut::new();
        data.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8); // 4 (length) + 4 (data) + 0 (padding)
        let mut bytes = buf.freeze();
        assert_eq!(Opaque::decode(&mut bytes).unwrap(), data);
    }

    #[test]
    fn opaque_round_trip_unaligned() {
        let data = Opaque(vec![0x01, 0x02, 0x03]);
        let mut buf = BytesMut::new();
        data.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8); // 4 (length) + 3 (data) + 1 (padding)
        assert_eq!(buf[7], 0); // padding byte is zero
        let mut bytes = buf.freeze();
        assert_eq!(Opaque::decode(&mut bytes).unwrap(), data);
    }

    #[test]
    fn opaque_empty() {
        let data = Opaque(vec![]);
        let mut buf = BytesMut::new();
        data.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);
        let mut bytes = buf.freeze();
        assert_eq!(Opaque::decode(&mut bytes).unwrap(), data);
    }

    #[test]
    fn string_round_trip() {
        let s = String::from("hello");
        let mut buf = BytesMut::new();
        s.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 12); // 4 (length) + 5 (data) + 3 (padding)
        let mut bytes = buf.freeze();
        assert_eq!(String::decode(&mut bytes).unwrap(), "hello");
    }

    #[test]
    fn string_empty() {
        let s = String::new();
        let mut buf = BytesMut::new();
        s.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);
        let mut bytes = buf.freeze();
        assert_eq!(String::decode(&mut bytes).unwrap(), "");
    }

    #[test]
    fn vec_u32_round_trip() {
        let data: Vec<u32> = vec![1, 2, 3];
        let mut buf = BytesMut::new();
        data.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 16); // 4 (length) + 3 * 4 (elements)
        let mut bytes = buf.freeze();
        assert_eq!(Vec::<u32>::decode(&mut bytes).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn option_some_round_trip() {
        let val: Option<u32> = Some(42);
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8); // 4 (discriminant=1) + 4 (value)
        let mut bytes = buf.freeze();
        assert_eq!(Option::<u32>::decode(&mut bytes).unwrap(), Some(42));
    }

    #[test]
    fn option_none_round_trip() {
        let val: Option<u32> = None;
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4); // 4 (discriminant=0)
        let mut bytes = buf.freeze();
        assert_eq!(Option::<u32>::decode(&mut bytes).unwrap(), None);
    }
}
