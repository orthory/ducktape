use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use arrayvec::ArrayVec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    BindRequest {
        from: NodeKey,
    },
    BindResponse {
        reflexive: SocketAddr,
    },
    Register {
        key: NodeKey,
    },
    /// Rebind re-advertisement: republish the sender's reflexive under a
    /// strictly-monotonic `nonce`. Unlike `Register` (an unconditional nonce-0
    /// boot baseline), the coordinator applies the `AdvertBook` staleness guard
    /// so a replayed/reordered equal-or-lower nonce cannot roll a fresh mapping
    /// back. The reflexive stored is still the coordinator-observed source, never
    /// this datagram's self-report — the nonce only orders adverts.
    Readvertise {
        key: NodeKey,
        nonce: u64,
    },
    Lookup {
        key: NodeKey,
    },
    LookupResponse {
        key: NodeKey,
        reflexive: Option<SocketAddr>,
    },
    PunchSync {
        peer: NodeKey,
        peer_reflexive: SocketAddr,
    },
    Punch {
        from: NodeKey,
    },
    /// Publish a self-authenticating invite blob to the coordinator's shelf
    /// under its content id. Authenticated self-op (`key` == caller).
    InvitePut {
        key: NodeKey,
        id: [u8; INVITE_ID_LEN],
        expires_unix_secs: u64,
        blob: Vec<u8>,
    },
    /// The coordinator's answer to `InvitePut`. Node-directed.
    InvitePutAck {
        id: [u8; INVITE_ID_LEN],
        ok: bool,
    },
    /// Fetch one chunk of a shelved blob. `pad` zero bytes trail the datagram
    /// so the request is never smaller than the reply (anti-amplification).
    InviteGet {
        key: NodeKey,
        id: [u8; INVITE_ID_LEN],
        chunk: u16,
        pad: u16,
    },
    /// One chunk of a shelved blob (`total` == 0 means unknown/expired id).
    /// Node-directed.
    InviteChunk {
        id: [u8; INVITE_ID_LEN],
        chunk: u16,
        total: u16,
        bytes: Vec<u8>,
    },
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
    #[error("bad crypto encoding")]
    BadCrypto,
    #[error("auth envelope inner is not a request")]
    NotARequest,
    #[error("length field exceeds the protocol maximum")]
    TooLarge,
}

const TAG_BIND_REQ: u8 = 1;
const TAG_BIND_RESP: u8 = 2;
const TAG_REGISTER: u8 = 3;
const TAG_LOOKUP: u8 = 4;
const TAG_LOOKUP_RESP: u8 = 5;
const TAG_PUNCH_SYNC: u8 = 6;
const TAG_PUNCH: u8 = 7;
// Tags 8 and 9 carried the retired DERP-style relay messages
// (RelayRequest/RelayGrant). They stay reserved so a stale peer speaking the
// old protocol decodes as BadTag here instead of aliasing a future message.
const TAG_READVERTISE: u8 = 10;
const TAG_AUTH_REQUEST: u8 = 11;
// Short-invite shelf tags (PR2). 8/9 stay reserved (see above).
const TAG_INVITE_PUT: u8 = 12;
const TAG_INVITE_PUT_ACK: u8 = 13;
const TAG_INVITE_GET: u8 = 14;
const TAG_INVITE_CHUNK: u8 = 15;

/// Short-invite wire numerology — the single source of truth (`invite_store`
/// and the coordinator dispatch consume these).
/// content id = first 16 bytes of sha256(blob bytes).
pub const INVITE_ID_LEN: usize = 16;
/// hard cap on a shelved raw blob.
pub const INVITE_BLOB_MAX: usize = 8192;
/// bytes carried per `InviteChunk`.
pub const INVITE_CHUNK_BYTES: usize = 1000;
/// minimum zero-pad on an `InviteGet` datagram: the request is at least this
/// large, so a reply to a spoofed source never amplifies (reflection ≤ 1×).
pub const INVITE_GET_PAD: u16 = 1024;

fn put<const CAP: usize>(out: &mut ArrayVec<u8, CAP>, bytes: &[u8]) {
    out.try_extend_from_slice(bytes)
        .expect("wire buffer capacity covers every message");
}

