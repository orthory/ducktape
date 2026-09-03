//! the guest-side remote-session channel (data-plane-free), mirroring
//! [`crate::GatewayJob`]/[`crate::GatewayLane`].
//!
//! A guest node that directs a session on a HOST peer never touches the mesh
//! from the daemon: it hands a [`SessionJob`] onto the [`SessionLane`] and the
//! overlay client half in `bin/node`'s `term_plane` drives the peer stream. The
//! job carries plain data plus a oneshot the client half resolves — exactly the
//! `GatewayJob` shape — so this crate stays free of any data-plane dependency.

use std::collections::HashMap;
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

/// guest-side registry: session id → the host node that owns its pty. Set when a
/// remote create returns; read by the ws input handler to pick the forward lane
/// over the (absent) local session. `Arc<Mutex<..>>` like the gateway's ws-token
/// store.
#[derive(Clone, Default)]
pub struct RemoteSessions(Arc<Mutex<HashMap<String, [u8; 32]>>>);

impl RemoteSessions {
    /// remember that `session` lives on `host` (a remote create returned).
    pub fn remember(&self, session: String, host: [u8; 32]) {
        self.0
            .lock()
            .expect("remote sessions lock poisoned")
            .insert(session, host);
    }

    /// the host that owns `session`, or `None` for a local (non-remote) session.
    pub fn host_of(&self, session: &str) -> Option<[u8; 32]> {
        self.0
            .lock()
            .expect("remote sessions lock poisoned")
            .get(session)
            .copied()
    }

    /// drop the binding on close.
    pub fn forget(&self, session: &str) {
        self.0
            .lock()
            .expect("remote sessions lock poisoned")
            .remove(session);
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
}
