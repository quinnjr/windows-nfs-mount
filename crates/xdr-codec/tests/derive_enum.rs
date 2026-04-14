use bytes::BytesMut;
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

/// Simple C-like enum — discriminant only, no data.
#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
#[repr(u32)]
enum Status {
    Ok = 0,
    Error = 1,
    Retry = 2,
}

/// Discriminated union — each variant carries different data.
#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
enum MyResult {
    #[xdr(discriminant = 0)]
    Success(u32),
    #[xdr(discriminant = 1)]
    Failure(String),
    #[xdr(discriminant = 2)]
    Pending,
}

/// Enum with struct-like variant fields.
#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
enum Message {
    #[xdr(discriminant = 1)]
    Data { seq: u32, payload: Opaque },
    #[xdr(discriminant = 2)]
    Ack { seq: u32 },
    #[xdr(discriminant = 0)]
    Empty,
}

#[test]
fn c_like_enum_round_trip() {
    for (val, expected_disc) in [(Status::Ok, 0u32), (Status::Error, 1), (Status::Retry, 2)] {
        let mut buf = BytesMut::new();
        val.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);
        let mut bytes = buf.freeze();
        assert_eq!(u32::decode(&mut bytes.clone()).unwrap(), expected_disc);
        let decoded = Status::decode(&mut bytes).unwrap();
        assert_eq!(decoded, val);
    }
}

#[test]
fn discriminated_union_success() {
    let val = MyResult::Success(42);
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    assert_eq!(buf.len(), 8); // 4 (disc) + 4 (u32)
    let mut bytes = buf.freeze();
    assert_eq!(MyResult::decode(&mut bytes).unwrap(), val);
}

#[test]
fn discriminated_union_failure() {
    let val = MyResult::Failure(String::from("oops"));
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    assert_eq!(MyResult::decode(&mut bytes).unwrap(), val);
}

#[test]
fn discriminated_union_empty_variant() {
    let val = MyResult::Pending;
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    assert_eq!(buf.len(), 4); // discriminant only
    let mut bytes = buf.freeze();
    assert_eq!(MyResult::decode(&mut bytes).unwrap(), val);
}

#[test]
fn struct_variant_round_trip() {
    let val = Message::Data {
        seq: 1,
        payload: Opaque(vec![0xAA, 0xBB]),
    };
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    assert_eq!(Message::decode(&mut bytes).unwrap(), val);
}

#[test]
fn invalid_discriminant() {
    let mut buf = BytesMut::new();
    99u32.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    let err = MyResult::decode(&mut bytes).unwrap_err();
    assert!(matches!(err, xdr_codec::XdrError::InvalidEnum { discriminant: 99, .. }));
}
