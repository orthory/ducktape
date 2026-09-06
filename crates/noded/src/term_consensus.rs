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
//! keeps the log readable and writable as ordinary chat (external `User`
//! authors pass the open policy; `validate_channel_namespace` is not consulted
//! on `PostMessage`) while the DRIVER gate below decides what reaches the pty,
//! and the app hides it via an `isModuleChannel` extension that also matches
//! `term-<hex>`.
//!
//! ## Authorization: the pty is driven by the channel's OWNER
//!
//! Posting to `term-<id>` is open — the channel is a public, committed log and
//! its id is not a secret (it lands in chat state and is enumerable). DRIVING
//! the pty is not: [`project_message`] feeds a committed message to the pty
//! only when its cryptographically verified author equals the channel's
//! `owner`, and every other post is refused with `command_not_channel_owner`.
//!
//! `Channel.owner` is the right anchor and it needs nothing new. Chat sets it
//! from the SIGNED author at `CreateChannel` (`chat/src/lib.rs` `stage_channel`)
//! and already enforces it for rename/archive (`check_channel_admin`) — it is
//! the tree's existing answer to "who owns this channel". [`ensure_channel`] is
//! submitted by the node that owns the pty, so the owner IS that host on both
//! binaries and with no node-key plumbing: `bin/node` signs with its node key,
//! `bin/noded` with its configured origin — PROVIDED this node's own create
//! actually won the id. `term-<id>` is not secret (every forwarded session's
//! chunks fan out to every mesh member), so a member can `CreateChannel` the
//! same id first; [`ensure_channel`] treating that race loss as success used to
//! be #1746 — the projector would then read back and trust WHOEVER won, not
//! necessarily this host. `ensure_channel` now confirms ownership itself before
//! the projector ever spawns, so what the projector reads back is exactly what
//! `ensure_channel` already verified.
//!
//! **The gate is here, not on the post policy, and that is deliberate.**
//! `SetMembership` IS author-gated now — chat routes it through
//! `check_channel_admin`, so only the channel's owner writes the roster — which
//! means a multi-driver participant set has become expressible. It is still not
//! adopted here, for a reason that is a product decision rather than a missing
//! primitive: a `Shared` session spends the HOST's own env credential, which
//! carries no grant record and therefore no grantee, so widening the driver set
//! widens who spends the operator's personal subscription. Until that question
//! is answered the rightful set stays ONE — the owner — and it is enforced
//! where the effect happens rather than where the bytes become durable.
//! `JoinHuddle` under an open policy remains self-service, so a huddle roster
//! is not a candidate for this gate either.
//!
//! What this does NOT change: the node-local ws `TermCommand` lane
//! (`crate::stream::handle_term_command`) still drives the same FIFO with a
//! caller-supplied `origin`. That lane is trusted-local by construction — it
//! rides the same surface as `/v1/submit`, and a local process can already read
//! the node's key off disk — so it is gated by reachability, not by this.
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
/// keeping the poll cheap (at most [`agent_service::MAX_TERM_SESSIONS`] live
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
#[derive(Debug, PartialEq, Eq)]
struct Projected {
    origin: String,
    text: String,
}

/// true when a committed message's verified author IS the channel owner — the
/// driver gate. Only a `User` author can be one: chat mints `Channel.owner`
/// from `AuthorRef::User` bytes and leaves it `None` for module/system-minted
/// channels, so no module, agent, or system origin can ever match one.
fn author_is_owner(author: &chat::AuthorRef, owner: &[u8]) -> bool {
    match author {
        chat::AuthorRef::User(user) => user == owner,
        chat::AuthorRef::Agent { .. } | chat::AuthorRef::Module(_) | chat::AuthorRef::System => {
            false
        }
    }
}

/// the drive-or-refuse decision for one committed message — the unit-testable
/// core of the projector. `Err` is a stable snake_case reason the caller logs;
/// it advances its cursor either way, so a refused post is skipped, never
/// retried. Two refusals:
///
/// - `command_deleted` — the message is tombstoned (content and reactions are
///   cleared; running an empty redaction would be wrong).
/// - `command_not_channel_owner` — the verified author is not this channel's
///   owner, i.e. not the node that owns the pty. THIS is the gate: the channel
///   is open to post to, and open to read, but only its owner drives.
fn project_message(view: &chat::MessageView, owner: &[u8]) -> Result<Projected, &'static str> {
    let is_tombstone = view.head.deleted;
    if is_tombstone {
        return Err("command_deleted");
    }
    let from_owner = author_is_owner(&view.head.author, owner);
    if !from_owner {
        return Err("command_not_channel_owner");
    }
    Ok(Projected {
        origin: render_author(&view.head.author),
        text: command_text(&view.head.blocks),
    })
}

