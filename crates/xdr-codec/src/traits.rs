use bytes::{Bytes, BytesMut};

use crate::error::XdrError;

/// Encode a value into XDR format, appending to the buffer.
pub trait XdrEncode {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError>;
}

/// Decode a value from XDR format, advancing the buffer cursor.
pub trait XdrDecode: Sized {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl XdrEncode for Dummy {
        fn encode(&self, _buf: &mut BytesMut) -> Result<(), XdrError> {
            Ok(())
        }
    }

    impl XdrDecode for Dummy {
        fn decode(_buf: &mut Bytes) -> Result<Self, XdrError> {
            Ok(Dummy)
        }
    }

    #[test]
    fn dummy_round_trip() {
        let mut buf = BytesMut::new();
        Dummy.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let _ = Dummy::decode(&mut bytes).unwrap();
    }
}
