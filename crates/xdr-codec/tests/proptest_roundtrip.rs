use bytes::BytesMut;
use proptest::prelude::*;
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

fn round_trip<T: XdrEncode + XdrDecode + PartialEq + std::fmt::Debug>(val: &T) {
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    let decoded = T::decode(&mut bytes).unwrap();
    assert_eq!(&decoded, val, "round-trip mismatch");
    assert_eq!(bytes.len(), 0, "trailing bytes after decode");
}

proptest! {
    #[test]
    fn u32_round_trip(val: u32) {
        round_trip(&val);
    }

    #[test]
    fn i32_round_trip(val: i32) {
        round_trip(&val);
    }

    #[test]
    fn u64_round_trip(val: u64) {
        round_trip(&val);
    }

    #[test]
    fn i64_round_trip(val: i64) {
        round_trip(&val);
    }

    #[test]
    fn bool_round_trip(val: bool) {
        round_trip(&val);
    }

    #[test]
    fn opaque_round_trip(data: Vec<u8>) {
        round_trip(&Opaque(data));
    }

    #[test]
    fn string_round_trip(val: String) {
        round_trip(&val);
    }

    #[test]
    fn vec_u32_round_trip(val: Vec<u32>) {
        round_trip(&val);
    }

    #[test]
    fn option_u32_round_trip(val: Option<u32>) {
        round_trip(&val);
    }

    #[test]
    fn opaque_encoding_is_padded(data: Vec<u8>) {
        let mut buf = BytesMut::new();
        Opaque(data.clone()).encode(&mut buf).unwrap();
        assert_eq!(buf.len() % 4, 0, "opaque encoding not 4-byte aligned");
    }

    #[test]
    fn string_encoding_is_padded(val: String) {
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        assert_eq!(buf.len() % 4, 0, "string encoding not 4-byte aligned");
    }
}
