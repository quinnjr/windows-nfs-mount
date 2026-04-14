use bytes::BytesMut;
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
struct Simple {
    a: u32,
    b: i32,
}

#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
struct WithOpaque {
    tag: u32,
    data: Opaque,
}

#[derive(Debug, PartialEq, XdrEncode, XdrDecode)]
struct Nested {
    header: Simple,
    name: String,
}

#[test]
fn simple_struct_round_trip() {
    let val = Simple { a: 1, b: -1 };
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    assert_eq!(buf.len(), 8);
    let mut bytes = buf.freeze();
    assert_eq!(Simple::decode(&mut bytes).unwrap(), val);
}

#[test]
fn simple_struct_wire_format() {
    let val = Simple { a: 0x0A0B0C0D, b: -1 };
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    assert_eq!(
        &buf[..],
        &[0x0A, 0x0B, 0x0C, 0x0D, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn struct_with_opaque_round_trip() {
    let val = WithOpaque {
        tag: 42,
        data: Opaque(vec![1, 2, 3]),
    };
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    assert_eq!(WithOpaque::decode(&mut bytes).unwrap(), val);
}

#[test]
fn nested_struct_round_trip() {
    let val = Nested {
        header: Simple { a: 10, b: 20 },
        name: String::from("test"),
    };
    let mut buf = BytesMut::new();
    val.encode(&mut buf).unwrap();
    let mut bytes = buf.freeze();
    assert_eq!(Nested::decode(&mut bytes).unwrap(), val);
}
