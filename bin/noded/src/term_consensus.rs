//! PR2 — the CONSENSUS command source for shared terminal sessions.
//!
//! PR1 gave a `Shared` session an ordered, origin-attributed command lane fed by
//! a node-local ws op (`ClientMsg::TermCommand` -> [`crate::term::TerminalSessions::enqueue_command`]).
//! That lane trusts a caller-supplied `origin` string and orders commands only
//! within one node. PR2 makes the command source CONSENSUS: a member submits a
//! command as an origin-SIGNED, totally-ORDERED, durable chat message; consensus
//! signs + orders + persists it for free; and the node that OWNS the pty projects
//! the committed messages into it, in sequence, attributed to the cryptographically
//! verified author.
//!
//! ## Why chat, and why no new module
//!
//! A new consensus module changes the genesis root-hash — a flag day that kills
//! every existing network. So we reuse the CHAT module, already in genesis and
//! already an ordered, origin-signed, durable per-channel append log: a shared
//! session's command lane IS a dedicated chat channel, a submitted command IS a
//! chat message. This mirrors forge's hidden `forge:<repo>:<n>` discussion
//! channels (`crates/modules/apps/forge/src/tracker_iface.rs`).
//!
//! ## Channel scheme (see the resolution note on [`session_channel`])
//!
//! `term-<session_id>`, a NON-colon id, created by the host node under its own
//! key (an external `User` origin) with an OPEN post policy. A colon id
//! (`term:<id>`) would be self-hiding — chat reserves colon ids to
//! module/system origins and the app's `isModuleChannel` hides them — but a
//! RUNNING node cannot mint one: `NodeCommand::Submit` signs with the node key
//! (a `User` origin) on `bin/node` (see `bin/node/.../ingress.rs on_http`), and
//! chat's `validate_channel_namespace` forbids a `User` origin any colon id.
//! System origin is genesis/catch-up only, unreachable from a live submit lane.
//! So the node creates a non-colon channel it CAN author, `PostPolicy::Open`
//! lets any member post commands (external `User` authors pass the open policy;
//! `validate_channel_namespace` is not consulted on `PostMessage`), and the app
//! hides it via an `isModuleChannel` extension that also matches `term-<hex>`.
//!
//! ## Authorization posture (deferred ACL — read this before trusting it)
//!
//! `PostPolicy::Open` means ANY network member can post a command the projector
//! feeds into the pty — NOT only the session's participants, and NOT only those
//! "who know the id". The channel id is not a secret: `term-<id>` lands in
//! committed chat state and is enumerable via `ChatQuery::Channels`; the
//! `isModuleChannel` hide is UI-only. What contains the blast radius is the
//! session posture, not the channel: a Shared session runs the RESTRICTED,
//! read-only, non-prompting argv, the pty is Podman-sandboxed under the node
//! identity, and the credential never enters the container — so an injected
//! command can only spend tokens + inject conversation text, never write/exec.
//! The NAMED next step is per-channel membership (`PostPolicy::MembersOnly`)
//! once a shared session carries an on-chain participant set; per-member spend
//! caps are the epic's finding #2. Until then, treat a shared session as
//! open-to-the-network by construction.
//!
//! ## Why a projector, not a `host::worker::Worker`
//!
//! The obvious shape — a worker that decodes a committed `ChatEvent::MessagePosted`
//! event — does not work: chat emits `MessagePosted` via `ctx.emit_msg` to
//! registered HOOK MODULES (a within-block dispatch), never as an `sdk::Event`
//! on the worker seam (`crates/modules/apps/chat/src/lib.rs` has NO `emit_event`). A
//! channel with no hooks emits nothing to observe, and a hook target must be a
//! real registered module — which PR2 must not add. So the worker seam never
//! sees a chat post. Instead we do what `bin/node`'s dispatch reactor already
//! does for its off-loop work (`resident_dispatch.rs`): read COMMITTED state.
//! A per-session background task queries committed chat over the ordinary
//! command lane (`NodeCommand::Query`, exactly like every HTTP read) and drives
//! new messages into the pty. It runs on the http/axum task, NOT the consensus
//! loop, so the query never re-enters and never deadlocks — and the SAME
//! `create_session` handler wires it on both `bin/noded` and `bin/node`.

