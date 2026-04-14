use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

use onc_rpc::{
    AcceptStatus, AuthFlavor, RecordReader, RecordResult, RpcCall, RpcReply, RpcTransport,
    TcpTransport, write_record,
};

/// Helper: read exactly one complete RPC record from `conn`.
async fn read_one_record(
    conn: &mut TcpStream,
    reader: &mut RecordReader,
) -> bytes::Bytes {
    let mut buf = [0u8; 8192];
    loop {
        let n = conn.read(&mut buf).await.unwrap();
        assert!(n > 0, "connection closed before complete record");
        match reader.read(&buf[..n]).unwrap() {
            RecordResult::Complete(record) => return record,
            RecordResult::Incomplete => continue,
        }
    }
}

/// Helper: encode an `RpcReply` and write it as a framed record.
async fn send_reply(conn: &mut TcpStream, reply: &RpcReply) {
    let mut reply_buf = BytesMut::new();
    reply.encode(&mut reply_buf).unwrap();
    let mut wire_buf = BytesMut::new();
    write_record(&reply_buf, &mut wire_buf);
    conn.write_all(&wire_buf).await.unwrap();
    conn.flush().await.unwrap();
}

/// Full round trip with AUTH_SYS credentials.
///
/// The mock server reads a call, extracts the procedure number, and
/// replies with that number encoded as a big-endian u32 in the body.
/// The test verifies that the transport layer faithfully delivers the
/// procedure number and that AUTH_SYS credentials do not interfere
/// with the round trip.
#[tokio::test]
async fn full_rpc_round_trip_with_auth_sys() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut conn, _) = listener.accept().await.unwrap();
        let mut reader = RecordReader::new();

        let record = read_one_record(&mut conn, &mut reader).await;
        let mut record_bytes = record;
        let call = RpcCall::decode(&mut record_bytes).unwrap();

        // Echo back the procedure number as a u32 body.
        let mut body = BytesMut::new();
        call.procedure.encode(&mut body).unwrap();

        let reply = RpcReply {
            xid: call.xid,
            verf: AuthFlavor::None,
            status: AcceptStatus::Success(Opaque(body.to_vec())),
        };
        send_reply(&mut conn, &reply).await;
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let transport = TcpTransport::new(stream);

    let cred = AuthFlavor::Sys(onc_rpc::AuthSys {
        stamp: 12345,
        machine_name: String::from("testhost"),
        uid: 1000,
        gid: 1000,
        gids: vec![1000, 100, 10],
    });

    let response = transport
        .call(100003, 4, 42, &[], &cred, &AuthFlavor::None)
        .await
        .unwrap();

    // The body should contain a single u32 = 42.
    match &response.status {
        AcceptStatus::Success(body) => {
            let mut data = bytes::Bytes::from(body.0.clone());
            let echoed = u32::decode(&mut data).unwrap();
            assert_eq!(echoed, 42);
        }
        other => panic!("expected Success, got {other:?}"),
    }

    server.await.unwrap();
}

/// Multiple sequential calls with distinct procedure numbers.
///
/// Ensures that one TCP connection handles back-to-back calls
/// correctly and that each response carries the right procedure
/// number in its body.
#[tokio::test]
async fn multiple_sequential_calls_different_procedures() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut conn, _) = listener.accept().await.unwrap();
        let mut reader = RecordReader::new();

        for _ in 0..3 {
            let record = read_one_record(&mut conn, &mut reader).await;
            let mut record_bytes = record;
            let call = RpcCall::decode(&mut record_bytes).unwrap();

            let mut body = BytesMut::new();
            call.procedure.encode(&mut body).unwrap();

            let reply = RpcReply {
                xid: call.xid,
                verf: AuthFlavor::None,
                status: AcceptStatus::Success(Opaque(body.to_vec())),
            };
            send_reply(&mut conn, &reply).await;
        }
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let transport = TcpTransport::new(stream);

    let procedures = [0u32, 1, 255];

    for &proc_num in &procedures {
        let response = transport
            .call(
                100003,
                4,
                proc_num,
                &[],
                &AuthFlavor::None,
                &AuthFlavor::None,
            )
            .await
            .unwrap();

        match &response.status {
            AcceptStatus::Success(body) => {
                let mut data = bytes::Bytes::from(body.0.clone());
                let echoed = u32::decode(&mut data).unwrap();
                assert_eq!(
                    echoed, proc_num,
                    "procedure {proc_num} was not echoed correctly"
                );
            }
            other => panic!("expected Success for procedure {proc_num}, got {other:?}"),
        }
    }

    server.await.unwrap();
}

/// Call with a 128-byte body payload.
///
/// The mock server reads the call body, measures its length, and
/// replies with that length as a u32. This proves the body bytes
/// survive the XDR-encode -> record-frame -> TCP -> record-unframe ->
/// XDR-decode pipeline intact.
#[tokio::test]
async fn call_with_body_data() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut conn, _) = listener.accept().await.unwrap();
        let mut reader = RecordReader::new();

        let record = read_one_record(&mut conn, &mut reader).await;
        let mut record_bytes = record;
        let call = RpcCall::decode(&mut record_bytes).unwrap();

        // Reply with the body length as a u32.
        let body_len = call.body.0.len() as u32;
        let mut body = BytesMut::new();
        body_len.encode(&mut body).unwrap();

        let reply = RpcReply {
            xid: call.xid,
            verf: AuthFlavor::None,
            status: AcceptStatus::Success(Opaque(body.to_vec())),
        };
        send_reply(&mut conn, &reply).await;
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let transport = TcpTransport::new(stream);

    let payload = vec![0u8; 128];

    let response = transport
        .call(
            100003,
            4,
            1,
            &payload,
            &AuthFlavor::None,
            &AuthFlavor::None,
        )
        .await
        .unwrap();

    match &response.status {
        AcceptStatus::Success(body) => {
            let mut data = bytes::Bytes::from(body.0.clone());
            let length = u32::decode(&mut data).unwrap();
            assert_eq!(length, 128);
        }
        other => panic!("expected Success, got {other:?}"),
    }

    server.await.unwrap();
}