fn put_key<const CAP: usize>(out: &mut ArrayVec<u8, CAP>, key: &NodeKey) {
    put(out, &key.0);
}

fn put_u64<const CAP: usize>(out: &mut ArrayVec<u8, CAP>, value: u64) {
    put(out, &value.to_be_bytes());
}

fn put_addr<const CAP: usize>(out: &mut ArrayVec<u8, CAP>, addr: &SocketAddr) {
    match addr.ip() {
        IpAddr::V4(ip) => {
            out.push(4);
            put(out, &ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6);
            put(out, &ip.octets());
        }
    }
    put(out, &addr.port().to_be_bytes());
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
    fn u16(&mut self) -> Result<u16, WireError> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn invite_id(&mut self) -> Result<[u8; INVITE_ID_LEN], WireError> {
        let s = self.take(INVITE_ID_LEN)?;
        let mut id = [0u8; INVITE_ID_LEN];
        id.copy_from_slice(s);
        Ok(id)
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
    fn sig(&mut self) -> Result<commonware_cryptography::ed25519::Signature, WireError> {
        use commonware_codec::DecodeExt as _;
        let s = self.take(64)?;
        commonware_cryptography::ed25519::Signature::decode(s).map_err(|_| WireError::BadCrypto)
    }
    fn pubkey(&mut self) -> Result<commonware_cryptography::ed25519::PublicKey, WireError> {
        use commonware_codec::DecodeExt as _;
        let s = self.take(32)?;
        commonware_cryptography::ed25519::PublicKey::decode(s).map_err(|_| WireError::BadCrypto)
    }
}

impl Msg {
    /// Largest encoded bare message — an `InvitePut` carrying a full-size blob.
    /// This fixed upper bound lets the hot UDP loop encode replies on its stack
    /// instead of allocating per datagram. The largest REPLY the coordinator
    /// ever sends is still small (an `InviteChunk`, ≤ 1023 B); the buffer is
    /// sized for the largest inbound request an `AuthRequest` can wrap.
    pub const MAX_ENCODED_LEN: usize = 1 + 32 + INVITE_ID_LEN + 8 + 2 + INVITE_BLOB_MAX;

    /// Encode into a stack-backed, fixed-capacity vector.
    pub fn encode_inline(&self) -> ArrayVec<u8, { Self::MAX_ENCODED_LEN }> {
        let mut out = ArrayVec::new();
        self.write(&mut out);
        out
    }

    pub fn encode(&self) -> Vec<u8> {
        self.encode_inline().into_iter().collect()
    }

    fn write<const CAP: usize>(&self, out: &mut ArrayVec<u8, CAP>) {
        match self {
            Msg::BindRequest { from } => {
                out.push(TAG_BIND_REQ);
                put_key(out, from);
            }
            Msg::BindResponse { reflexive } => {
                out.push(TAG_BIND_RESP);
                put_addr(out, reflexive);
            }
            Msg::Register { key } => {
                out.push(TAG_REGISTER);
                put_key(out, key);
            }
            Msg::Readvertise { key, nonce } => {
                out.push(TAG_READVERTISE);
                put_key(out, key);
                put_u64(out, *nonce);
            }
            Msg::Lookup { key } => {
                out.push(TAG_LOOKUP);
                put_key(out, key);
            }
            Msg::LookupResponse { key, reflexive } => {
                out.push(TAG_LOOKUP_RESP);
                put_key(out, key);
                match reflexive {
                    Some(a) => {
                        out.push(1);
                        put_addr(out, a);
                    }
                    None => out.push(0),
                }
            }
            Msg::PunchSync {
                peer,
                peer_reflexive,
            } => {
                out.push(TAG_PUNCH_SYNC);
                put_key(out, peer);
                put_addr(out, peer_reflexive);
            }
            Msg::Punch { from } => {
                out.push(TAG_PUNCH);
                put_key(out, from);
            }
            Msg::InvitePut {
                key,
                id,
                expires_unix_secs,
                blob,
            } => {
                out.push(TAG_INVITE_PUT);
                put_key(out, key);
                put(out, id);
                put_u64(out, *expires_unix_secs);
                put(out, &(blob.len() as u16).to_be_bytes());
                put(out, blob);
            }
            Msg::InvitePutAck { id, ok } => {
                out.push(TAG_INVITE_PUT_ACK);
                put(out, id);
                out.push(u8::from(*ok));
            }
            Msg::InviteGet {
                key,
                id,
                chunk,
                pad,
            } => {
                out.push(TAG_INVITE_GET);
                put_key(out, key);
                put(out, id);
                put(out, &chunk.to_be_bytes());
                put(out, &pad.to_be_bytes());
                // structural anti-amplification: trail `pad` zero bytes so the
                // datagram is at least as large as any reply. A pad past the
                // buffer capacity trips the same expect every other message does.
                const ZERO: [u8; 256] = [0u8; 256];
                let mut remaining = *pad as usize;
                while remaining > 0 {
                    let n = remaining.min(ZERO.len());
                    put(out, &ZERO[..n]);
                    remaining -= n;
                }
            }
            Msg::InviteChunk {
                id,
                chunk,
                total,
                bytes,
            } => {
                out.push(TAG_INVITE_CHUNK);
                put(out, id);
                put(out, &chunk.to_be_bytes());
                put(out, &total.to_be_bytes());
                put(out, &(bytes.len() as u16).to_be_bytes());
                put(out, bytes);
            }
        }
    }

    /// The claimed identity of a client→coordinator *request*, if this is one.
    pub fn subject_key(&self) -> Option<NodeKey> {
        match self {
            Msg::BindRequest { from } => Some(*from),
            Msg::Register { key }
            | Msg::Readvertise { key, .. }
            | Msg::Lookup { key }
            | Msg::InvitePut { key, .. }
            | Msg::InviteGet { key, .. } => Some(*key),
            _ => None,
        }
    }

    pub fn is_request(&self) -> bool {
        self.subject_key().is_some()
    }

    pub fn decode(buf: &[u8]) -> Result<Msg, WireError> {
        let mut r = Reader::new(buf);
        let msg = Msg::read(&mut r)?;
        // Reject oversized/malformed datagrams that decode a valid prefix but
        // carry trailing garbage: a well-formed message consumes the whole
        // buffer, nothing more.
        if r.pos != buf.len() {
            return Err(WireError::Trailing);
        }
        Ok(msg)
    }

    /// Read exactly one message (tag + body) from `r`, WITHOUT the
    /// whole-buffer check — used both by `decode` and the auth envelope.
    fn read(r: &mut Reader) -> Result<Msg, WireError> {
        let tag = r.take(1)?[0];
        let msg = match tag {
            TAG_BIND_REQ => Msg::BindRequest { from: r.key()? },
            TAG_BIND_RESP => Msg::BindResponse {
                reflexive: r.addr()?,
            },
            TAG_REGISTER => Msg::Register { key: r.key()? },
            TAG_READVERTISE => Msg::Readvertise {
                key: r.key()?,
                nonce: r.u64()?,
            },
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
            TAG_INVITE_PUT => {
                let key = r.key()?;
                let id = r.invite_id()?;
                let expires_unix_secs = r.u64()?;
                let blob_len = r.u16()? as usize;
                if blob_len > INVITE_BLOB_MAX {
                    return Err(WireError::TooLarge);
                }
                let blob = r.take(blob_len)?.to_vec();
                Msg::InvitePut {
                    key,
                    id,
                    expires_unix_secs,
                    blob,
                }
            }
            TAG_INVITE_PUT_ACK => {
                let id = r.invite_id()?;
                let ok = r.take(1)?[0] != 0;
                Msg::InvitePutAck { id, ok }
            }
            TAG_INVITE_GET => {
                let key = r.key()?;
                let id = r.invite_id()?;
                let chunk = r.u16()?;
                let pad = r.u16()?;
                // consume the pad zero bytes; only their SIZE matters (the
                // anti-amplification property), never their content.
                r.take(pad as usize)?;
                Msg::InviteGet {
                    key,
                    id,
                    chunk,
                    pad,
                }
            }
            TAG_INVITE_CHUNK => {
                let id = r.invite_id()?;
                let chunk = r.u16()?;
                let total = r.u16()?;
                let len = r.u16()? as usize;
                if len > INVITE_CHUNK_BYTES {
                    return Err(WireError::TooLarge);
                }
                let bytes = r.take(len)?.to_vec();
                Msg::InviteChunk {
                    id,
                    chunk,
                    total,
                    bytes,
                }
            }
            other => return Err(WireError::BadTag(other)),
        };
        Ok(msg)
    }
}

use crate::auth::{Authenticator, CoordCap};

/// An authenticated wrapper around one request `Msg`, carrying the per-request
/// authenticator. Wire tag 11. Only the four request shapes are wrappable.
///
/// `caller` is the authenticating identity — the key whose signer produced the
/// PoP. The coordinator authenticates THIS key, not the inner message's key:
/// for a `Lookup { key: peer }` the inner key is the peer being resolved, while
/// `caller` is the (different) node doing the resolving. Authenticating the
/// caller is what makes a cross-peer lookup possible; the inner key is only
/// cross-checked against `caller` for the self-ops (see the coordinator).
#[derive(Clone, Debug, PartialEq)]
pub struct AuthRequest {
    pub caller: NodeKey,
    pub inner: Msg,
    pub auth: Authenticator,
}

impl AuthRequest {
    /// Largest accepted envelope, including a capability and the largest bare
    /// `Msg`. Valid request inners are smaller; the broader bound also covers
    /// malformed response inners so they can still be encoded for rejection
    /// tests without an allocation fallback.
    pub const MAX_ENCODED_LEN: usize = 1 + 32 + Msg::MAX_ENCODED_LEN + 8 + 64 + 1 + 32 + 8 + 64;

    /// Encode into a stack-backed, fixed-capacity vector.
    pub fn encode_inline(&self) -> ArrayVec<u8, { Self::MAX_ENCODED_LEN }> {
        let mut out = ArrayVec::new();
        out.push(TAG_AUTH_REQUEST);
        put_key(&mut out, &self.caller);
        self.inner.write(&mut out);
        put_u64(&mut out, self.auth.timestamp);
        put(&mut out, self.auth.pop_sig.as_ref());
        match &self.auth.cap {
            None => out.push(0),
            Some(cap) => {
                out.push(1);
                put(&mut out, cap.issuer.as_ref());
                put_u64(&mut out, cap.not_after);
                put(&mut out, cap.issuer_sig.as_ref());
            }
        }
        out
    }

    pub fn encode(&self) -> Vec<u8> {
        self.encode_inline().into_iter().collect()
    }

    pub fn decode(buf: &[u8]) -> Result<AuthRequest, WireError> {
        let mut r = Reader::new(buf);
        let tag = r.take(1)?[0];
        if tag != TAG_AUTH_REQUEST {
            return Err(WireError::BadTag(tag));
        }
        let caller = r.key()?;
        let inner = Msg::read(&mut r)?;
        if !inner.is_request() {
            return Err(WireError::NotARequest);
        }
        let timestamp = r.u64()?;
        let pop_sig = r.sig()?;
        let cap = match r.take(1)?[0] {
            0 => None,
            1 => Some(CoordCap {
                issuer: r.pubkey()?,
                not_after: r.u64()?,
                issuer_sig: r.sig()?,
            }),
            _ => return Err(WireError::BadCrypto),
        };
        if r.pos != buf.len() {
            return Err(WireError::Trailing);
        }
        Ok(AuthRequest {
            caller,
            inner,
            auth: Authenticator {
                timestamp,
                pop_sig,
                cap,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, o)), p)
    }

    fn addr6(last: u16, port: u16) -> SocketAddr {
        SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, last)),
            port,
        )
    }

    #[test]
    fn every_variant_roundtrips() {
        let cases = vec![
            Msg::BindRequest {
                from: NodeKey([1u8; 32]),
            },
            Msg::BindResponse {
                reflexive: addr(2, 51820),
            },
            Msg::Register {
                key: NodeKey([3u8; 32]),
            },
            Msg::Readvertise {
                key: NodeKey([13u8; 32]),
                nonce: 0x0102_0304_dead_beef,
            },
            Msg::Lookup {
                key: NodeKey([4u8; 32]),
            },
            Msg::LookupResponse {
                key: NodeKey([5u8; 32]),
                reflexive: Some(addr6(6, 443)),
            },
            Msg::LookupResponse {
                key: NodeKey([7u8; 32]),
                reflexive: None,
            },
            Msg::PunchSync {
                peer: NodeKey([8u8; 32]),
                peer_reflexive: addr6(9, 7000),
            },
            Msg::Punch {
                from: NodeKey([10u8; 32]),
            },
            Msg::InvitePut {
                key: NodeKey([11u8; 32]),
                id: [0xcd; INVITE_ID_LEN],
                expires_unix_secs: 1_800_000_000,
                blob: vec![0xee; 1500],
            },
            Msg::InvitePutAck {
                id: [0xcd; INVITE_ID_LEN],
                ok: true,
            },
            Msg::InviteGet {
                key: NodeKey([12u8; 32]),
                id: [0xcd; INVITE_ID_LEN],
                chunk: 3,
                pad: INVITE_GET_PAD,
            },
            Msg::InviteChunk {
                id: [0xcd; INVITE_ID_LEN],
                chunk: 3,
                total: 9,
                bytes: vec![0xaa; INVITE_CHUNK_BYTES],
            },
        ];
        for m in cases {
            let bytes = m.encode();
            let inline = m.encode_inline();
            assert_eq!(&inline[..], bytes);
            let back = Msg::decode(&bytes).expect("decode");
            assert_eq!(m, back);
        }
    }

    #[test]
    fn invite_put_rejects_an_oversized_blob_and_get_pads_the_datagram() {
        // an InvitePut blob above INVITE_BLOB_MAX must not decode (buffer bound).
        let mut big = Msg::InvitePut {
            key: NodeKey([1u8; 32]),
            id: [0; INVITE_ID_LEN],
            expires_unix_secs: 0,
            blob: vec![0; INVITE_BLOB_MAX],
        }
        .encode();
        // grow the declared length past the cap by hand: tag(1)+key(32)+id(16)+expires(8) then len u16
        let len_at = 1 + 32 + INVITE_ID_LEN + 8;
        big[len_at..len_at + 2].copy_from_slice(&((INVITE_BLOB_MAX as u16) + 1).to_be_bytes());
        big.push(0);
        assert!(Msg::decode(&big).is_err());

        // an InviteGet's encoded datagram is AT LEAST pad bytes long — the
        // anti-amplification property is structural, not caller discipline.
        let get = Msg::InviteGet {
            key: NodeKey([2u8; 32]),
            id: [7; INVITE_ID_LEN],
            chunk: 0,
            pad: INVITE_GET_PAD,
        };
        assert!(get.encode().len() >= INVITE_GET_PAD as usize);
        assert_eq!(Msg::decode(&get.encode()).unwrap(), get);
    }

    #[test]
    fn short_buffer_is_error() {
        assert_eq!(Msg::decode(&[]), Err(WireError::Short));
        assert_eq!(Msg::decode(&[0xff]), Err(WireError::BadTag(0xff)));
    }

    #[test]
    fn retired_relay_tags_are_rejected_not_aliased() {
        // Tags 8/9 were the DERP-style RelayRequest/RelayGrant. A stale peer
        // still speaking the old protocol must get a clean BadTag, and the
        // tags must never be reassigned to a new message shape.
        let mut relay_req = vec![8u8];
        relay_req.extend_from_slice(&[0x11; 32]);
        assert_eq!(Msg::decode(&relay_req), Err(WireError::BadTag(8)));

        let mut relay_grant = vec![9u8];
        relay_grant.extend_from_slice(&42u64.to_be_bytes());
        relay_grant.push(4);
        relay_grant.extend_from_slice(&[192, 0, 2, 1]);
        relay_grant.extend_from_slice(&4000u16.to_be_bytes());
        assert_eq!(Msg::decode(&relay_grant), Err(WireError::BadTag(9)));
    }

    #[test]
    fn decode_rejects_trailing_garbage_bytes() {
        // A well-formed message with extra bytes appended (oversized /
        // malformed datagram) must be rejected outright, not silently
        // accepted by ignoring whatever the reader didn't consume.
        let mut bytes = Msg::BindRequest {
            from: NodeKey([9u8; 32]),
        }
        .encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));

        let mut bytes = Msg::PunchSync {
            peer: NodeKey([1u8; 32]),
            peer_reflexive: addr(2, 51820),
        }
        .encode();
        bytes.extend_from_slice(&[0, 0, 0]);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn readvertise_carries_key_and_nonce() {
        let m = Msg::Readvertise {
            key: NodeKey([0xab; 32]),
            nonce: 0xffff_0000_ffff_0001,
        };
        let back = Msg::decode(&m.encode()).expect("decode");
        assert_eq!(m, back);
        // Trailing garbage after a Readvertise is rejected like any other message.
        let mut bytes = m.encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }

    #[test]
    fn auth_request_roundtrips_for_every_request_shape() {
        use crate::auth::{mint_coord_cap, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let node = ed25519::PrivateKey::from_seed(1);
        let g = ed25519::PrivateKey::from_seed(2);
        let mut subject = [0u8; 32];
        subject.copy_from_slice(node.public_key().as_ref());
        let subject = NodeKey(subject);

        let inners = vec![
            Msg::BindRequest { from: subject },
            Msg::Register { key: subject },
            Msg::Readvertise {
                key: subject,
                nonce: 42,
            },
            Msg::Lookup {
                key: NodeKey([7u8; 32]),
            },
            Msg::InvitePut {
                key: subject,
                id: [0xcd; INVITE_ID_LEN],
                expires_unix_secs: 1_800_000_000,
                blob: vec![0xee; 1500],
            },
            Msg::InviteGet {
                key: subject,
                id: [0xcd; INVITE_ID_LEN],
                chunk: 3,
                pad: INVITE_GET_PAD,
            },
        ];
        for inner in inners {
            // With and without a cap.
            for cap in [None, Some(mint_coord_cap(&g, subject, 9_999_999))] {
                let auth = sign_authenticator(&node, &inner.encode(), 1234, cap);
                // caller is the authenticating identity — for a cross-peer
                // Lookup it deliberately differs from the inner key.
                let req = AuthRequest {
                    caller: subject,
                    inner: inner.clone(),
                    auth,
                };
                let bytes = req.encode();
                let inline = req.encode_inline();
                assert_eq!(&inline[..], bytes);
                let back = AuthRequest::decode(&bytes).expect("decode");
                assert_eq!(req, back);
            }
        }
    }

    #[test]
    fn auth_request_rejects_response_inner() {
        use crate::auth::sign_authenticator;
        use commonware_cryptography::{Signer as _, ed25519};
        let node = ed25519::PrivateKey::from_seed(1);
        // Hand-encode an envelope whose inner is a RESPONSE (LookupResponse).
        let inner = Msg::LookupResponse {
            key: NodeKey([1u8; 32]),
            reflexive: None,
        };
        let auth = sign_authenticator(&node, &inner.encode(), 1, None);
        let bytes = AuthRequest {
            caller: NodeKey([9u8; 32]),
            inner,
            auth,
        }
        .encode();
        assert_eq!(AuthRequest::decode(&bytes), Err(WireError::NotARequest));
    }

    #[test]
    fn auth_request_rejects_trailing_and_bare_msg_decode_rejects_tag_11() {
        use crate::auth::sign_authenticator;
        use commonware_cryptography::{Signer as _, ed25519};
        let node = ed25519::PrivateKey::from_seed(1);
        let inner = Msg::Register {
            key: NodeKey([2u8; 32]),
        };
        let auth = sign_authenticator(&node, &inner.encode(), 1, None);
        let mut bytes = AuthRequest {
            caller: NodeKey([2u8; 32]),
            inner,
            auth,
        }
        .encode();
        bytes.push(0xff);
        assert_eq!(AuthRequest::decode(&bytes), Err(WireError::Trailing));
        // A tag-11 envelope must NOT decode as a bare Msg.
        let clean = AuthRequest {
            caller: NodeKey([2u8; 32]),
            inner: Msg::Register {
                key: NodeKey([2u8; 32]),
            },
            auth: sign_authenticator(
                &node,
                &Msg::Register {
                    key: NodeKey([2u8; 32]),
                }
                .encode(),
                1,
                None,
            ),
        }
        .encode();
        assert_eq!(Msg::decode(&clean), Err(WireError::BadTag(11)));
    }
}
