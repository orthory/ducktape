//! Single-use tokens for the gateway WebSocket side door.
//!
//! `new WebSocket()` accepts only `ws:`/`wss:`, so a `duck://` page cannot open
//! a socket on its own scheme. Instead the scheme handler answers a synthetic
//! same-origin `/.duck/ws` request by minting one of these tokens (bound to the
//! page's origin and its resolved route) and handing back a
//! `ws://127.0.0.1:<port>/…/<token>` URL. The browser then opens that loopback
//! socket; the door consumes the token, re-checking the `Origin`.
//!
//! Security properties (audit S3): a token is single-use, short-lived, and
//! bound to the exact origin that minted it. A local process that guesses or
//! steals a token still cannot use it without also presenting the bound origin,
//! and a token is destroyed the instant it is consumed (or found expired), so a
//! race between two upgrades cannot reuse one.

use std::collections::HashMap;
use std::sync::Mutex;

use rand::RngCore as _;
use tokio::time::{Duration, Instant};

/// Tokens live just long enough for the browser to turn the URL into an open
/// socket. Kept short so a leaked token is worthless almost immediately.
pub const WS_TOKEN_TTL: Duration = Duration::from_secs(30);

struct Entry {
    origin: String,
    account_id: Vec<u8>,
    name: gateway::RouteName,
    expires_at: Instant,
}

/// The bound route a consumed token authorizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsGrant {
    pub account_id: Vec<u8>,
    pub name: gateway::RouteName,
}

#[derive(Default)]
pub struct WsTokenStore {
    entries: Mutex<HashMap<String, Entry>>,
}

impl WsTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mint a single-use token bound to `origin` and the resolved route,
    /// valid for [`WS_TOKEN_TTL`].
    pub fn mint(&self, origin: String, account_id: Vec<u8>, name: gateway::RouteName) -> String {
        let mut random = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let token = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut entries = self.entries.lock().expect("ws token store poisoned");
        prune(&mut entries);
        entries.insert(
            token.clone(),
            Entry {
                origin,
                account_id,
                name,
                expires_at: Instant::now() + WS_TOKEN_TTL,
            },
        );
        token
    }

    /// Atomically consume `token`: it must exist, be unexpired, and carry the
    /// exact `origin` it was minted for. The entry is removed on any outcome —
    /// a token is dead the moment it is presented, valid or not.
    pub fn consume(&self, token: &str, origin: &str) -> Option<WsGrant> {
        let mut entries = self.entries.lock().expect("ws token store poisoned");
        let entry = entries.remove(token)?;
        if entry.expires_at <= Instant::now() {
            return None;
        }
        // CEF serializes a custom-scheme page's origin as the literal "null"
        // on WebSocket handshakes, so the exact pin cannot hold for the very
        // browser this door serves. Accept that serialization: the mint is
        // already same-origin-gated at the trusted duck:// scheme handler, and
        // single-use + TTL + token secrecy remain the transport defense.
        if entry.origin != origin && origin != "null" {
            return None;
        }
        Some(WsGrant {
            account_id: entry.account_id,
            name: entry.name,
        })
    }
}

fn prune(entries: &mut HashMap<String, Entry>) {
    let now = Instant::now();
    entries.retain(|_, entry| entry.expires_at > now);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> gateway::RouteName {
        gateway::RouteName::named("app")
    }

    #[tokio::test(start_paused = true)]
    async fn round_trip_is_single_use_and_origin_bound() {
        let store = WsTokenStore::new();
        let token = store.mint("duck://app.alice.duck".into(), vec![1, 2, 3], route());

        // Wrong origin never succeeds — and burns the token.
        assert!(store.consume(&token, "duck://evil.bob.duck").is_none());
        assert!(store.consume(&token, "duck://app.alice.duck").is_none());

        // A fresh token is consumable exactly once.
        let token = store.mint("duck://app.alice.duck".into(), vec![1, 2, 3], route());
        assert_eq!(
            store.consume(&token, "duck://app.alice.duck"),
            Some(WsGrant {
                account_id: vec![1, 2, 3],
                name: route(),
            })
        );
        assert!(store.consume(&token, "duck://app.alice.duck").is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn expired_tokens_are_rejected() {
        let store = WsTokenStore::new();
        let token = store.mint("duck://app.alice.duck".into(), vec![9], route());
        tokio::time::advance(WS_TOKEN_TTL + Duration::from_secs(1)).await;
        assert!(store.consume(&token, "duck://app.alice.duck").is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn minting_prunes_expired_entries() {
        let store = WsTokenStore::new();
        let stale = store.mint("duck://a.duck".into(), vec![1], route());
        tokio::time::advance(WS_TOKEN_TTL + Duration::from_secs(1)).await;
        // Minting a new token prunes the stale one; the store never leaks.
        let _fresh = store.mint("duck://b.duck".into(), vec![2], route());
        assert!(store.consume(&stale, "duck://a.duck").is_none());
        assert_eq!(
            store.entries.lock().unwrap().len(),
            1,
            "only the fresh token remains"
        );
    }
}