/// the channel's owner, or the stable reason there is none to gate on. FAILS
/// CLOSED on both: a projector that cannot name the owner drives nothing.
///
/// - `channel_unreadable` — the channel record is absent (a create that never
///   committed) or the query itself failed.
/// - `channel_unowned` — the record exists with no owner, which for a
///   `term-<id>` channel is impossible by construction (a live node can only
///   author a `User` origin, and chat owns every user-created channel) and so
///   means the id was squatted by a module/system-minted channel.
fn channel_owner(channel: Option<chat::Channel>) -> Result<Vec<u8>, &'static str> {
    let Some(channel) = channel else {
        return Err("channel_unreadable");
    };
    channel.owner.ok_or("channel_unowned")
}

/// what [`ensure_channel`] learned about the session's command channel.
///
/// A plain `Result<(), String>` used to treat "already exists" as success
/// outright — which is exactly the hole #1746 closes: "already exists" means
/// somebody else's `CreateChannel` won a race for this id, and that somebody
/// might not be this node. `Squatted` and `Failed` get different treatment
/// from [`crate::term::create_local`]: a squat is fatal to a `Shared` session
/// (closed, never silently driven by whoever squatted it), while a transient
/// failure degrades to the node-local ws command lane, exactly as before.
pub(crate) enum EnsureChannelOutcome {
    /// the channel exists AND this node's own account is its `owner` — safe
    /// for [`spawn_projector`] to trust.
    Ready,
    /// the channel exists under a DIFFERENT owner. Carries the stable reason
    /// [`confirm_ownership`] resolved it to.
    Squatted(&'static str),
    /// neither of the above — an actor/transport failure, never an
    /// authorization outcome. Carries the raw (free-form) detail.
    Failed(String),
}

/// ensure the session's command channel exists AND this node owns it
/// (idempotent). Submits a chat `CreateChannel` under the node's own key with
/// an OPEN post policy — the log is public; [`project_message`] is what
/// decides which post drives the pty. On `bin/node` the origin bytes are
/// ignored and the node key signs; on `bin/noded` they become the external
/// author — either way a `User` origin authoring a non-colon id, which chat
/// accepts, and which becomes the `Channel.owner` the projector gates on.
///
/// "already exists" used to be treated as success outright: a member watching
/// this session's chunk fanout learns its id and can `CreateChannel` the same
/// `term-<id>` first, and if that commit lands ahead of this node's own, this
/// node's create loses the race — but the caller has no way to tell "I already
/// had it" from "someone else already has it" from the error string alone. So
/// a race loss is followed by [`confirm_ownership`], which asks the one
/// question that matters: is the channel that already exists OURS.
pub(crate) async fn ensure_channel(handle: &NodeHandle, channel: &str) -> EnsureChannelOutcome {
    let (reply, rx) = futures::channel::oneshot::channel();
    let payload = chat::encode_msg(&chat::ChatMsg::CreateChannel {
        channel_id: channel.to_string(),
        name: channel.to_string(),
        post_policy: chat::PostPolicy::Open,
    });
    let sent = handle
        .send(NodeCommand::Submit {
            target: chat::DEFAULT_CHAT_TARGET.to_string(),
            payload,
            origin: crate::DEFAULT_ORIGIN.as_bytes().to_vec(),
            reply,
        })
        .await;
    if sent.is_err() {
        return EnsureChannelOutcome::Failed("actor gone".to_string());
    }
    match rx.await {
        Err(_) => EnsureChannelOutcome::Failed("reply dropped".to_string()),
        Ok(Ok(_)) => EnsureChannelOutcome::Ready,
        // a concurrent create raced us to the same id: the channel is there,
        // but "there" is not "ours" — confirm it before trusting it.
        Ok(Err(reason)) if reason.contains("already exists") => {
            confirm_ownership(handle, channel).await
        }
        Ok(Err(reason)) => EnsureChannelOutcome::Failed(reason),
    }
}

/// this node's own account, in the same shape `Channel.owner` compares
/// against — a pure LOCAL read, no query and no write.
///
/// Mirrors the origin resolution [`ensure_channel`]'s doc describes: `bin/node`
/// signs a `Submit` with its own node key regardless of the `origin` field, and
/// that key is exactly `NodeStatus.public_key` (`bin/node/src/boot/mesh.rs`
/// derives both from the same signer) — published once at boot, so reading it
/// back here costs nothing that reaching consensus would. A daemon with no mesh
/// identity (the embedded local daemon, `bin/noded`) publishes an empty
/// `public_key`, and on that path the origin field IS the author: the literal
/// [`crate::DEFAULT_ORIGIN`] bytes `ensure_channel` submits.
fn self_account(handle: &NodeHandle) -> Vec<u8> {
    let public_key = handle.status_cell().current().public_key;
    match crate::term::decode_node_key(&public_key) {
        Some(bytes) => bytes.to_vec(),
        None => crate::DEFAULT_ORIGIN.as_bytes().to_vec(),
    }
}

/// prove this node's own account owns `channel` after losing the create race
/// for it — a READ, not a write: `query_channel` is the same `ChatQuery::Channel`
/// point read [`projector_loop`] already trusts, so this asks it the one
/// question that matters BEFORE the projector ever spawns, instead of after.
/// No new chat op and no new query shape — `Channel.owner` was always the
/// right anchor, the missing piece was comparing it against something.
async fn confirm_ownership(handle: &NodeHandle, channel: &str) -> EnsureChannelOutcome {
    let record = match query_channel(handle, channel).await {
        Ok(record) => record,
        Err(reason) => return EnsureChannelOutcome::Failed(reason),
    };
    match channel_owner(record) {
        Ok(owner) if owner == self_account(handle) => EnsureChannelOutcome::Ready,
        // either a different account's `User` origin owns it, or (impossible
        // by construction for a `term-<id>` channel, but fail closed anyway)
        // it has no owner at all — neither is this node's own account.
        Ok(_) | Err(_) => EnsureChannelOutcome::Squatted("channel_owned_by_another_account"),
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

/// query committed chat for the session channel's record — the projector reads
/// it once, for `Channel.owner`. Rides the same command lane as
/// [`query_messages`].
async fn query_channel(
    handle: &NodeHandle,
    channel: &str,
) -> Result<Option<chat::Channel>, String> {
    let (reply, rx) = futures::channel::oneshot::channel();
    let req = chat::encode_query(&chat::ChatQuery::Channel {
        channel_id: channel.to_string(),
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
        chat::ChatReply::Channel(channel) => Ok(channel),
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
///
/// The owner is resolved ONCE, before the first poll, and the projector refuses
/// to start without one — fail closed. Once resolved it cannot change: chat has
/// no op that rewrites `Channel.owner` (rename and archive touch other fields),
/// so re-reading it per tick would buy nothing and cost a query every 200 ms.
async fn projector_loop(handle: NodeHandle, session_id: String, channel: String) {
    let resolved = query_channel(&handle, &channel)
        .await
        .map_err(|_| "channel_unreadable")
        .and_then(channel_owner);
    let owner = match resolved {
        Ok(owner) => owner,
        // once per session, and the session keeps working on the node-local ws
        // lane — only the consensus lane is refused. No id, no author bytes.
        Err(reason) => {
            tracing::warn!(target: "ducktape::term", reason, "term_consensus_projector_refused");
            return;
        }
    };
    tracing::info!(target: "ducktape::term", session = %session_id, "term_consensus_projector_started");
    // the per-session cursor: the highest chat seq already projected. Starts at
    // 0 — the channel is minted empty at session create, so seq 1 is the first
    // command. A pty session is ephemeral and node-local (it never survives a
    // restart), so there is no durable cursor to restore.
    let mut cursor = 0u64;
    loop {
        // stop as soon as the pty is gone. A session has a `mode` exactly while
        // it is in the bridge's map, so `None` means the entry left it (EOF,
        // close, reaper, or the agent service detaching) and nothing more can be
        // driven.
        match handle.terminals() {
            Some(terminals) if terminals.mode(&session_id).is_some() => {}
            _ => break,
        }
        match query_messages(&handle, &channel, cursor + 1).await {
            Ok(views) => {
                for view in &views {
                    cursor = view.seq;
                    let projected = match project_message(view, &owner) {
                        Ok(projected) => projected,
                        // per-post, so `debug` — a member who spams the open
                        // channel must not evict the log ring. The reason is the
                        // whole diagnosis; the author and the text stay out.
                        Err(reason) => {
                            tracing::debug!(target: "ducktape::term", reason, "term_command_refused");
                            continue;
                        }
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
    use futures::StreamExt as _;

    fn committed_block() -> crate::BlockSummary {
        crate::BlockSummary {
            height: 1,
            root_hash: "ab".repeat(32),
        }
    }

    /// a stand-in chat actor for [`ensure_channel`]/[`confirm_ownership`]:
    /// answers `CreateChannel` with `create_reply` (the first-create outcome)
    /// and `ChatQuery::Channel` with a record owned by `existing_owner` —
    /// [`confirm_ownership`]'s READ, reached only after a create loss. Every
    /// other op panics — these are the only two `ensure_channel`'s path ever
    /// issues.
    fn spawn_chat_actor(
        mut rx: futures::channel::mpsc::Receiver<NodeCommand>,
        create_reply: Result<(), &'static str>,
        existing_owner: Option<Vec<u8>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(cmd) = rx.next().await {
                match cmd {
                    NodeCommand::Submit { payload, reply, .. } => {
                        match chat::decode_msg(&payload).expect("a chat op") {
                            chat::ChatMsg::CreateChannel { .. } => {
                                let _ = reply.send(
                                    create_reply
                                        .map(|()| committed_block())
                                        .map_err(|reason| reason.to_string()),
                                );
                            }
                            other => panic!("ensure_channel submitted an unexpected op: {other:?}"),
                        }
                    }
                    NodeCommand::Query { req, reply, .. } => {
                        let chat::ChatQuery::Channel { channel_id } =
                            chat::decode_query(&req).expect("a chat query")
                        else {
                            panic!("confirm_ownership queried something other than Channel");
                        };
                        let record = existing_owner.clone().map(|owner| chat::Channel {
                            id: channel_id.clone(),
                            name: channel_id,
                            created_at: 0,
                            head_seq: 0,
                            post_policy: chat::PostPolicy::Open,
                            hooks: Vec::new(),
                            pinned: Vec::new(),
                            huddle: Vec::new(),
                            owner: Some(owner),
                            archived: false,
                        });
                        let _ =
                            reply.send(Ok(chat::encode_reply(&chat::ChatReply::Channel(record))));
                    }
                    NodeCommand::SubmitFrame { .. } => {
                        panic!("ensure_channel used SubmitFrame unexpectedly")
                    }
                }
            }
        })
    }

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

    /// the channel owner every test below gates against — the host node that
    /// created the session's channel.
    const HOST: [u8; 2] = [0xab, 0xcd];

    fn channel(owner: Option<Vec<u8>>) -> chat::Channel {
        chat::Channel {
            id: "term-0000000000000001".into(),
            name: "term-0000000000000001".into(),
            created_at: 0,
            head_seq: 0,
            post_policy: chat::PostPolicy::Open,
            hooks: Vec::new(),
            pinned: Vec::new(),
            huddle: Vec::new(),
            owner,
            archived: false,
        }
    }

    #[test]
    fn the_channel_owner_drives_the_pty() {
        let projected = project_message(
            &view(
                1,
                AuthorRef::User(HOST.into()),
                command_blocks("pwd"),
                false,
            ),
            &HOST,
        )
        .expect("the owner's command projects");
        assert_eq!(projected.text, "pwd");
        // the verified User author renders to hex — a spoof-proof identity, not
        // a caller string (spec finding #5).
        assert_eq!(projected.origin, "abcd");
    }

    #[test]
    fn any_other_member_is_refused_at_the_pty() {
        // THE HOLE THIS GATE CLOSES: the command channel is `PostPolicy::Open`,
        // so an admitted member's post commits exactly like the owner's — it is
        // signed, ordered, durable and indistinguishable at the chat layer. The
        // projector is the only thing between it and a live pty spending the
        // host's own subscription. Mutating `author_is_owner` to `true` reddens
        // this and nothing else.
        let stranger = AuthorRef::User(vec![0x99, 0x99]);
        assert_eq!(
            project_message(&view(1, stranger, command_blocks("rm -rf /"), false), &HOST),
            Err("command_not_channel_owner"),
        );
    }

    #[test]
    fn no_module_agent_or_system_author_drives_a_session() {
        // an owner is always `AuthorRef::User` bytes, so no non-user origin can
        // equal one — asserted per kind so a new `AuthorRef` variant has to be
        // routed here rather than silently defaulting open.
        for author in [
            AuthorRef::Module("chat".into()),
            AuthorRef::Agent {
                module: "runs".into(),
                agent_id: "a1".into(),
            },
            AuthorRef::System,
        ] {
            assert!(!author_is_owner(&author, &HOST));
        }
    }

    #[test]
    fn a_tombstone_is_refused_even_from_the_owner() {
        // a deleted (redacted) message must NOT run: its content is cleared, and
        // running an empty redaction would be wrong.
        assert_eq!(
            project_message(
                &view(2, AuthorRef::User(HOST.into()), Vec::new(), true),
                &HOST
            ),
            Err("command_deleted"),
        );
    }

    #[test]
    fn a_projector_with_no_owner_to_gate_on_refuses_to_start() {
        // fail closed, both ways: no channel record and an unowned channel each
        // yield a named refusal, never an owner the gate would compare against.
        assert_eq!(channel_owner(None), Err("channel_unreadable"));
        assert_eq!(channel_owner(Some(channel(None))), Err("channel_unowned"));
        assert_eq!(
            channel_owner(Some(channel(Some(HOST.into())))),
            Ok(HOST.to_vec()),
        );
    }

    #[tokio::test]
    async fn ensure_channel_is_ready_on_a_fresh_create() {
        let (handle, rx, _hub) = NodeHandle::channel();
        let actor = spawn_chat_actor(rx, Ok(()), None);
        assert!(matches!(
            ensure_channel(&handle, "term-0000000000000001").await,
            EnsureChannelOutcome::Ready
        ));
        drop(handle);
        actor.await.expect("actor task");
    }

    #[tokio::test]
    async fn ensure_channel_confirms_ownership_after_losing_the_create_race() {
        // the create lost the race ("already exists"), but the READ back says
        // the existing channel's owner is `DEFAULT_ORIGIN` — the same account
        // `NodeHandle::channel()`'s bare test handle (no mesh identity)
        // resolves to. This node's own account already IS the channel's
        // owner. Ready, same as a fresh create — and no write was made.
        let (handle, rx, _hub) = NodeHandle::channel();
        let actor = spawn_chat_actor(
            rx,
            Err("channel already exists: term-0000000000000001"),
            Some(crate::DEFAULT_ORIGIN.as_bytes().to_vec()),
        );
        assert!(matches!(
            ensure_channel(&handle, "term-0000000000000001").await,
            EnsureChannelOutcome::Ready
        ));
        drop(handle);
        actor.await.expect("actor task");
    }

    /// **#1746, the regression.** A create loss ("already exists") used to be
    /// treated as success outright, with no check of who actually won it — so
    /// a member who pre-squatted `term-<id>` became the session's command
    /// author. Now the READ-back owner is compared against this node's own
    /// account, and a mismatch surfaces as `Squatted`, which `create_local`
    /// closes the session on rather than spawning a projector that would
    /// trust the squatter as `Channel.owner`.
    #[tokio::test]
    async fn ensure_channel_refuses_a_channel_squatted_by_another_account() {
        let (handle, rx, _hub) = NodeHandle::channel();
        let actor = spawn_chat_actor(
            rx,
            Err("channel already exists: term-0000000000000001"),
            Some(vec![0x99, 0x99]),
        );
        assert!(matches!(
            ensure_channel(&handle, "term-0000000000000001").await,
            EnsureChannelOutcome::Squatted("channel_owned_by_another_account")
        ));
        drop(handle);
        actor.await.expect("actor task");
    }

    #[tokio::test]
    async fn ensure_channel_passes_through_a_transient_failure() {
        // NOT an authorization outcome — an actor/transport failure never
        // reaches `confirm_ownership` and must not be mistaken for a squat.
        let (handle, rx, _hub) = NodeHandle::channel();
        let actor = spawn_chat_actor(rx, Err("host busy"), None);
        assert!(matches!(
            ensure_channel(&handle, "term-0000000000000001").await,
            EnsureChannelOutcome::Failed(reason) if reason == "host busy"
        ));
        drop(handle);
        actor.await.expect("actor task");
    }

    #[test]
    fn self_account_falls_back_to_default_origin_with_no_mesh_identity() {
        // the bare test handle publishes no `NodeStatus` (empty `public_key`)
        // — the embedded-local-daemon shape `ensure_channel`'s doc describes.
        let (handle, _rx, _hub) = NodeHandle::channel();
        assert_eq!(self_account(&handle), crate::DEFAULT_ORIGIN.as_bytes());
    }

    #[test]
    fn self_account_reads_the_published_mesh_identity_when_present() {
        let (handle, _rx, _hub) = NodeHandle::channel();
        handle.status_cell().publish(crate::NodeStatus {
            public_key: "ab".repeat(32),
            ..Default::default()
        });
        assert_eq!(self_account(&handle), vec![0xab; 32]);
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
        assert_eq!(
            render_author(&AuthorRef::Module("chat".into())),
            "module:chat"
        );
        assert_eq!(render_author(&AuthorRef::System), "system");
    }
}
