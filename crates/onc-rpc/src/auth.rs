use bytes::{Buf, BufMut, Bytes, BytesMut};
use xdr_codec::{XdrDecode, XdrEncode, XdrError};

pub const AUTH_NONE: u32 = 0;
pub const AUTH_SYS: u32 = 1;
pub const AUTH_TLS: u32 = 7;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthFlavor {
    None,
    Sys(AuthSys),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthSys {
    pub stamp: u32,
    pub machine_name: String,
    pub uid: u32,
    pub gid: u32,
    pub gids: Vec<u32>,
}

impl XdrEncode for AuthFlavor {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            AuthFlavor::None => {
                AUTH_NONE.encode(buf)?;
                0u32.encode(buf)?;
            }
            AuthFlavor::Sys(sys) => {
                AUTH_SYS.encode(buf)?;
                let mut body = BytesMut::new();
                sys.stamp.encode(&mut body)?;
                sys.machine_name.encode(&mut body)?;
                sys.uid.encode(&mut body)?;
                sys.gid.encode(&mut body)?;
                sys.gids.encode(&mut body)?;
                let body_len = body.len() as u32;
                body_len.encode(buf)?;
                buf.put_slice(&body);
            }
        }
        Ok(())
    }
}

impl XdrDecode for AuthFlavor {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let flavor = u32::decode(buf)?;
        let body_len = u32::decode(buf)? as usize;
        match flavor {
            AUTH_NONE => {
                if body_len > 0 {
                    buf.advance(body_len);
                }
                Ok(AuthFlavor::None)
            }
            AUTH_SYS => {
                let stamp = u32::decode(buf)?;
                let machine_name = String::decode(buf)?;
                let uid = u32::decode(buf)?;
                let gid = u32::decode(buf)?;
                let gids = Vec::<u32>::decode(buf)?;
                Ok(AuthFlavor::Sys(AuthSys {
                    stamp,
                    machine_name,
                    uid,
                    gid,
                    gids,
                }))
            }
            _ => {
                if buf.remaining() < body_len {
                    return Err(XdrError::BufferTooShort {
                        needed: body_len,
                        available: buf.remaining(),
                    });
                }
                buf.advance(body_len);
                Ok(AuthFlavor::None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_none_round_trip() {
        let auth = AuthFlavor::None;
        let mut buf = BytesMut::new();
        auth.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
        let mut bytes = buf.freeze();
        assert_eq!(AuthFlavor::decode(&mut bytes).unwrap(), AuthFlavor::None);
    }

    #[test]
    fn auth_none_wire_format() {
        let auth = AuthFlavor::None;
        let mut buf = BytesMut::new();
        auth.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn auth_sys_round_trip() {
        let auth = AuthFlavor::Sys(AuthSys {
            stamp: 1234,
            machine_name: String::from("host"),
            uid: 1000,
            gid: 1000,
            gids: vec![1000, 100],
        });
        let mut buf = BytesMut::new();
        auth.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(AuthFlavor::decode(&mut bytes).unwrap(), auth);
        assert_eq!(bytes.remaining(), 0);
    }

    #[test]
    fn auth_sys_wire_format_flavor() {
        let auth = AuthFlavor::Sys(AuthSys {
            stamp: 0,
            machine_name: String::new(),
            uid: 0,
            gid: 0,
            gids: vec![],
        });
        let mut buf = BytesMut::new();
        auth.encode(&mut buf).unwrap();
        assert_eq!(&buf[..4], &[0, 0, 0, 1]);
    }
}
