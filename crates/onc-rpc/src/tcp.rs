use std::sync::atomic::{AtomicU32, Ordering};

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use xdr_codec::{Opaque, XdrDecode, XdrEncode};

use crate::auth::AuthFlavor;
use crate::error::RpcError;
use crate::message::{RpcCall, RpcReply};
use crate::record::{RecordReader, RecordResult, write_record};
use crate::transport::{RpcResponse, RpcTransport};

pub struct TcpTransport {
    stream: Mutex<TcpStream>,
    next_xid: AtomicU32,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Mutex::new(stream),
            next_xid: AtomicU32::new(1),
        }
    }

    fn next_xid(&self) -> u32 {
        self.next_xid.fetch_add(1, Ordering::Relaxed)
    }
}

impl RpcTransport for TcpTransport {
    async fn call(
        &self,
        program: u32,
        version: u32,
        procedure: u32,
        args: &[u8],
        cred: &AuthFlavor,
        verf: &AuthFlavor,
    ) -> Result<RpcResponse, RpcError> {
        let xid = self.next_xid();

        let call = RpcCall {
            xid,
            program,
            version,
            procedure,
            cred: cred.clone(),
            verf: verf.clone(),
            body: Opaque(args.to_vec()),
        };
        let mut msg_buf = BytesMut::new();
        call.encode(&mut msg_buf)?;

        let mut wire_buf = BytesMut::new();
        write_record(&msg_buf, &mut wire_buf);

        let mut stream = self.stream.lock().await;

        stream.write_all(&wire_buf).await?;
        stream.flush().await?;

        let mut reader = RecordReader::new();
        let mut read_buf = [0u8; 8192];
        let record = loop {
            let n = stream.read(&mut read_buf).await?;
            if n == 0 {
                return Err(RpcError::Transport(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed while reading reply",
                )));
            }
            match reader.read(&read_buf[..n])? {
                RecordResult::Complete(record) => break record,
                RecordResult::Incomplete => continue,
            }
        };

        let mut record_bytes = record;
        let reply = RpcReply::decode(&mut record_bytes)?;

        if reply.xid != xid {
            return Err(RpcError::XidMismatch {
                expected: xid,
                actual: reply.xid,
            });
        }

        Ok(RpcResponse {
            xid: reply.xid,
            verf: reply.verf,
            status: reply.status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthFlavor;
    use crate::message::{AcceptStatus, RpcCall, RpcReply};
    use crate::record::{RecordReader, RecordResult, write_record};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use xdr_codec::{Opaque, XdrDecode, XdrEncode};

    #[tokio::test]
    async fn tcp_transport_call_and_reply() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn mock server
        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();

            // Read one record from the client
            let mut reader = RecordReader::new();
            let mut buf = [0u8; 8192];
            let record = loop {
                let n = conn.read(&mut buf).await.unwrap();
                assert!(n > 0, "connection closed before complete record");
                match reader.read(&buf[..n]).unwrap() {
                    RecordResult::Complete(record) => break record,
                    RecordResult::Incomplete => continue,
                }
            };

            // Decode the call
            let mut record_bytes = record;
            let call = RpcCall::decode(&mut record_bytes).unwrap();

            // Build a reply with the same xid
            let reply = RpcReply {
                xid: call.xid,
                verf: AuthFlavor::None,
                status: AcceptStatus::Success(Opaque(vec![0xDE, 0xAD, 0xBE, 0xEF])),
            };
            let mut reply_buf = BytesMut::new();
            reply.encode(&mut reply_buf).unwrap();

            let mut wire_buf = BytesMut::new();
            write_record(&reply_buf, &mut wire_buf);

            conn.write_all(&wire_buf).await.unwrap();
            conn.flush().await.unwrap();
        });

        // Connect and make a call
        let stream = TcpStream::connect(addr).await.unwrap();
        let transport = TcpTransport::new(stream);

        let response = transport
            .call(
                100003,
                4,
                1,
                &[],
                &AuthFlavor::None,
                &AuthFlavor::None,
            )
            .await
            .unwrap();

        assert_eq!(
            response.status,
            AcceptStatus::Success(Opaque(vec![0xDE, 0xAD, 0xBE, 0xEF]))
        );
        assert_eq!(response.verf, AuthFlavor::None);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn tcp_transport_sequential_calls() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn mock server that handles 2 calls
        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            let mut reader = RecordReader::new();
            let mut buf = [0u8; 8192];

            for i in 0u8..2 {
                // Read one record
                let record = loop {
                    let n = conn.read(&mut buf).await.unwrap();
                    assert!(n > 0, "connection closed before complete record");
                    match reader.read(&buf[..n]).unwrap() {
                        RecordResult::Complete(record) => break record,
                        RecordResult::Incomplete => continue,
                    }
                };

                let mut record_bytes = record;
                let call = RpcCall::decode(&mut record_bytes).unwrap();

                // Reply with a single byte indicating which call this is
                let reply = RpcReply {
                    xid: call.xid,
                    verf: AuthFlavor::None,
                    status: AcceptStatus::Success(Opaque(vec![i])),
                };
                let mut reply_buf = BytesMut::new();
                reply.encode(&mut reply_buf).unwrap();

                let mut wire_buf = BytesMut::new();
                write_record(&reply_buf, &mut wire_buf);

                conn.write_all(&wire_buf).await.unwrap();
                conn.flush().await.unwrap();
            }
        });

        // Connect and make two sequential calls
        let stream = TcpStream::connect(addr).await.unwrap();
        let transport = TcpTransport::new(stream);

        let response0 = transport
            .call(100003, 4, 1, &[], &AuthFlavor::None, &AuthFlavor::None)
            .await
            .unwrap();
        assert_eq!(
            response0.status,
            AcceptStatus::Success(Opaque(vec![0]))
        );

        let response1 = transport
            .call(100003, 4, 1, &[], &AuthFlavor::None, &AuthFlavor::None)
            .await
            .unwrap();
        assert_eq!(
            response1.status,
            AcceptStatus::Success(Opaque(vec![1]))
        );

        server.await.unwrap();
    }
}
