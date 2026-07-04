use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    BindRequest { from: NodeKey },
    BindResponse { reflexive: SocketAddr },
    Register { key: NodeKey },
    /// Rebind re-advertisement: republish the sender's reflexive under a
    /// strictly-monotonic `nonce`. Unlike `Register` (an unconditional nonce-0
    /// boot baseline), the coordinator applies the `AdvertBook` staleness guard
    /// so a replayed/reordered equal-or-lower nonce cannot roll a fresh mapping
    /// back. The reflexive stored is still the coordinator-observed source, never
    /// this datagram's self-report — the nonce only orders adverts.
    Readvertise { key: NodeKey, nonce: u64 },
    Lookup { key: NodeKey },
    LookupResponse { key: NodeKey, reflexive: Option<SocketAddr> },
    PunchSync { peer: NodeKey, peer_reflexive: SocketAddr },
    Punch { from: NodeKey },
    RelayRequest { peer: NodeKey },
    RelayGrant { session: u64, relay: SocketAddr },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("buffer too short")]
    Short,
    #[error("bad tag {0}")]
    BadTag(u8),
    #[error("bad address encoding")]
    BadAddr,
    #[error("trailing bytes after message")]
    Trailing,
}

const TAG_BIND_REQ: u8 = 1;
const TAG_BIND_RESP: u8 = 2;
const TAG_REGISTER: u8 = 3;
const TAG_LOOKUP: u8 = 4;
const TAG_LOOKUP_RESP: u8 = 5;
const TAG_PUNCH_SYNC: u8 = 6;
const TAG_PUNCH: u8 = 7;
const TAG_RELAY_REQ: u8 = 8;
const TAG_RELAY_GRANT: u8 = 9;
const TAG_READVERTISE: u8 = 10;

