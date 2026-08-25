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
///
/// `deny_unknown_fields`, like every type on this boundary: there is no live
/// network and no compat obligation, so a frame carrying a field this build
/// does not know is a SKEW REPORT, not something to tolerate. Tolerating it
/// silently drops whatever the other side thought it was saying — and on
/// [`Create`] that would mean running a session without a restriction the
/// sender believed it had imposed.
///
/// What the daemon DOES with a refused decode differs by direction, and only
/// one direction is finished. An [`Event`] the node cannot decode is answered
/// with a `BadFrame` naming the field. A [`Command`] the DAEMON cannot decode
/// is only dropped, so a skewed `TermCreate` leaves the node's create waiting
/// — see `classify` in `bin/node/src/agent/link.rs` for why that is left, and
/// what closing it looks like.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
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
///
/// EVERY field is required. Nothing here defaults, deliberately: each of the
/// two that used to would have failed OPEN if a sender omitted it — an absent
/// `credential` reads as "run on the operator's own locally-resolved
/// credential", and absent `limits` as "the provider's defaults" — so a skewed
/// sender's silence would have widened this session's authority or its
/// resource ceiling. A decision's output must be stated, not inferred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Create {
    /// the node-minted session id — also the workdir name and the ws topic.
    pub session: String,
    /// the provider tag to resolve (`claude`, `codex`, a test provider).
    pub provider: String,
    /// `true` runs the restricted (read-only, non-prompting) argv — the shared
    /// session's command-lane shape. `false` is the full solo TUI.
    pub restricted: bool,
    /// cpu/mem ceilings for the sandbox. An EMPTY map means the provider's
    /// defaults — and must be written as one, never omitted.
    pub limits: BTreeMap<String, u64>,
    /// a lent credential resolved from committed state, or `null` for a session
    /// running on the operator's own locally-resolved credential. `null` is a
    /// statement the sender makes; it is not what silence means.
    ///
    /// `deserialize_with` is what makes that true. Serde lets an `Option` field
    /// go MISSING even with no `#[serde(default)]` — absent decodes to `None`,
    /// which here is the fail-open "use the operator's own credential". Naming
    /// a deserializer suppresses that fallback, so an omitted field is a
    /// `missing field` error like every other.
    #[serde(deserialize_with = "Option::deserialize")]
    pub credential: Option<Credential>,
}

/// a consensus-resolved credential record, in transit to the daemon's broker.
/// A field-for-field mirror of `provider_host::ResolvedCredential` — mirrored
/// rather than re-exported so this protocol stays a stable, inspectable shape
/// that a third-party plug can implement without linking provider-host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub name: String,
    pub kind: CredentialKind,
    /// the owner's duckdns handle (`airlock.<handle>.duck`).
    pub authority: String,
    /// the local node's browser-gateway base the request routes through.
    pub via: String,
    /// the owner's PUBLIC seal key, pinned as the broker's trust anchor.
    pub seal_pk: [u8; 32],
}

/// which vendor a credential is for — the one airlock vocabulary, serialized
/// snake_case on this wire exactly as the lender serializes it.
pub use provider_host::CredentialKind;

/// daemon → node. The lifecycle of a session, as the process that owns it sees
/// it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
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
    /// the interactive spawn itself failed (guest artifacts absent, no
    /// `/dev/kvm`, the guest never dialled back, …).
    SpawnFailed,
}

/// a session id is 16 lowercase hex — the shape the node mints. Checked on
/// arrival at the daemon, because the id becomes a directory name there, and by
/// the mesh term plane before a grain reaches a ring. Lives here rather than in
/// either consumer: the id is this protocol's, so its validity rule is too.
pub fn valid_session(session: &str) -> bool {
    session.len() == 16
        && session
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase() && byte.is_ascii_hexdigit())
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
    fn a_session_id_is_sixteen_lowercase_hex() {
        assert!(valid_session("0123456789abcdef"));
        // the id becomes a directory name on the daemon: nothing that could
        // walk out of the workdir root may pass.
        for bad in [
            "",
            "0123456789abcde",       // short
            "0123456789abcdef0",     // long
            "0123456789ABCDEF",      // upper
            "../../etc/passwd",
            "0123456789abcde/",
        ] {
            assert!(!valid_session(bad), "must reject {bad:?}");
        }
    }

    /// Every skew this protocol can see is a NAMED decode error. There is no
    /// live network, so nothing here may be tolerant: an unknown field and an
    /// absent field both stop the frame instead of quietly becoming a default.
    ///
    /// This is the whole justification the build gate's deletion rests on — the
    /// gate was a whole-connection version check standing in for per-frame
    /// decoding, and per-frame decoding only replaces it if it actually
    /// refuses. Before this, an added field decoded `Ok` and was dropped: a
    /// spend cap added to [`Credential`] would have been silently discarded and
    /// the session run without it.
    #[test]
    fn skew_in_either_direction_is_a_named_decode_error() {
        let full = r#"{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{},"credential":null}"#;
        serde_json::from_str::<Command>(full).expect("the current shape decodes");

        // a NEWER sender's extra field — on the command, on the create, and on
        // a lent credential (where dropping one is dropping a restriction).
        //
        // `cred` must be EXACTLY the current shape and nothing more. It once
        // carried a since-deleted `account` field, and serde reports the FIRST
        // unknown field it meets: the assertion below went on passing while the
        // `spend_cap` arm — the one this test exists for — stopped running
        // entirely. Deleting `"spend_cap":10` from the composed string is the
        // check that it still does.
        let cred = r#""credential":{"name":"n","kind":"claude","authority":"a","via":"v","seal_pk":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]"#;
        for newer in [
            r#"{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{},"credential":null,"future":1}"#.to_string(),
            r#"{"op":"term_close","session":"a","force":true}"#.to_string(),
            format!(r#"{{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{{}},{cred},"spend_cap":10}}}}"#),
        ] {
            let error = serde_json::from_str::<Command>(&newer)
                .expect_err(&format!("an unknown field must refuse: {newer}"));
            assert!(error.to_string().contains("unknown field"), "{error}");
        }

        // an OLDER sender's omission. Both of these used to decode to a
        // fail-open default: no credential (the operator's own), no limits.
        for older in [
            r#"{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{}}"#,
            r#"{"op":"term_create","session":"a","provider":"claude","restricted":false,"credential":null}"#,
        ] {
            let error = serde_json::from_str::<Command>(older)
                .expect_err(&format!("a missing field must refuse: {older}"));
            assert!(error.to_string().contains("missing field"), "{error}");
        }

        // and the daemon → node direction, same rule.
        let event = serde_json::from_str::<Event>(r#"{"op":"term_ended","session":"a","code":0}"#)
            .expect_err("an unknown field must refuse on an event too");
        assert!(event.to_string().contains("unknown field"), "{event}");
    }
}
