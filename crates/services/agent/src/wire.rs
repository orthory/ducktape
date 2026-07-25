//! the node ↔ agent-daemon protocol: four commands down, four events up.
//!
//! Both directions ride the ONE ws connection the daemon dials at
//! `/v1/ws` — the same localhost surface the CLI uses. The daemon always
//! dials; the node never dials the daemon. That is what keeps a service plug
//! unprivileged: it needs no listening port, no address in the node's config,
//! and a node that has never heard from it simply has no interactive plane.
//!
//! ## why the session id is minted by the NODE
//!
//! The node owns the id because the id is also the correlation token: it names
//! the ws topic (`term:<id>`) subscribers are already attached to, it keys the
//! node's per-session metadata, and it lets an output frame that races the
//! create reply still find its session. A daemon-minted id would need a second
//! correlation field and a window where output has nowhere to land.
//!
//! ## what does NOT cross this boundary
//!
//! No key material and no credential secret. [`Credential`] is the *resolved
//! record* — a name, a duckdns authority, the local browser-gateway `via`, a
//! PUBLIC seal key and the account pubkey the grant is checked against. The
//! secret itself never leaves the lender's airlock; the daemon reconstructs a
//! `provider_host::AirlockConfig` from these public facts and the broker dials
//! for a sealed session exactly as the in-node path did.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// node → daemon. One variant per thing the node can ask of a pty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    /// spawn a session under the node-minted `session` id.
    TermCreate(Create),
    /// write raw bytes to a live session's pty. `data_b64` is base64 of the
    /// bytes — never the bytes themselves, and never logged either way.
    TermInput { session: String, data_b64: String },
    /// a window-size change on a live session.
    TermResize {
        session: String,
        cols: u16,
        rows: u16,
    },
    /// end a session now. Idempotent; an unknown id is a no-op.
    TermClose { session: String },
}

/// everything the daemon needs to spawn one session. The node has already
/// decided admission (who may create, with whose credential, at what limits);
/// this is the decision's output, not its input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Create {
    /// the node-minted session id — also the workdir name and the ws topic.
    pub session: String,
    /// the provider tag to resolve (`claude`, `codex`, a test provider).
    pub provider: String,
    /// `true` runs the restricted (read-only, non-prompting) argv — the shared
    /// session's command-lane shape. `false` is the full solo TUI.
    pub restricted: bool,
    /// cpu/mem ceilings for the sandbox. Empty = the provider's defaults.
    #[serde(default)]
    pub limits: BTreeMap<String, u64>,
    /// a lent credential resolved from committed state, or `None` for a session
    /// running on the operator's own locally-resolved credential.
    #[serde(default)]
    pub credential: Option<Credential>,
}

/// a consensus-resolved credential record, in transit to the daemon's broker.
/// A field-for-field mirror of `provider_host::ResolvedCredential` — mirrored
/// rather than re-exported so this protocol stays a stable, inspectable shape
/// that a third-party plug can implement without linking provider-host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credential {
    pub name: String,
    pub kind: CredentialKind,
    /// the owner's duckdns handle (`airlock.<handle>.duck`).
    pub authority: String,
    /// the local node's browser-gateway base the request routes through.
    pub via: String,
    /// the owner's PUBLIC seal key, pinned as the broker's trust anchor.
    pub seal_pk: [u8; 32],
    /// the account the run acts on behalf of — checked by the owner's gateway.
    pub account: Vec<u8>,
}

/// which vendor a credential is for. Mirrors `provider_host::CredentialKind`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    Claude,
    Codex,
}

/// daemon → node. The lifecycle of a session, as the process that owns it sees
/// it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Event {
    /// the pty is live. Answers exactly one [`Command::TermCreate`].
    TermCreated { session: String },
    /// the create failed, for one nameable reason. Answers exactly one
    /// [`Command::TermCreate`].
    TermRefused {
        session: String,
        reason: Refusal,
        detail: String,
    },
    /// one chunk of pty output, base64. Ordered with respect to every other
    /// frame on this connection — which is what makes [`Event::TermEnded`] a
    /// true terminator rather than a race.
    TermOutput { session: String, chunk_b64: String },
    /// the session is over: the child exited, an explicit close landed, or the
    /// wall-clock ceiling fired. Exactly one per created session, emitted by
    /// whichever path actually removed it, so a close racing an EOF cannot
    /// double-terminate.
    TermEnded { session: String },
}

/// why a create refused. A stable snake_case token on the wire: the node maps
/// it to an HTTP status and the mesh maps it to a refusal reason, so the
/// 503-vs-`spawn_failed` diagnosis ladder keeps its rungs across the process
/// boundary.
///
/// There is deliberately no `no_sandbox` variant. A daemon with no runnable
/// sandbox refuses to start at all, so "no sandbox" can only ever mean "no
/// agent daemon is attached to this node" — a fact only the node can observe,
/// and which it answers without asking anyone.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// the daemon's concurrent-session cap is reached.
    AtCapacity,
    /// no provider in this daemon's set serves the requested tag.
    UnknownProvider,
    /// the interactive spawn itself failed (image absent, podman error, …).
    SpawnFailed,
}

impl Refusal {
    /// the stable token — what the mesh refusal reason and the logs carry.
    pub fn token(self) -> &'static str {
        match self {
            Refusal::AtCapacity => "at_capacity",
            Refusal::UnknownProvider => "unknown_provider",
            Refusal::SpawnFailed => "spawn_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refusal_tokens_are_the_ones_the_mesh_already_publishes() {
        // these three strings are a wire contract: `term_plane`'s guest surfaces
        // them verbatim as `host refused: <reason>: <detail>`, and the pty CLI's
        // diagnosis ladder reads them. Renaming one is a wire change.
        assert_eq!(Refusal::AtCapacity.token(), "at_capacity");
        assert_eq!(Refusal::UnknownProvider.token(), "unknown_provider");
        assert_eq!(Refusal::SpawnFailed.token(), "spawn_failed");
    }

    #[test]
    fn a_command_round_trips_through_json() {
        let create = Command::TermCreate(Create {
            session: "abc".into(),
            provider: "claude".into(),
            restricted: true,
            limits: BTreeMap::from([("cpu".to_string(), 2)]),
            credential: None,
        });
        let text = serde_json::to_string(&create).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&text).unwrap(), create);
        // the tag is what the node's ws writer and a third-party plug agree on.
        assert!(text.contains(r#""op":"term_create""#), "{text}");
    }

    #[test]
    fn an_event_round_trips_through_json() {
        let ended = Event::TermEnded {
            session: "abc".into(),
        };
        let text = serde_json::to_string(&ended).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&text).unwrap(), ended);
        assert!(text.contains(r#""op":"term_ended""#), "{text}");
    }

    #[test]
    fn a_create_without_optional_fields_decodes() {
        // a third-party plug must be able to omit what it does not use; the
        // node's own encoder always writes them.
        let create: Create =
            serde_json::from_str(r#"{"session":"a","provider":"claude","restricted":false}"#)
                .unwrap();
        assert!(create.limits.is_empty());
        assert!(create.credential.is_none());
    }
}