use std::time::Duration;

use crate::{NodeCommand, NodeHandle};

/// how often the projector polls committed chat for new commands. A shared
/// terminal is human-driven, so ~200 ms adds no perceptible latency while
/// keeping the poll cheap (at most [`crate::term::MAX_TERM_SESSIONS`] live
/// sessions, one bounded query each).
/// `ponytail:` fixed interval poll; a block-commit watch (StreamHub already
/// has `subscribe_blocks`) would cut idle polls and trim the latency floor if a
/// future workload proves it matters.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// the chat channel that carries a session's ordered command lane. NON-colon by
/// necessity (see the module doc): a live node can only author `User`-origin
/// ids, and chat reserves colon ids to module/system origins. The 16-hex
/// session id keeps the pattern precise, so the app's hide predicate matches
/// exactly these and never a member's own `term-*` channel.
pub fn session_channel(session_id: &str) -> String {
    format!("term-{session_id}")
}

/// encode a submitted command line as the chat message body a member posts: a
/// single plain paragraph. Inverse of [`command_text`].
pub fn command_blocks(text: &str) -> Vec<chat::Block> {
    vec![chat::Block::paragraph(text)]
}

/// decode a committed chat message body back to the command line fed to the pty
/// — the inverse of [`command_blocks`]. Flattens paragraph/quote spans and code
/// text (a command is one plain paragraph, but be liberal in what we accept),
/// joining blocks with newlines so a pasted multi-line command survives.
pub fn command_text(blocks: &[chat::Block]) -> String {
    fn spans(out: &mut String, spans: &[chat::Span]) {
        for span in spans {
            out.push_str(&span.text);
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        let mut piece = String::new();
        match block {
            chat::Block::Paragraph(s) | chat::Block::Quote(s) => spans(&mut piece, s),
            chat::Block::Code { text, .. } => piece.push_str(text),
            chat::Block::Divider => {}
        }
        parts.push(piece);
    }
    parts.join("\n")
}

/// render a verified chat author into the display-grade `origin` string the
/// command lane records (mirrors the tags/index author rendering). This is the
/// CRYPTOGRAPHICALLY verified author derived from the committed frame's signed
/// origin — never a caller-supplied claim — which is what resolves the spec's
/// spoofable-origin finding: a `User` author's key bytes render to hex, so the
/// app can map them to a bound display name exactly as it does elsewhere.
fn render_author(author: &chat::AuthorRef) -> String {
    match author {
        chat::AuthorRef::User(bytes) => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        chat::AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        chat::AuthorRef::Module(id) => format!("module:{id}"),
        chat::AuthorRef::System => "system".to_string(),
    }
}

/// one committed command ready for the pty: the verified `origin` and the
/// decoded `text`. Deliberately does NOT carry the seq — the projector owns the
/// per-session cursor.
struct Projected {
    origin: String,
    text: String,
}

/// the project-or-skip decision for one committed message — the unit-testable
/// core of the projector. A tombstoned (deleted) message is skipped (its
/// content and reactions are cleared; running an empty redaction would be
/// wrong), but the caller still advances its cursor past it. Everything else
/// projects with the verified author and decoded text.
fn project_message(view: &chat::MessageView) -> Option<Projected> {
    if view.head.deleted {
        return None;
    }
    Some(Projected {
        origin: render_author(&view.head.author),
        text: command_text(&view.head.blocks),
    })
}

/// ensure the session's command channel exists (idempotent). Submits a chat
/// `CreateChannel` under the node's own key with an OPEN post policy so any
/// member can post commands. On `bin/node` the origin bytes are ignored and the
/// node key signs; on `bin/noded` they become the external author — either way
/// a `User` origin authoring a non-colon id, which chat accepts. An
/// already-existing channel (a retry) is success.
pub(crate) async fn ensure_channel(handle: &NodeHandle, channel: &str) -> Result<(), String> {
    let (reply, rx) = futures::channel::oneshot::channel();
    let payload = chat::encode_msg(&chat::ChatMsg::CreateChannel {
        channel_id: channel.to_string(),
        name: channel.to_string(),
        post_policy: chat::PostPolicy::Open,
    });
    handle
        .send(NodeCommand::Submit {
            target: chat::DEFAULT_CHAT_TARGET.to_string(),
            payload,
            origin: crate::DEFAULT_ORIGIN.as_bytes().to_vec(),
            reply,
        })
        .await
        .map_err(|_| "actor gone".to_string())?;
    match rx.await.map_err(|_| "reply dropped".to_string())? {
        Ok(_) => Ok(()),
        // a concurrent/retried create raced us to the same id: the channel is
        // there, which is all we needed.
        Err(reason) if reason.contains("already exists") => Ok(()),
        Err(reason) => Err(reason),
    }
}

/// query committed chat for the session channel's messages with `seq >= from_seq`,
/// ascending. Rides the ordinary command lane (`NodeCommand::Query`), so it
/// reads CANONICAL committed state and can never re-enter the consensus loop —
/// the projector task is not that loop.
async fn query_messages(
    handle: &NodeHandle,
    channel: &str,
    from_seq: u64,
) -> Result<Vec<chat::MessageView>, String> {
    let (reply, rx) = futures::channel::oneshot::channel();
    let req = chat::encode_query(&chat::ChatQuery::MessagesRange {
        channel_id: channel.to_string(),
        from_seq,
        limit: chat::MAX_QUERY_LIMIT,
    });
    handle
        .send(NodeCommand::Query {
            target: chat::DEFAULT_CHAT_TARGET.to_string(),
            req,
            reply,
        })
        .await
        .map_err(|_| "actor gone".to_string())?;
    let bytes = rx.await.map_err(|_| "reply dropped".to_string())??;
    match chat::decode_reply(&bytes)? {
        chat::ChatReply::Messages(views) => Ok(views),
        _ => Err("unexpected chat reply".to_string()),
    }
}

/// spawn the off-loop projector for a freshly created `Shared` session. Wired
/// once, from `create_session`, so it covers both binaries.
pub(crate) fn spawn_projector(handle: NodeHandle, session_id: String) {
    let channel = session_channel(&session_id);
    tokio::spawn(projector_loop(handle, session_id, channel));
}

/// the projector: poll committed chat for the session's channel and drive each
/// new message into the pty, in sequence, with its verified author. The single
/// per-session command consumer (`spawn_command_consumer` in `term.rs`) is what
/// actually feeds the pty and stamps the shared command log; this only enqueues,
/// so consensus order becomes pty order.
///
/// Precedence vs PR1: `enqueue_command` also backs the node-local ws
/// `TermCommand` path, which stays for single-node use. Both feed the SAME FIFO
/// lane, so they interleave in arrival order; for a multi-member shared session
/// the CONSENSUS path is authoritative and a client should drive exactly one
/// source (mutual exclusion is a possible follow-up, not required here).
///
/// Lifecycle: the loop exits the moment the session leaves the manager (EOF,
/// explicit close, or the wall-clock reaper), so it never outlives its pty and
/// never leaks — the same drop-driven teardown the pump and consumer take.
async fn projector_loop(handle: NodeHandle, session_id: String, channel: String) {
    tracing::info!(target: "ducktape::term", session = %session_id, "term_consensus_projector_started");
    // the per-session cursor: the highest chat seq already projected. Starts at
    // 0 — the channel is minted empty at session create, so seq 1 is the first
    // command. A pty session is ephemeral and node-local (it never survives a
    // restart), so there is no durable cursor to restore.
    let mut cursor = 0u64;
    loop {
        // stop as soon as the pty is gone. `session()` returning None means the
        // entry left the manager map; nothing more can be driven.
        match handle.terminals() {
            Some(terminals) if terminals.session(&session_id).is_some() => {}
            _ => break,
        }
        match query_messages(&handle, &channel, cursor + 1).await {
            Ok(views) => {
                for view in &views {
                    cursor = view.seq;
                    let Some(projected) = project_message(view) else {
                        continue;
                    };
                    // re-fetch the manager each command: it may have gone away
                    // between the query and here. enqueue_command logs the per-
                    // command debug (with seq, never the text) and no-ops+warns
                    // on an unknown/non-shared session, so this stays a no-op if
                    // the session just ended.
                    if let Some(terminals) = handle.terminals() {
                        terminals.enqueue_command(&session_id, projected.origin, projected.text);
                    }
                }
            }
            // transient (actor busy/gone mid-shutdown): retry next tick. debug,
            // not warn — a persistent failure must not bomb the log ring.
            Err(reason) => {
                tracing::debug!(target: "ducktape::term", session = %session_id, reason = %reason, "term_consensus_poll_failed");
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    tracing::info!(target: "ducktape::term", session = %session_id, "term_consensus_projector_stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat::{AuthorRef, Block, MessageHead, MessageView, Span};

    fn view(seq: u64, author: AuthorRef, blocks: Vec<Block>, deleted: bool) -> MessageView {
        MessageView {
            channel_id: "term-0000000000000001".into(),
            seq,
            head: MessageHead {
                message_id: format!("m{seq}"),
                author,
                blocks,
                created_at: 0,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::new(),
            channel_head_seq: seq,
        }
    }

    #[test]
    fn session_channel_is_non_colon_and_prefixed() {
        let ch = session_channel("00000000deadbeef");
        assert_eq!(ch, "term-00000000deadbeef");
        // the whole point of the scheme: a live node can author this, unlike a
        // colon id, and the app hide predicate keys off exactly this shape.
        assert!(!ch.contains(':'));
    }

    #[test]
    fn command_blocks_and_text_round_trip() {
        let line = "cargo test -p noded";
        let blocks = command_blocks(line);
        assert_eq!(command_text(&blocks), line);
        // a single plain paragraph is the exact wire a member posts.
        assert!(matches!(blocks.as_slice(), [Block::Paragraph(_)]));
    }

    #[test]
    fn command_text_flattens_multi_span_and_multi_block() {
        // liberal decode: multiple spans concatenate, blocks join by newline.
        let blocks = vec![
            Block::Paragraph(vec![Span::plain("echo "), Span::plain("hi")]),
            Block::Code {
                lang: None,
                text: "ls -la".into(),
            },
        ];
        assert_eq!(command_text(&blocks), "echo hi\nls -la");
    }

    #[test]
    fn project_skips_a_tombstone_but_projects_a_live_command() {
        // a deleted (redacted) message must NOT run; a live one projects with
        // the decoded text.
        assert!(project_message(&view(2, AuthorRef::System, Vec::new(), true)).is_none());
        let projected =
            project_message(&view(1, AuthorRef::User(vec![0xab, 0xcd]), command_blocks("pwd"), false))
                .expect("a live command projects");
        assert_eq!(projected.text, "pwd");
        // the verified User author renders to hex — a spoof-proof identity, not
        // a caller string (spec finding #5).
        assert_eq!(projected.origin, "abcd");
    }

    #[test]
    fn render_author_covers_every_kind() {
        assert_eq!(render_author(&AuthorRef::User(vec![0x01, 0xff])), "01ff");
        assert_eq!(
            render_author(&AuthorRef::Agent {
                module: "runs".into(),
                agent_id: "a1".into()
            }),
            "agent:runs/a1"
        );
        assert_eq!(render_author(&AuthorRef::Module("chat".into())), "module:chat");
        assert_eq!(render_author(&AuthorRef::System), "system");
    }
}