fn put_key(out: &mut Vec<u8>, k: &NodeKey) {
    out.extend_from_slice(&k.0);
}

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn put_addr(out: &mut Vec<u8>, a: &SocketAddr) {
    match a.ip() {
        IpAddr::V4(v4) => {
            out.push(4);
            out.extend_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            out.push(6);
            out.extend_from_slice(&v6.octets());
        }
    }
    out.extend_from_slice(&a.port().to_be_bytes());
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::Short)?;
        if end > self.buf.len() {
            return Err(WireError::Short);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn key(&mut self) -> Result<NodeKey, WireError> {
        let s = self.take(32)?;
        let mut k = [0u8; 32];
        k.copy_from_slice(s);
        Ok(NodeKey(k))
    }
    fn u64(&mut self) -> Result<u64, WireError> {
        let s = self.take(8)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(s);
        Ok(u64::from_be_bytes(b))
    }
    fn addr(&mut self) -> Result<SocketAddr, WireError> {
        let fam = self.take(1)?[0];
        let ip = match fam {
            4 => {
                let o = self.take(4)?;
                IpAddr::V4(Ipv4Addr::new(o[0], o[1], o[2], o[3]))
            }
            6 => {
                let o = self.take(16)?;
                let mut b = [0u8; 16];
                b.copy_from_slice(o);
                IpAddr::V6(Ipv6Addr::from(b))
            }
            _ => return Err(WireError::BadAddr),
        };
        let p = self.take(2)?;
        let port = u16::from_be_bytes([p[0], p[1]]);
        Ok(SocketAddr::new(ip, port))
    }
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(48);
        match self {
            Msg::BindRequest { from } => {
                out.push(TAG_BIND_REQ);
                put_key(&mut out, from);
            }
            Msg::BindResponse { reflexive } => {
                out.push(TAG_BIND_RESP);
                put_addr(&mut out, reflexive);
            }
            Msg::Register { key } => {
                out.push(TAG_REGISTER);
                put_key(&mut out, key);
            }
            Msg::Readvertise { key, nonce } => {
                out.push(TAG_READVERTISE);
                put_key(&mut out, key);
                put_u64(&mut out, *nonce);
            }
            Msg::Lookup { key } => {
                out.push(TAG_LOOKUP);
                put_key(&mut out, key);
            }
            Msg::LookupResponse { key, reflexive } => {
                out.push(TAG_LOOKUP_RESP);
                put_key(&mut out, key);
                match reflexive {
                    Some(a) => {
                        out.push(1);
                        put_addr(&mut out, a);
                    }
                    None => out.push(0),
                }
            }
            Msg::PunchSync { peer, peer_reflexive } => {
                out.push(TAG_PUNCH_SYNC);
                put_key(&mut out, peer);
                put_addr(&mut out, peer_reflexive);
            }
            Msg::Punch { from } => {
                out.push(TAG_PUNCH);
                put_key(&mut out, from);
            }
            Msg::RelayRequest { peer } => {
                out.push(TAG_RELAY_REQ);
                put_key(&mut out, peer);
            }
            Msg::RelayGrant { session, relay } => {
                out.push(TAG_RELAY_GRANT);
                put_u64(&mut out, *session);
                put_addr(&mut out, relay);
            }
        }
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Msg, WireError> {
        let mut r = Reader::new(buf);
        let tag = r.take(1)?[0];
        let msg = match tag {
            TAG_BIND_REQ => Msg::BindRequest { from: r.key()? },
            TAG_BIND_RESP => Msg::BindResponse { reflexive: r.addr()? },
            TAG_REGISTER => Msg::Register { key: r.key()? },
            TAG_READVERTISE => Msg::Readvertise { key: r.key()?, nonce: r.u64()? },
            TAG_LOOKUP => Msg::Lookup { key: r.key()? },
            TAG_LOOKUP_RESP => {
                let key = r.key()?;
                let present = r.take(1)?[0];
                let reflexive = match present {
                    0 => None,
                    1 => Some(r.addr()?),
                    _ => return Err(WireError::BadAddr),
                };
                Msg::LookupResponse { key, reflexive }
            }
            TAG_PUNCH_SYNC => Msg::PunchSync {
                peer: r.key()?,
                peer_reflexive: r.addr()?,
            },
            TAG_PUNCH => Msg::Punch { from: r.key()? },
            TAG_RELAY_REQ => Msg::RelayRequest { peer: r.key()? },
            TAG_RELAY_GRANT => Msg::RelayGrant { session: r.u64()?, relay: r.addr()? },
            other => return Err(WireError::BadTag(other)),
        };
        // Reject oversized/malformed datagrams that decode a valid prefix but
        // carry trailing garbage: a well-formed message consumes the whole
        // buffer, nothing more.
        if r.pos != buf.len() {
            return Err(WireError::Trailing);
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, o)), p)
    }

    #[test]
    fn every_variant_roundtrips() {
        let cases = vec![
            Msg::BindRequest { from: NodeKey([1u8; 32]) },
            Msg::BindResponse { reflexive: addr(2, 51820) },
            Msg::Register { key: NodeKey([3u8; 32]) },
            Msg::Readvertise { key: NodeKey([13u8; 32]), nonce: 0x0102_0304_dead_beef },
            Msg::Lookup { key: NodeKey([4u8; 32]) },
            Msg::LookupResponse { key: NodeKey([5u8; 32]), reflexive: Some(addr(6, 443)) },
            Msg::LookupResponse { key: NodeKey([7u8; 32]), reflexive: None },
            Msg::PunchSync { peer: NodeKey([8u8; 32]), peer_reflexive: addr(9, 7000) },
            Msg::Punch { from: NodeKey([10u8; 32]) },
            Msg::RelayRequest { peer: NodeKey([11u8; 32]) },
            Msg::RelayGrant { session: 0x0102_0304_0506_0708, relay: addr(12, 51820) },
        ];
        for m in cases {
            let bytes = m.encode();
            let back = Msg::decode(&bytes).expect("decode");
            assert_eq!(m, back);
        }
    }

    #[test]
    fn short_buffer_is_error() {
        assert_eq!(Msg::decode(&[]), Err(WireError::Short));
        assert_eq!(Msg::decode(&[0xff]), Err(WireError::BadTag(0xff)));
    }

    #[test]
    fn decode_rejects_trailing_garbage_bytes() {
        // A well-formed message with extra bytes appended (oversized /
        // malformed datagram) must be rejected outright, not silently
        // accepted by ignoring whatever the reader didn't consume.
        let mut bytes = Msg::BindRequest { from: NodeKey([9u8; 32]) }.encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));

        let mut bytes = Msg::PunchSync { peer: NodeKey([1u8; 32]), peer_reflexive: addr(2, 51820) }
            .encode();
        bytes.extend_from_slice(&[0, 0, 0]);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn readvertise_carries_key_and_nonce() {
        let m = Msg::Readvertise { key: NodeKey([0xab; 32]), nonce: 0xffff_0000_ffff_0001 };
        let back = Msg::decode(&m.encode()).expect("decode");
        assert_eq!(m, back);
        // Trailing garbage after a Readvertise is rejected like any other message.
        let mut bytes = m.encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn relay_grant_carries_session_and_addr() {
        let m = Msg::RelayGrant { session: 42, relay: addr(3, 4000) };
        let back = Msg::decode(&m.encode()).expect("decode");
        assert_eq!(m, back);
        // Trailing garbage after a RelayGrant is still rejected.
        let mut bytes = m.encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }
}
