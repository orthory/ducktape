//! the guest-side remote-session channel (data-plane-free), mirroring
//! [`crate::GatewayJob`]/[`crate::GatewayLane`].
//!
//! A guest node that directs a session on a HOST peer never touches the mesh
//! from the daemon: it hands a [`SessionJob`] onto the [`SessionLane`] and the
//! overlay client half in `bin/node`'s `term_plane` drives the peer stream. The
//! job carries plain data plus a oneshot the client half resolves — exactly the
//! `GatewayJob` shape — so this crate stays free of any data-plane dependency.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::term::CreatedSession;

/// the daemon-side twin of `term_plane`'s `SessionInputEvent` (kept here so
/// noded carries no data-plane dep; `term_plane` maps one to the other 1:1).
pub enum SessionInputWire {
    Input { session: String, data_b64: String },
    Resize { session: String, cols: u16, rows: u16 },
}

/// a unit of remote-session work the guest node hands its overlay client half.
pub enum SessionJob {
    Create {
        host: [u8; 32],
        provider: String,
        cred: String,
        cpu: Option<u64>,
        mem_gb: Option<u64>,
        reply: oneshot::Sender<Result<CreatedSession, String>>,
    },
    Close {
        host: [u8; 32],
        session: String,
    },
    Input {
        host: [u8; 32],
        event: SessionInputWire,
    },
}

pub type SessionLane = mpsc::Sender<SessionJob>;

/// how many MIRRORED sessions a node keeps a first-sender host binding for.
///
/// `ponytail:` a flat FIFO cap. Every forged grain naming an id this node has
/// never seen mints one binding, so the map needs a ceiling; the oldest goes
/// first, and a session whose binding was evicted simply re-binds on its host's
/// next grain. Make it an LRU keyed to the ring's own session set if a node ever
/// mirrors more sessions than this at once.
const MAX_OBSERVED_SESSIONS: usize = 64;

/// guest-side registry: session id → the node that hosts its pty.
///
/// Two ways a node learns that, kept apart on purpose:
/// - [`Self::remember`] — THIS node directed the create, so the host is known
///   outright. Only these are a forward lane: [`Self::host_of`] is what the ws
///   input/resize/close handlers read to send input to the host.
/// - [`Self::feed_host`] — this node merely MIRRORS the session (a Shared
///   session hosted anywhere on the mesh fans out to every peer, which is what
///   makes it watchable like a huddle). Nobody told this node who hosts it, so
///   the FIRST peer to deliver a grain for the id is bound to it and every other
///   peer is refused from then on. A session id is 16 hex of randomness the host
///   mints, so a third party cannot pre-claim an id before the host's own grains
///   start arriving.
///
/// `Arc<Mutex<..>>` like the gateway's ws-token store.
#[derive(Clone, Default)]
pub struct RemoteSessions(Arc<Mutex<Bindings>>);

#[derive(Default)]
struct Bindings {
    /// sessions this node created on a host — the forward lane.
    created: HashMap<String, [u8; 32]>,
    /// sessions this node only mirrors, bound to their first sender.
    observed: HashMap<String, [u8; 32]>,
    /// `observed` insertion order, for the FIFO cap.
    seen: VecDeque<String>,
}

impl RemoteSessions {
    /// remember that `session` lives on `host` (a remote create returned).
    pub fn remember(&self, session: String, host: [u8; 32]) {
        self.lock().created.insert(session, host);
    }

    /// the host this node DIRECTED the session to, or `None` for a local session
    /// or one this node only mirrors. The forward lane's question, so a mirrored
    /// session deliberately answers `None`: observing someone else's session
    /// confers no right to type into it or close it.
    pub fn host_of(&self, session: &str) -> Option<[u8; 32]> {
        self.lock().created.get(session).copied()
    }

    /// the node whose grains this session accepts, binding `sender` to it if
    /// nothing is bound yet. A directed create's host always wins; otherwise the
    /// first sender claims the id and holds it until [`Self::forget`].
    pub fn feed_host(&self, session: &str, sender: [u8; 32]) -> [u8; 32] {
        let mut bindings = self.lock();
        if let Some(host) = bindings.created.get(session) {
            return *host;
        }
        if let Some(host) = bindings.observed.get(session) {
            return *host;
        }
        while bindings.observed.len() >= MAX_OBSERVED_SESSIONS {
            let Some(oldest) = bindings.seen.pop_front() else {
                break;
            };
            bindings.observed.remove(&oldest);
        }
        bindings.observed.insert(session.to_string(), sender);
        bindings.seen.push_back(session.to_string());
        sender
    }

    /// drop the binding: the session closed, or its host said it ended.
    pub fn forget(&self, session: &str) {
        let mut bindings = self.lock();
        bindings.created.remove(session);
        bindings.observed.remove(session);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Bindings> {
        self.0.lock().expect("remote sessions lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_sessions_remembers_and_forgets_a_binding() {
        let sessions = RemoteSessions::default();
        let host = [7u8; 32];
        assert!(sessions.host_of("00000000deadbeef").is_none());
        sessions.remember("00000000deadbeef".into(), host);
        assert_eq!(sessions.host_of("00000000deadbeef"), Some(host));
        // a different id is still unknown.
        assert!(sessions.host_of("00000000cafef00d").is_none());
        sessions.forget("00000000deadbeef");
        assert!(sessions.host_of("00000000deadbeef").is_none());
    }

    #[test]
    fn a_mirrored_session_binds_to_its_first_sender_and_a_known_host_outranks_it() {
        let sessions = RemoteSessions::default();
        let a = [1u8; 32];
        let b = [2u8; 32];
        // an id nobody told this node about: the first peer to deliver a grain
        // for it IS its host from then on...
        assert_eq!(sessions.feed_host("00000000deadbeef", a), a);
        // ...so a second peer naming the same id gets A back, not itself — which
        // is how the feed gate refuses it.
        assert_eq!(sessions.feed_host("00000000deadbeef", b), a);
        // a directed create's host wins over any first sender: B claims the id
        // first, then this node learns it created the session on A.
        assert_eq!(sessions.feed_host("00000000cafef00d", b), b);
        sessions.remember("00000000cafef00d".into(), a);
        assert_eq!(sessions.feed_host("00000000cafef00d", b), a);
        // and the id is free again once the session is done.
        sessions.forget("00000000deadbeef");
        assert_eq!(sessions.feed_host("00000000deadbeef", b), b);
    }

    #[test]
    fn observed_bindings_are_capped_and_evict_oldest_first() {
        let sessions = RemoteSessions::default();
        let attacker = [9u8; 32];
        let host = [1u8; 32];
        assert_eq!(sessions.feed_host("00000000deadbeef", host), host);
        for i in 0..MAX_OBSERVED_SESSIONS {
            assert_eq!(sessions.feed_host(&format!("{i:016x}"), attacker), attacker);
        }
        // the oldest binding aged out, so the id re-binds to whoever sends next
        // — the real host, whose grains never stopped.
        assert_eq!(sessions.feed_host("00000000deadbeef", host), host);
    }
}
