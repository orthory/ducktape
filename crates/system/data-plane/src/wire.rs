//! Wire encoding — the cross-node surface of the plane.
//!
//! Datagram frame (12-byte header, then payload):
//!
//! ```text
//! offset  0        1          2..10          10..12
//!         ver = 1  service    flow (u64 BE)  reserved = 0
//! ```
//!
//! Stream hello (one length-prefixed frame, opener → acceptor, answered by a
//! single [`HELLO_ACK`] byte):
//!
//! ```text
//! u16 BE len | ver = 1 | service | flow (u64 BE) | intent | meta (len-11 bytes)
//! ```
//!
//! `intent` and `meta` are service-defined and opaque to the plane.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Service;
use crate::flow::FlowId;

pub const WIRE_VERSION: u8 = 1;

/// Largest whole datagram frame. Derivation: overlay MTU 1420 (WireGuard
/// over a 1500 underlay) − 40 (IPv6) − 8 (UDP) = 1372 UDP payload bytes.
/// The plane never fragments — consumers must fit the payload bound.
pub const MAX_DATAGRAM: usize = 1372;
pub const DATAGRAM_HEADER_LEN: usize = 12;
pub const MAX_DATAGRAM_PAYLOAD: usize = MAX_DATAGRAM - DATAGRAM_HEADER_LEN;

/// Hello body length ahead of meta: ver + service + flow + intent.
const HELLO_FIXED_LEN: usize = 11;
pub const MAX_HELLO_META: usize = 1024;

/// The single byte an acceptor answers a hello with. Anything else (or a
/// closed stream) means the open was refused.
pub const HELLO_ACK: u8 = 0x06;

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("datagram payload {len} exceeds {MAX_DATAGRAM_PAYLOAD}")]
    PayloadTooLarge { len: usize },
    #[error("hello meta {len} exceeds {MAX_HELLO_META}")]
    MetaTooLarge { len: usize },
    #[error("frame truncated")]
    Truncated,
    #[error("unsupported wire version {0}")]
    BadVersion(u8),
    #[error("unknown service id {0}")]
    UnknownService(u8),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub fn encode_datagram(
    service: Service,
    flow: FlowId,
    payload: &[u8],
) -> Result<Vec<u8>, WireError> {
    if payload.len() > MAX_DATAGRAM_PAYLOAD {
        return Err(WireError::PayloadTooLarge { len: payload.len() });
    }
    let mut frame = Vec::with_capacity(DATAGRAM_HEADER_LEN + payload.len());
    frame.push(WIRE_VERSION);
    frame.push(service as u8);
    frame.extend_from_slice(&flow.as_u64().to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(payload);
    Ok(frame)
}

pub fn decode_datagram(frame: &[u8]) -> Result<(Service, FlowId, &[u8]), WireError> {
    if frame.len() < DATAGRAM_HEADER_LEN {
        return Err(WireError::Truncated);
    }
    if frame[0] != WIRE_VERSION {
        return Err(WireError::BadVersion(frame[0]));
    }
    let service = Service::try_from(frame[1]).map_err(WireError::UnknownService)?;
    let flow = FlowId::from_raw(u64::from_be_bytes(
        frame[2..10].try_into().expect("8 bytes"),
    ));
    Ok((service, flow, &frame[DATAGRAM_HEADER_LEN..]))
}

/// The stream-open frame. `intent`/`meta` are the service's to define.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    pub service: Service,
    pub flow: FlowId,
    pub intent: u8,
    pub meta: Vec<u8>,
}

pub async fn write_hello<W: AsyncWrite + Unpin>(
    writer: &mut W,
    hello: &Hello,
) -> Result<(), WireError> {
    if hello.meta.len() > MAX_HELLO_META {
        return Err(WireError::MetaTooLarge {
            len: hello.meta.len(),
        });
    }
    let len = HELLO_FIXED_LEN + hello.meta.len();
    let mut frame = Vec::with_capacity(2 + len);
    frame.extend_from_slice(&(len as u16).to_be_bytes());
    frame.push(WIRE_VERSION);
    frame.push(hello.service as u8);
    frame.extend_from_slice(&hello.flow.as_u64().to_be_bytes());
    frame.push(hello.intent);
    frame.extend_from_slice(&hello.meta);
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_hello<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Hello, WireError> {
    let mut len_bytes = [0u8; 2];
    reader.read_exact(&mut len_bytes).await?;
    let len = u16::from_be_bytes(len_bytes) as usize;
    if len < HELLO_FIXED_LEN {
        return Err(WireError::Truncated);
    }
    if len > HELLO_FIXED_LEN + MAX_HELLO_META {
        return Err(WireError::MetaTooLarge {
            len: len - HELLO_FIXED_LEN,
        });
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    if body[0] != WIRE_VERSION {
        return Err(WireError::BadVersion(body[0]));
    }
    let service = Service::try_from(body[1]).map_err(WireError::UnknownService)?;
    let flow = FlowId::from_raw(u64::from_be_bytes(body[2..10].try_into().expect("8 bytes")));
    Ok(Hello {
        service,
        flow,
        intent: body[10],
        meta: body[11..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagram_round_trip() {
        let flow = FlowId::derive(b"voice-channel:general");
        let frame = encode_datagram(Service::Voice, flow, b"opus bytes").unwrap();
        let (service, decoded_flow, payload) = decode_datagram(&frame).unwrap();
        assert_eq!(service, Service::Voice);
        assert_eq!(decoded_flow, flow);
        assert_eq!(payload, b"opus bytes");
    }

    #[test]
    fn datagram_rejects_oversize_and_garbage() {
        let flow = FlowId::from_raw(7);
        let big = vec![0u8; MAX_DATAGRAM_PAYLOAD + 1];
        assert!(matches!(
            encode_datagram(Service::Voice, flow, &big),
            Err(WireError::PayloadTooLarge { .. })
        ));
        assert!(matches!(
            decode_datagram(&[1, 2, 3]),
            Err(WireError::Truncated)
        ));
        let mut frame = encode_datagram(Service::Voice, flow, b"x").unwrap();
        frame[0] = 9;
        assert!(matches!(
            decode_datagram(&frame),
            Err(WireError::BadVersion(9))
        ));
        frame[0] = WIRE_VERSION;
        frame[1] = 250;
        assert!(matches!(
            decode_datagram(&frame),
            Err(WireError::UnknownService(250))
        ));
    }

    #[tokio::test]
    async fn hello_round_trip() {
        let hello = Hello {
            service: Service::StateSync,
            flow: FlowId::derive(b"snapshot:abc"),
            intent: 3,
            meta: b"range=0..100".to_vec(),
        };
        let (mut a, mut b) = tokio::io::duplex(4096);
        write_hello(&mut a, &hello).await.unwrap();
        let read = read_hello(&mut b).await.unwrap();
        assert_eq!(read, hello);
    }
}
