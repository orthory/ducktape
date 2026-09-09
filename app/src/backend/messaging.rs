//! The agent-messaging panel's authenticated I/O: the reads that draw one
//! conversation, the send that leaves it, and the fences around both.
//!
//! The division of labour this file exists to hold:
//!
//! * **The network decides who may see what.** Every read here goes to
//!   `POST /v1/query/reader` signed by this device's seated key, so the
//!   collaboration module resolves the caller from the authenticated envelope
//!   and answers or refuses. This app never asks the open `/v1/query` lane for
//!   a conversation — that lane reaches a module as the system origin, which
//!   the module refuses for protected content, and an anonymous read of
//!   protected state would be a refusal wearing an empty answer's clothes.
//! * **A refusal is a refusal.** Every failure — locked key, a moved network,
//!   a transport fault, a module denial — lands in [`MessagingView::error`] or
//!   [`MessagingView::denied`] with the panel's ids intact. It NEVER lands as
//!   an empty message list, which reads as "this conversation is empty".
//! * **Delivery is not work.** The panel shows each message's own delivery
//!   record and stops there. Nothing here infers a task claim, a model's
//!   attention, or a completion from a delivery state, from prose, or from any
//!   terminal output.
//! * **A scope fences everything.** A load and a send both carry the chain id
//!   they were started under, and both re-read the endpoint's chain before
//!   trusting it. An answer read from another network is never installed under
//!   this one, and a message written for one conversation is never sent under
//!   another.
//!
//! Local provider state — socket paths, session ids, provider tokens — never
//! appears here, because it never reaches this app: a binding is read through
//! `BindingView`, which carries its device LABEL and no key at all.

use super::*;
use collaboration::{
    BindingView, CollaborationMsg, CollaborationQuery, CollaborationReply, Conversation,
    DenyReason, EventPage, Message, MessageId, MessageKind, Participant, ProtectedRead, Receipt,
    Reference, Role, SendRequest, SendState,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The module this panel reads and writes. One spelling.
const COLLABORATION: &str = "collaboration";

/// Events one page asks for. Core admits up to `MAX_PAGE_LIMIT` (64); this is
/// smaller on purpose — one page's messages, their bodies and their receipts
/// all have to fit the view tree's frame budget beside the agent register.
const PAGE: u64 = 24;

/// How much of one body the panel DRAWS. The stored message is never altered
/// and never truncated at admission — this bounds the projection only, and the
/// row says so whenever it bites.
const BODY_DISPLAY_BYTES: usize = 1024;

/// References drawn per message before the line says how many are left.
const REFERENCES_SHOWN: usize = 4;

/// Characters of one reference the line draws. A `duck://` link may be a
/// kilobyte; a row is not the place to spell one out.
const REFERENCE_CHARS: usize = 160;

/// The delivery deadline a message from this panel asks for — the spec's
/// default, in the network's own `consensus_time` unit.
const DELIVERY_TTL_SECS: u64 = 24 * 60 * 60;

/// Probes one sequence allocation may spend before it refuses.
///
/// The search doubles and then halves, and BOTH phases spend from this budget:
/// a credential that has admitted `n` messages costs about `2·log2(n)` probes,
/// so this covers roughly 2^16 of them. A walk that needs more has met
/// something this allocator does not model, and guessing past it would mint a
/// duplicate id.
const MAX_SEQUENCE_PROBES: u32 = 32;

/// What a spent sequence search says. A refusal, never a guess: handing out a
/// sequence this walk could not prove free is how a duplicate id is minted.
const SEQUENCE_EXHAUSTED: &str =
    "could not find a free message sequence for this credential — nothing was sent";

/// What this network's storage and read visibility ACTUALLY is. Stated on the
/// panel because posting restrictions are not read confidentiality, and this
/// release neither implements nor promises end-to-end encrypted messages.
const VISIBILITY: &str = "Committed state on this network is replicated in plaintext to every \
                          validator. This panel authenticates who may read it over RPC — that is \
                          not end-to-end encryption, and this release does not promise private \
                          messages.";

// ---- the projection ---------------------------------------------------------

/// One seat on the conversation's roster.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize)]
pub struct MessagingSeat {
    pub participant: String,
    pub role: String,
    pub you: bool,
}

/// The acting participant's own input binding, as `BindingView` exposes it —
/// which is to say without its scoped service key, by construction.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize)]
pub struct MessagingBinding {
    pub present: bool,
    pub device: String,
    pub credential: String,
    pub detached: bool,
}

/// One admitted message and its SEPARATE delivery record.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize)]
pub struct MessagingMessage {
    pub seq: i64,
    pub sender: String,
    pub recipient: String,
    pub kind: String,
    /// a bounded slice of the stored body — see [`BODY_DISPLAY_BYTES`]
    pub body: String,
    pub body_bytes: i64,
    pub shown_bytes: i64,
    pub references: String,
    pub reply_to: i64,
    pub task: String,
    pub task_attempt: i64,
    /// the delivery state's stable token, or "" when this app could not
    /// establish it. NEVER a guess: an unread receipt is unknown, not `stored`.
    pub delivery: String,
    pub delivery_reason: String,
    pub mine: bool,
    pub expires_at: i64,
    pub admitted_at: i64,
}

/// One conversation as one authenticated reader saw it, at one scope.
///
/// `rpc` and `network` are the fence: the handler installs this only while the
/// app is still connected to that endpoint on that chain. `rpc` is deliberately
/// NOT forwarded to the view guest — the guest draws a conversation, and the
/// endpoint it was read from is none of its business.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize)]
pub struct MessagingView {
    pub rpc: String,
    pub network: String,
    /// the connection revision this read started under — a reconnect to the
    /// SAME endpoint on the SAME chain is still a different link, and an answer
    /// from the old one is about a session this app no longer holds
    pub link: i64,
    /// the account seated when this read started
    pub account: String,
    /// this read's own nonce. Two reads of the same conversation on the same
    /// link are still two operations, and the older one finishing last must not
    /// overwrite the newer one's answer.
    pub op: i64,
    pub participant: String,
    pub conversation: String,
    pub topic: String,
    pub roster: Vec<MessagingSeat>,
    pub binding: MessagingBinding,
    pub messages: Vec<MessagingMessage>,
    pub may_read: bool,
    pub may_send: bool,
    pub denied: String,
    pub error: String,
    pub history_gap: bool,
    pub floor_seq: i64,
    pub from_seq: i64,
    pub next_seq: i64,
    /// events one page asks for — the step the panel's older/newer move by
    pub page_size: i64,
    pub more_before: bool,
    pub more_after: bool,
    pub undelivered: i64,
    pub queued_bytes: i64,
    pub max_body_bytes: i64,
    pub answered: bool,
    pub visibility: String,
}

impl MessagingView {
    /// The panel's scope with nothing read yet — the shape every refusal
    /// below keeps, so a refused read still says WHAT was refused.
    fn scoped(scope: &Scope, participant: &str, conversation: &str) -> Self {
        Self {
            rpc: scope.rpc.clone(),
            network: scope.network.clone(),
            link: scope.link,
            account: scope.account.clone(),
            op: scope.op,
            participant: participant.to_owned(),
            conversation: conversation.to_owned(),
            max_body_bytes: count_i64(collaboration::MAX_BODY_BYTES),
            page_size: count_i64_u64(PAGE),
            visibility: VISIBILITY.to_owned(),
            ..Self::default()
        }
    }
}

/// What one send left under, and what became of it.
///
/// `refusal` is not a failure route: the send either was not attempted or was
/// not admitted, and either way this says so IN the scope it was tried in. The
/// scope is the point — a send admitted for conversation A must never clear a
/// draft written for B, and an error about A must never appear under B.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize)]
pub struct MessagingSend {
    pub rpc: String,
    pub network: String,
    pub link: i64,
    pub account: String,
    pub op: i64,
    pub participant: String,
    pub conversation: String,
    /// "" when the network admitted the message
    pub refusal: String,
}

/// What an operation was started under, carried through it and back out.
///
/// Internal glue: Ice constructs no struct, so the extern entry points take the
/// fields flat and build this once. It exists so the fence is one value the
/// whole operation carries rather than five arguments a call site can forget a
/// field of.
#[derive(Clone, Debug, Default)]
struct Scope {
    rpc: String,
    network: String,
    link: i64,
    account: String,
    op: i64,
}

/// Test and state seam: Ice reads extern structs but cannot construct one.
pub fn messaging_none() -> MessagingView {
    MessagingView::default()
}

/// Whether this reading still belongs to what the app is showing: the same
/// endpoint, the same chain, the same acting participant, the same
/// conversation.
///
/// This is the fence a late answer meets. A read started before a network
/// switch, a participant change or a conversation change answers into an app
/// that has moved on, and installing it would show one conversation's messages
/// under another's name.
///
/// The ids alone are NOT the fence. Leaving A for B and coming back to A
/// restores every id, so A's first answer would match on the way back in and
/// overwrite what the second read found. `link`, `account` and `op` are what
/// make the comparison identify the OPERATION rather than its subject: a
/// reconnect moves `link`, a seat change moves `account`, and each dispatch
/// mints a fresh `op`.
#[allow(clippy::too_many_arguments)]
pub fn messaging_in_scope(
    view: &MessagingView,
    rpc: &str,
    network: &str,
    link: i64,
    account: &str,
    op: i64,
    participant: &str,
    conversation: &str,
) -> bool {
    view.rpc == rpc
        && view.network == network
        && view.link == link
        && view.account == account
        && view.op == op
        && view.participant == participant
        && view.conversation == conversation
}

/// [`messaging_in_scope`] for a send's outcome.
#[allow(clippy::too_many_arguments)]
pub fn messaging_send_in_scope(
    send: &MessagingSend,
    rpc: &str,
    network: &str,
    link: i64,
    account: &str,
    op: i64,
    participant: &str,
    conversation: &str,
) -> bool {
    send.rpc == rpc
        && send.network == network
        && send.link == link
        && send.account == account
        && send.op == op
        && send.participant == participant
        && send.conversation == conversation
}

// ---- reading ----------------------------------------------------------------

/// Read one conversation as `participant`, at one page.
///
/// Infallible at this boundary on purpose: the panel has ONE shape, and a
/// failure is a field in it. A `Result` here would split the refusal display
/// across two handler routes, and the branch that forgot one would show an
/// empty conversation.
#[allow(clippy::too_many_arguments)]
pub async fn load_messaging(
    rpc: String,
    network: String,
    link: i64,
    account: String,
    op: i64,
    participant: String,
    conversation: String,
    from_seq: i64,
    newest: bool,
) -> MessagingView {
    let scope = Scope {
        rpc,
        network,
        link,
        account,
        op,
    };
    let view = MessagingView::scoped(&scope, &participant, &conversation);
    // nothing is open: the panel's own closed state, not a refusal
    if participant.is_empty() || conversation.is_empty() {
        return view;
    }
    match read_conversation(&view, from_seq, newest).await {
        Ok(read) => read,
        Err(message) => MessagingView {
            error: message,
            answered: true,
            ..view
        },
    }
}

async fn read_conversation(
    view: &MessagingView,
    from_seq: i64,
    newest: bool,
) -> Result<MessagingView, String> {
    let (client, identity) = reader_client(&view.rpc, &view.network).await?;
    let access = match read(
        &client,
        &view.participant,
        ProtectedRead::Access {
            conversation_id: view.conversation.clone(),
        },
    )
    .await?
    {
        CollaborationReply::Access(access) => access,
        CollaborationReply::Denied(reason) => return Ok(refused(view, reason)),
        other => return Err(unexpected("an access check", &other)),
    };
    // `ConversationAccess` fails closed: an unknown conversation, a revoked
    // participant and a caller off the roster all answer all-false. That is a
    // refusal, and it is shown as one — never as a conversation with no
    // messages in it.
    if !access.may_read {
        return Ok(MessagingView {
            denied: deny_token(DenyReason::NotPermitted).to_owned(),
            answered: true,
            ..view.clone()
        });
    }
    let conversation = match read(
        &client,
        &view.participant,
        ProtectedRead::Conversation {
            conversation_id: view.conversation.clone(),
        },
    )
    .await?
    {
        CollaborationReply::Conversation(conversation) => conversation,
        CollaborationReply::Denied(reason) => return Ok(refused(view, reason)),
        other => return Err(unexpected("a conversation", &other)),
    };
    let binding = match read(
        &client,
        &view.participant,
        ProtectedRead::Binding {
            conversation_id: view.conversation.clone(),
        },
    )
    .await?
    {
        CollaborationReply::Binding(binding) => binding,
        CollaborationReply::Denied(reason) => return Ok(refused(view, reason)),
        other => return Err(unexpected("a binding", &other)),
    };
    let mailbox = match read(&client, &view.participant, ProtectedRead::Mailbox).await? {
        CollaborationReply::Mailbox(usage) => usage,
        CollaborationReply::Denied(reason) => return Ok(refused(view, reason)),
        other => return Err(unexpected("a mailbox", &other)),
    };
    let from = page_start(from_seq, newest, &conversation);
    let page = match read(
        &client,
        &view.participant,
        ProtectedRead::Events {
            conversation_id: view.conversation.clone(),
            from_seq: from,
            limit: PAGE,
        },
    )
    .await?
    {
        CollaborationReply::Events(page) => page,
        CollaborationReply::Denied(reason) => return Ok(refused(view, reason)),
        other => return Err(unexpected("an event page", &other)),
    };
    let opened = MessagingView {
        topic: conversation.topic.clone(),
        roster: seats(&conversation, &view.participant),
        binding: binding_view(binding.as_ref()),
        may_read: access.may_read,
        may_send: access.may_send,
        floor_seq: count_i64_u64(conversation.floor_seq),
        next_seq: count_i64_u64(conversation.next_seq),
        from_seq: count_i64_u64(from),
        undelivered: count_i64_u64(mailbox.undelivered),
        queued_bytes: count_i64_u64(mailbox.queued_bytes),
        answered: true,
        ..view.clone()
    };
    let EventPage::Page {
        messages, next_seq, ..
    } = page
    else {
        // the cursor is below the retained floor: resync from the floor rather
        // than advance past events that no longer exist
        still_signing_as(&identity).await?;
        return Ok(MessagingView {
            history_gap: true,
            ..opened
        });
    };
    let rows = message_rows(&client, view, &messages).await;
    still_signing_as(&identity).await?;
    Ok(MessagingView {
        more_before: from > conversation.floor_seq,
        more_after: next_seq < conversation.next_seq,
        messages: rows,
        ..opened
    })
}

/// A client that signs its reads with the key ALREADY SEATED, on the network
/// this panel was opened against, and the identity it signs as.
///
/// Three refusals live here, and each is the honest answer rather than an empty
/// panel: an unnamed or moved chain must never answer under this scope (ids
/// carry no network in them, so the same conversation name on another chain is
/// another conversation), a locked key cannot make an authenticated read at
/// all, and a node that could not be asked is neither of those.
///
/// The signing hook rides a client this caller OWNS for one operation. It is
/// never attached to the process-wide cached client, which outlives a Lock, an
/// account switch and a network change.
async fn reader_client(rpc: &str, network: &str) -> Result<(RpcClient, String), String> {
    if network.is_empty() {
        return Err(
            "this node has not named its network yet — an unnamed chain is not a scope \
                    to read a conversation under"
                .into(),
        );
    }
    let client = rpc_client(rpc)?;
    let status = client
        .status_json()
        .await
        .map_err(|error| error.to_string())?;
    let facts = node_facts(&status);
    if facts.chain_id != network {
        return Err(format!(
            "this endpoint now serves network {}, not the one this conversation was opened on \
             — reopen it",
            facts.chain_id
        ));
    }
    match seated_data_plane_signer(&facts.public_key).await {
        ReadSigner::Seated { auth, key } => Ok((client.with_write_auth(auth), key)),
        ReadSigner::Locked => Err("this device's key is locked; unlock it to read messages".into()),
        ReadSigner::Unavailable(reason) => Err(format!(
            "this node could not say which key to sign this read for: {reason}"
        )),
    }
}

/// Refuse an answer produced under an identity this session no longer holds.
///
/// A read takes several round trips. A Lock or an account switch during them
/// leaves an answer that WAS authorized — for somebody who is no longer at this
/// keyboard — and installing it would show one identity's mail under another's
/// name. The snapshot is taken before the first read and checked before the
/// answer is returned.
async fn still_signing_as(identity: &str) -> Result<(), String> {
    match seated_public_key().await.as_deref() {
        Some(seated) if seated == identity => Ok(()),
        Some(_) => Err(
            "the signing key changed while this was being read — reopen the \
                        conversation"
                .into(),
        ),
        None => Err("this device's key was locked while this was being read".into()),
    }
}

/// One authenticated read, as the acting participant.
///
/// `via` is `None` on purpose: this app holds the participant's OWNER key, not
/// a conversation-scoped service key. The scoped keys belong to the attached
/// agent services and never reach this process.
async fn read(
    client: &RpcClient,
    participant: &str,
    read: ProtectedRead,
) -> Result<CollaborationReply, String> {
    client
        .query_as_reader(
            COLLABORATION,
            &CollaborationQuery::Read {
                participant_id: participant.to_owned(),
                via: None,
                read,
            },
        )
        .await
        .map_err(|error| error.to_string())
}

/// The message rows of one page, each with its OWN delivery record.
///
/// The receipt is read per message rather than folded out of the page's
/// events, because a message's delivery advances at a LATER sequence than its
/// admission — often past the end of the page it is drawn on. Folding the page
/// alone would show every message as `stored` forever. Bounded by [`PAGE`].
async fn message_rows(
    client: &RpcClient,
    view: &MessagingView,
    messages: &[Message],
) -> Vec<MessagingMessage> {
    let receipts = iced::futures::future::join_all(messages.iter().map(|message| {
        read(
            client,
            &view.participant,
            ProtectedRead::Receipt {
                conversation_id: view.conversation.clone(),
                seq: message.seq,
            },
        )
    }))
    .await;
    messages
        .iter()
        .zip(receipts)
        .map(|(message, receipt)| message_row(message, receipt, &view.participant))
        .collect()
}

fn message_row(
    message: &Message,
    receipt: Result<CollaborationReply, String>,
    participant: &str,
) -> MessagingMessage {
    let (body, shown) = display_body(&message.body);
    // AN UNRESOLVED RECEIPT IS UNKNOWN, never `stored`. Naming a delivery state
    // this app did not read would be a fabricated delivery claim, and `stored`
    // is the most reassuring of the seven — exactly the wrong guess.
    let state: Option<Receipt> = match receipt {
        Ok(CollaborationReply::Receipt(Some(receipt))) => Some(receipt),
        _ => None,
    };
    MessagingMessage {
        seq: count_i64_u64(message.seq),
        sender: message.sender.clone(),
        recipient: message.recipient.clone(),
        kind: kind_token(message.kind).to_owned(),
        body,
        body_bytes: count_i64(message.body.len()),
        shown_bytes: shown,
        references: render_references(&message.references),
        reply_to: message.reply_to.map(count_i64_u64).unwrap_or_default(),
        task: message
            .task
            .as_ref()
            .map(|task| task.id.clone())
            .unwrap_or_default(),
        task_attempt: message
            .task
            .as_ref()
            .map(|task| count_i64_u64(task.expected_attempt))
            .unwrap_or_default(),
        delivery: state
            .as_ref()
            .map(|receipt| receipt.state.as_str().to_owned())
            .unwrap_or_default(),
        delivery_reason: state.and_then(|receipt| receipt.reason).unwrap_or_default(),
        mine: message.sender == participant,
        expires_at: count_i64_u64(message.expires_at),
        admitted_at: count_i64_u64(message.admitted_at),
    }
}

/// The bounded slice of a body this panel draws, cut on a character boundary.
/// The stored message is untouched; the row states the bound whenever it bites.
fn display_body(body: &str) -> (String, i64) {
    if body.len() <= BODY_DISPLAY_BYTES {
        return (body.to_owned(), count_i64(body.len()));
    }
    let mut end = BODY_DISPLAY_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    (body[..end].to_owned(), count_i64(end))
}

/// A message's references as one bounded line. Structured in the record and
/// rendered here — a row is not the place to spell out a kilobyte of link.
fn render_references(references: &[Reference]) -> String {
    let shown: Vec<String> = references
        .iter()
        .take(REFERENCES_SHOWN)
        .map(render_reference)
        .collect();
    let hidden = references.len().saturating_sub(shown.len());
    let line = shown.join(" · ");
    if hidden == 0 {
        return line;
    }
    format!("{line} · +{hidden} more")
}

fn render_reference(reference: &Reference) -> String {
    match reference {
        Reference::Commit { repo, commit } => format!("commit {repo}@{commit}"),
        Reference::Blob { hash } => format!("blob {hash}"),
        Reference::Duck { url } => clip(url, REFERENCE_CHARS),
    }
}

fn clip(value: &str, chars: usize) -> String {
    if value.chars().count() <= chars {
        return value.to_owned();
    }
    let head: String = value.chars().take(chars).collect();
    format!("{head}…")
}

/// Where a page starts: the tail when the reader asked for the newest, and
/// exactly what they asked for otherwise — a cursor below the retained floor
/// is NOT clamped up to it, because the module's own history-gap answer is the
/// honest one and clamping would hide the gap.
fn page_start(from_seq: i64, newest: bool, conversation: &Conversation) -> u64 {
    if newest {
        return conversation
            .next_seq
            .saturating_sub(PAGE)
            .max(conversation.floor_seq);
    }
    u64::try_from(from_seq).unwrap_or_default()
}

fn seats(conversation: &Conversation, participant: &str) -> Vec<MessagingSeat> {
    conversation
        .roster
        .iter()
        .map(|(seat, role)| MessagingSeat {
            participant: seat.clone(),
            role: match role {
                Role::Member => "member",
                Role::Observer => "observer",
            }
            .to_owned(),
            you: seat == participant,
        })
        .collect()
}

fn binding_view(binding: Option<&BindingView>) -> MessagingBinding {
    let Some(binding) = binding else {
        return MessagingBinding::default();
    };
    MessagingBinding {
        present: true,
        device: binding.device.clone(),
        credential: binding.credential.to_string(),
        detached: binding.detached,
    }
}

fn refused(view: &MessagingView, reason: DenyReason) -> MessagingView {
    MessagingView {
        denied: deny_token(reason).to_owned(),
        answered: true,
        ..view.clone()
    }
}

/// The module's own refusal vocabulary, as the stable tokens the panel shows
/// and the guest branches on — the spellings its wire enum serializes to.
fn deny_token(reason: DenyReason) -> &'static str {
    match reason {
        DenyReason::Unauthenticated => "unauthenticated",
        DenyReason::NotReader => "not_reader",
        DenyReason::NotPermitted => "not_permitted",
    }
}

fn unexpected(what: &str, reply: &CollaborationReply) -> String {
    format!("the collaboration module answered {reply:?} to {what}")
}

// ---- sending ----------------------------------------------------------------

/// Send one message as the participant's OWNER — the person at this device.
///
/// The credential is the participant's `owner_credential`, never a binding's:
/// a scoped service key belongs to an attached agent service and never reaches
/// this app. Everything the module needs and a person cannot supply — the
/// credential, the sequence, the deadline in the network's own time unit — is
/// resolved here, from the network.
/// Infallible at this boundary, like [`load_messaging`] and for the same
/// reason: the outcome carries the SCOPE it happened in, and a `Result`'s error
/// arm cannot. A refusal that arrives without its scope has nowhere safe to be
/// shown once the reader has moved on.
#[allow(clippy::too_many_arguments)]
pub async fn send_agent_message(
    rpc: String,
    network: String,
    link: i64,
    account: String,
    op: i64,
    participant: String,
    conversation: String,
    kind: String,
    recipient: String,
    body: String,
    reply_to: i64,
    password: String,
) -> MessagingSend {
    let scope = MessagingSend {
        rpc: rpc.clone(),
        network: network.clone(),
        link,
        account,
        op,
        participant: participant.clone(),
        conversation: conversation.clone(),
        refusal: String::new(),
    };
    match submit_message(
        rpc,
        network,
        participant,
        conversation,
        kind,
        recipient,
        body,
        reply_to,
        password,
    )
    .await
    {
        Ok(()) => scope,
        Err(refusal) => MessagingSend { refusal, ..scope },
    }
}

#[allow(clippy::too_many_arguments)]
async fn submit_message(
    rpc: String,
    network: String,
    participant: String,
    conversation: String,
    kind: String,
    recipient: String,
    body: String,
    reply_to: i64,
    password: String,
) -> Result<(), String> {
    let (client, identity) = reader_client(&rpc, &network).await?;
    let record = match read(&client, &participant, ProtectedRead::Participant).await? {
        CollaborationReply::Participant(record) => record,
        CollaborationReply::Denied(reason) => {
            return Err(format!(
                "this device's key may not act as participant {participant} ({})",
                deny_token(reason)
            ));
        }
        other => return Err(unexpected("a participant record", &other)),
    };
    if record.revoked {
        return Err(format!(
            "participant {participant} is revoked and admits nothing"
        ));
    }
    let mut outbox = Outbox::open(&network)?;
    let request = compose(
        &client,
        &mut outbox,
        &record,
        &conversation,
        &kind,
        &recipient,
        &body,
        reply_to,
    )
    .await?;
    // NOTHING IS SIGNED UNDER AN IDENTITY THIS SESSION NO LONGER HOLDS.
    // Composing took several round trips; a Lock or an account switch during
    // them must stop the send, not send it as whoever is seated now.
    still_signing_as(&identity).await?;
    // THE OUTBOX IS WRITTEN BEFORE THE SUBMIT. A send whose answer never
    // arrives is ambiguous, and the only safe retry is the SAME id over the
    // SAME bytes — which is only possible if they were durable first.
    outbox.stage(&record.id, &request)?;
    let payload = collaboration::encode_msg(&CollaborationMsg::Send(request.clone()));
    match signed_write(&client, COLLABORATION, payload, password).await {
        Ok(_) => outbox.settle(&record.id, request.message_id),
        // LEFT PENDING ON PURPOSE. A refusal we can read and an answer that
        // never came look the same from here, so the entry stays: the next send
        // reconciles it against the network's own `SendState` and either
        // retries these exact bytes or frees the sequence.
        Err(refusal) => Err(refusal),
    }
}

/// The exact request to submit: a pending one retried byte-for-byte, or a
/// fresh one under a newly allocated sequence.
#[allow(clippy::too_many_arguments)]
async fn compose(
    client: &RpcClient,
    outbox: &mut Outbox,
    record: &Participant,
    conversation: &str,
    kind: &str,
    recipient: &str,
    body: &str,
    reply_to: i64,
) -> Result<SendRequest, String> {
    let kind = message_kind(kind)?;
    if body.len() > collaboration::MAX_BODY_BYTES {
        return Err(format!(
            "this message is {} bytes; the network admits at most {} — shorten it",
            body.len(),
            collaboration::MAX_BODY_BYTES
        ));
    }
    outbox
        .reconcile(client, &record.id, record.owner_credential)
        .await?;
    // A RETRY REUSES ITS ID AND ITS BYTES. Pressing send again after an
    // ambiguous failure must not mint a second message: the module answers a
    // byte-identical retry with the original receipt, and refuses different
    // bytes under that id rather than admitting a duplicate.
    let repeat = outbox.pending_match(
        &record.id,
        record.owner_credential,
        conversation,
        recipient,
        kind,
        body,
        reply_to,
    );
    if let Some(request) = repeat {
        return Ok(request);
    }
    let sequence = allocate_sequence(client, record, outbox).await?;
    Ok(SendRequest {
        conversation_id: conversation.to_owned(),
        sender_participant_id: record.id.clone(),
        message_id: MessageId {
            generation: record.owner_credential,
            sequence,
        },
        recipient_participant_id: recipient.to_owned(),
        kind,
        reply_to: u64::try_from(reply_to).ok().filter(|seq| *seq > 0),
        // A task update names a task and the attempt it addresses. That pair is
        // an attached service's to hold, so this panel composes no task
        // reference at all rather than inventing one.
        task: None,
        body: body.to_owned(),
        references: Vec::new(),
        expires_at: deadline(client).await?,
    })
}

fn message_kind(kind: &str) -> Result<MessageKind, String> {
    match kind {
        "notice" => Ok(MessageKind::Notice),
        "question" => Ok(MessageKind::Question),
        "task_request" => Ok(MessageKind::TaskRequest),
        "result" => Ok(MessageKind::Result),
        // `task_update` is absent by construction: it MUST name a task and the
        // attempt it addresses, and this panel holds neither.
        other => Err(format!("{other} is not a kind this panel sends")),
    }
}

fn kind_token(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Notice => "notice",
        MessageKind::Question => "question",
        MessageKind::TaskRequest => "task_request",
        MessageKind::TaskUpdate => "task_update",
        MessageKind::Result => "result",
    }
}

/// The absolute deadline this message carries, in the NETWORK's own
/// `consensus_time` unit.
///
/// The unit is REQUIRED, never defaulted: reading a millisecond lane as a
/// height lane turns a 24-hour intent into 86 seconds, and picking wrong is
/// the whole bug this read exists to avoid. The sum is checked, because a
/// wrapped deadline is one in the PAST — a message that expires the moment it
/// lands, from a panel that looked like it worked.
async fn deadline(client: &RpcClient) -> Result<u64, String> {
    let status = client
        .status_json()
        .await
        .map_err(|error| error.to_string())?;
    let now = status["consensus_time"]
        .as_u64()
        .ok_or("this node's status carries no consensus_time")?;
    let ttl = match status["consensus_time_unit"].as_str() {
        // the height lane counts BLOCKS; one block is one second only while
        // the chain heartbeats at that rate, so this mapping is nominal
        Some("height") => DELIVERY_TTL_SECS,
        Some("millis") => DELIVERY_TTL_SECS.saturating_mul(1_000),
        _ => {
            return Err(
                "this node's status names no consensus_time_unit — an unnamed unit \
                        cannot be scaled into"
                    .into(),
            );
        }
    };
    now.checked_add(ttl)
        .ok_or_else(|| "this network's clock is past the end of a delivery deadline".to_string())
}

/// The smallest sequence this credential has not used.
///
/// The network is the authority, not a local counter: this participant's owner
/// credential is the SAME credential on every device the person's key sits on,
/// so a counter that was lost, restored from a backup, or is being kept by a
/// second app would collide. The walk is exponential-then-binary over the
/// module's own `SendState`, raised by the outbox's high-water mark so an
/// ambiguous send's sequence is never handed out twice.
///
/// A concurrent sender can still take the sequence between this probe and the
/// submit. That ends in the module's explicit refusal ("admitted with different
/// bytes"), never in a silently overwritten message.
async fn allocate_sequence(
    client: &RpcClient,
    record: &Participant,
    outbox: &Outbox,
) -> Result<u64, String> {
    let generation = record.owner_credential;
    let mut probes = 0u32;
    // grow until a free sequence is found: everything below `low` is taken
    let mut low = 1u64;
    let mut high = 1u64;
    while used(client, record, generation, high).await? {
        probes += 1;
        if probes > MAX_SEQUENCE_PROBES {
            return Err(SEQUENCE_EXHAUSTED.into());
        }
        // A CREDENTIAL WHOSE SPACE IS FULL HAS NO NEXT SEQUENCE. Wrapping to 0
        // here would hand out a sequence that is certainly taken, so the walk
        // refuses instead of overflowing.
        low = high.checked_add(1).ok_or(SEQUENCE_EXHAUSTED)?;
        high = high.checked_mul(2).unwrap_or(u64::MAX);
    }
    // narrow onto the first free one in [low, high]
    while low < high {
        probes += 1;
        if probes > MAX_SEQUENCE_PROBES {
            return Err(SEQUENCE_EXHAUSTED.into());
        }
        let middle = low + (high - low) / 2;
        match used(client, record, generation, middle).await? {
            true => low = middle.checked_add(1).ok_or(SEQUENCE_EXHAUSTED)?,
            false => high = middle,
        }
    }
    let floor = outbox
        .high_water(&record.id, generation)
        .checked_add(1)
        .ok_or(SEQUENCE_EXHAUSTED)?;
    Ok(low.max(floor))
}

/// Whether one sequence is spoken for: admitted, or pruned below the replay
/// floor (which can never become a new admission either).
async fn used(
    client: &RpcClient,
    record: &Participant,
    generation: u64,
    sequence: u64,
) -> Result<bool, String> {
    match read(
        client,
        &record.id,
        ProtectedRead::SendState {
            generation,
            sequence,
        },
    )
    .await?
    {
        CollaborationReply::SendState(SendState::Absent) => Ok(false),
        CollaborationReply::SendState(_) => Ok(true),
        CollaborationReply::Denied(reason) => Err(format!(
            "this device's key may not act as participant {} ({})",
            record.id,
            deny_token(reason)
        )),
        other => Err(unexpected("a send state", &other)),
    }
}

// ---- the outbox -------------------------------------------------------------

/// The largest outbox this app will read. A pending entry carries a whole
/// `SendRequest`, so `MAX_OUTBOX_PENDING` bodies at `MAX_BODY_BYTES` plus JSON
/// overhead is the honest ceiling; anything past it is not an outbox this app
/// wrote, and it is refused rather than parsed.
const MAX_OUTBOX_BYTES: usize = 4 * 1024 * 1024;

/// Ambiguous sends this device tracks at once. Each one costs a `SendState`
/// round trip on the NEXT send's reconcile, so an unbounded list is an
/// unbounded reconcile.
const MAX_OUTBOX_PENDING: usize = 64;

/// Participant+credential pairs whose high-water mark this device remembers.
const MAX_OUTBOX_MARKS: usize = 256;

/// Take the outbox's exclusive advisory lock, or refuse.
///
/// Non-blocking on purpose: this runs on the async executor, a blocking
/// `flock` would park the whole runtime behind another process's send, and
/// "another window is sending as this device — try again" is a true sentence a
/// person can act on. The lock is a SEPARATE file because the outbox itself is
/// replaced by rename, which would hand the next opener a lock on an unlinked
/// inode.
fn lock_exclusive(path: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::unix::io::AsRawFd as _;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| {
            format!(
                "the outbox lock at {} is not writable: {error}",
                path.display()
            )
        })?;
    // SAFETY: a live fd this function owns, and an operation that only takes an
    // advisory lock on it.
    let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if taken != 0 {
        let error = std::io::Error::last_os_error();
        let busy = matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        );
        return Err(match busy {
            true => "another window or device process is sending as this device right now — \
                     wait for it to finish and send again"
                .to_owned(),
            false => format!(
                "this device's outbox could not be locked ({error}); nothing is sent \
                              while its state is unknown"
            ),
        });
    }
    Ok(file)
}

/// The outbox's bytes as state, or the reason this device will not send.
fn read_outbox(bytes: &[u8], network: &str, path: &std::path::Path) -> Result<OutboxFile, String> {
    let unreadable = |why: String| {
        format!(
            "this device's outbox at {} is unusable ({why}); it may hold sends whose outcome is \
             unknown, so nothing is sent until it is repaired or removed",
            path.display()
        )
    };
    if bytes.len() > MAX_OUTBOX_BYTES {
        return Err(unreadable(format!(
            "{} bytes, past the {MAX_OUTBOX_BYTES} this app writes",
            bytes.len()
        )));
    }
    let state: OutboxFile =
        serde_json::from_slice(bytes).map_err(|error| unreadable(error.to_string()))?;
    if state.chain_id != network {
        return Err(unreadable(format!(
            "it names chain {} and this node is on {network}",
            state.chain_id
        )));
    }
    if state.pending.len() > MAX_OUTBOX_PENDING {
        return Err(unreadable(format!(
            "{} pending sends, past the {MAX_OUTBOX_PENDING} this app stages",
            state.pending.len()
        )));
    }
    if state.high_water.len() > MAX_OUTBOX_MARKS {
        return Err(unreadable(format!(
            "{} high-water marks, past the {MAX_OUTBOX_MARKS} this app keeps",
            state.high_water.len()
        )));
    }
    Ok(state)
}

/// One message this device staged but has not seen settled.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Pending {
    /// the participant that staged it — a field, not a map key, because a
    /// participant id is any 1..=128 bytes and is not safe to encode into one
    participant: String,
    /// the WHOLE request, so a retry submits the same canonical bytes. A
    /// recomputed deadline would be different bytes under the same id, which
    /// the module refuses outright rather than admitting twice.
    request: SendRequest,
}

/// This device's record of what it has sent on ONE network, and what it is not
/// sure about.
///
/// It exists because an owner credential is shared: the same key on a second
/// device (or a second window here) draws from the same sequence space, so a
/// purely local counter is wrong the moment anything else sends. This file is
/// therefore a memory of AMBIGUITY, not a sequence allocator — the allocator
/// asks the network and only takes this as a floor.
struct Outbox {
    path: PathBuf,
    state: OutboxFile,
    /// The exclusive advisory lock, held from [`Outbox::open`] until this value
    /// is dropped — which is after the submit and its settle. Read-modify-write
    /// plus a network sequence probe is not atomic, so without it a second app
    /// process interleaves between the probe and the stage and both take the
    /// same sequence. Dropping the file releases the lock.
    _lock: std::fs::File,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct OutboxFile {
    #[serde(default)]
    chain_id: String,
    /// participant + credential -> the highest sequence this device staged
    #[serde(default)]
    high_water: Vec<HighWater>,
    #[serde(default)]
    pending: Vec<Pending>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct HighWater {
    participant: String,
    credential: u64,
    sequence: u64,
}

impl Outbox {
    /// This network's outbox, locked for the caller.
    ///
    /// The file is named by a DIGEST of the chain id: a chain id is not a path
    /// component, and an empty one is refused rather than used as a namespace —
    /// an empty fence is not a fence.
    ///
    /// EVERY failure to read an EXISTING outbox refuses the send. Only a file
    /// that is not there may start an empty one: a permission error, a short
    /// read, unparsable bytes or a chain id that is not this one all mean this
    /// device may be holding ambiguous sends it cannot see, and starting empty
    /// would re-allocate their sequences and mint duplicates.
    fn open(network: &str) -> Result<Self, String> {
        if network.is_empty() {
            return Err(
                "this node has not named its network yet — an unnamed chain is not a \
                        scope to send under"
                    .into(),
            );
        }
        let digest = hex_encode(&Sha256::digest(network.as_bytes()));
        let directory = super::app_dirs::state_dir()?.join("messaging");
        std::fs::create_dir_all(&directory).map_err(|error| {
            format!(
                "the outbox directory {} is not writable: {error}",
                directory.display()
            )
        })?;
        let path = directory.join(format!("outbox-{}.json", &digest[..16]));
        let lock = lock_exclusive(&path.with_extension("lock"))?;
        let state = match std::fs::read(&path) {
            Ok(bytes) => read_outbox(&bytes, network, &path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => OutboxFile {
                chain_id: network.to_owned(),
                ..OutboxFile::default()
            },
            Err(error) => {
                return Err(format!(
                    "this device's outbox at {} could not be read ({error}); it may hold sends \
                     whose outcome is unknown, so nothing is sent until it can be",
                    path.display()
                ));
            }
        };
        Ok(Self {
            path,
            state,
            _lock: lock,
        })
    }

    fn high_water(&self, participant: &str, credential: u64) -> u64 {
        self.state
            .high_water
            .iter()
            .find(|mark| mark.participant == participant && mark.credential == credential)
            .map(|mark| mark.sequence)
            .unwrap_or_default()
    }

    /// The pending request that IS this draft, if one is outstanding: same
    /// conversation, recipient, kind, body and reply target. Its bytes are
    /// reused whole, deadline included.
    #[allow(clippy::too_many_arguments)]
    fn pending_match(
        &self,
        participant: &str,
        credential: u64,
        conversation: &str,
        recipient: &str,
        kind: MessageKind,
        body: &str,
        reply_to: i64,
    ) -> Option<SendRequest> {
        let reply_to = u64::try_from(reply_to).ok().filter(|seq| *seq > 0);
        self.state
            .pending
            .iter()
            .find(|entry| {
                let request = &entry.request;
                entry.participant == participant
                    && request.message_id.generation == credential
                    && request.conversation_id == conversation
                    && request.recipient_participant_id == recipient
                    && request.kind == kind
                    && request.body == body
                    && request.reply_to == reply_to
            })
            .map(|entry| entry.request.clone())
    }

    /// Record a request BEFORE it is submitted, durably.
    fn stage(&mut self, participant: &str, request: &SendRequest) -> Result<(), String> {
        let staged = Pending {
            participant: participant.to_owned(),
            request: request.clone(),
        };
        let already = self.state.pending.iter().any(|entry| {
            entry.participant == participant && entry.request.message_id == request.message_id
        });
        if !already {
            // AN OUTBOX THAT ONLY GROWS IS A RECONCILE THAT ONLY GROWS. Every
            // entry costs a `SendState` round trip on the next send, and this
            // many unresolved sends means something upstream is wrong — say so
            // rather than staging one more onto a pile nothing drains.
            if self.state.pending.len() >= MAX_OUTBOX_PENDING {
                return Err(format!(
                    "this device is already holding {MAX_OUTBOX_PENDING} sends whose outcome it \
                     could not resolve — reconnect and let them settle before sending more"
                ));
            }
            self.state.pending.push(staged);
        }
        self.raise(participant, request.message_id)?;
        self.write()
    }

    /// Retire a request whose admission this device saw.
    fn settle(&mut self, participant: &str, id: MessageId) -> Result<(), String> {
        self.state
            .pending
            .retain(|entry| !(entry.participant == participant && entry.request.message_id == id));
        self.raise(participant, id)?;
        self.write()
    }

    fn raise(&mut self, participant: &str, id: MessageId) -> Result<(), String> {
        let mark = self
            .state
            .high_water
            .iter_mut()
            .find(|mark| mark.participant == participant && mark.credential == id.generation);
        match mark {
            Some(mark) => mark.sequence = mark.sequence.max(id.sequence),
            None => {
                if self.state.high_water.len() >= MAX_OUTBOX_MARKS {
                    return Err(format!(
                        "this device's outbox already tracks {MAX_OUTBOX_MARKS} credentials on \
                         this network — remove it to start a fresh one"
                    ));
                }
                self.state.high_water.push(HighWater {
                    participant: participant.to_owned(),
                    credential: id.generation,
                    sequence: id.sequence,
                });
            }
        }
        Ok(())
    }

    /// Ask the NETWORK what became of every ambiguous send of this
    /// participant, and keep only the ones still unresolved.
    ///
    /// `Admitted` means it landed after all — the retry that would have
    /// duplicated it is exactly what this prevents. `Absent` means it never
    /// did; the entry stays only while its deadline can still be met, because
    /// a request whose `expires_at` has passed can never be admitted with
    /// these bytes and retrying it forever would be a stuck outbox.
    async fn reconcile(
        &mut self,
        client: &RpcClient,
        participant: &str,
        credential: u64,
    ) -> Result<(), String> {
        let mine: Vec<SendRequest> = self
            .state
            .pending
            .iter()
            .filter(|entry| {
                entry.participant == participant
                    && entry.request.message_id.generation == credential
            })
            .map(|entry| entry.request.clone())
            .collect();
        if mine.is_empty() {
            return Ok(());
        }
        let now = self.now(client).await?;
        let mut settled: Vec<MessageId> = Vec::new();
        for request in &mine {
            let state = match read(
                client,
                participant,
                ProtectedRead::SendState {
                    generation: request.message_id.generation,
                    sequence: request.message_id.sequence,
                },
            )
            .await?
            {
                CollaborationReply::SendState(state) => state,
                CollaborationReply::Denied(reason) => {
                    return Err(format!(
                        "this device's key may not act as participant {participant} ({})",
                        deny_token(reason)
                    ));
                }
                other => return Err(unexpected("a send state", &other)),
            };
            let resolved = match state {
                SendState::Admitted { .. } | SendState::ReceiptPruned => true,
                SendState::Absent => request.expires_at <= now,
            };
            if resolved {
                settled.push(request.message_id);
            }
        }
        for id in settled {
            self.settle(participant, id)?;
        }
        Ok(())
    }

    async fn now(&self, client: &RpcClient) -> Result<u64, String> {
        let status = client
            .status_json()
            .await
            .map_err(|error| error.to_string())?;
        status["consensus_time"]
            .as_u64()
            .ok_or_else(|| "this node's status carries no consensus_time".to_string())
    }

    /// Write the file so a crash leaves either the old contents or the new
    /// ones: a torn outbox is a lost id, and a lost id is a duplicate message.
    fn write(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or("the messaging outbox has no directory")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(&self.state).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.new");
        {
            use std::io::Write as _;
            let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
            file.write_all(&bytes).map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
        }
        std::fs::rename(&temporary, &self.path).map_err(|error| error.to_string())?;
        // THE RENAME ITSELF HAS TO REACH THE DISK, or the new file can be
        // present with no directory entry naming it. This error is raised, not
        // swallowed: a write that returns `Ok` is what tells the caller it may
        // submit, and submitting on a record that might not survive a crash is
        // exactly the duplicate this file exists to prevent.
        let directory = std::fs::File::open(parent)
            .map_err(|error| format!("the outbox directory did not open to sync: {error}"))?;
        directory
            .sync_all()
            .map_err(|error| format!("the outbox rename did not reach the disk: {error}"))
    }
}

fn count_i64_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    //! The panel's host half: the scope fence, the display bounds it applies
    //! without touching a stored message, and the outbox that makes a retry the
    //! SAME message instead of a second one.
    //!
    //! These run against the REAL `collaboration` types — the same `SendRequest`
    //! and `Message` the module admits — so a wire change breaks them here
    //! rather than on a live network.

    use super::*;
    use collaboration::{Message, MessageId, MessageKind, Reference, SendRequest, TaskRef};

    fn request(sequence: u64, body: &str, expires_at: u64) -> SendRequest {
        SendRequest {
            conversation_id: "standup".into(),
            sender_participant_id: "claude-a".into(),
            message_id: MessageId {
                generation: 4,
                sequence,
            },
            recipient_participant_id: "codex-b".into(),
            kind: MessageKind::Question,
            reply_to: None,
            task: None,
            body: body.into(),
            references: Vec::new(),
            expires_at,
        }
    }

    fn admitted(body: &str, references: Vec<Reference>, task: Option<TaskRef>) -> Message {
        Message {
            seq: 7,
            message_id: MessageId {
                generation: 4,
                sequence: 1,
            },
            conversation_id: "standup".into(),
            sender: "codex-b".into(),
            recipient: "claude-a".into(),
            kind: MessageKind::Question,
            reply_to: None,
            task,
            body: body.into(),
            references,
            expires_at: 90_000,
            admitted_at: 41,
            digest: "00".repeat(32),
        }
    }

    /// An outbox over a temporary directory, holding its own lock — the shape
    /// `Outbox::open` produces, without the app's real state directory.
    fn held(directory: &tempfile::TempDir, state: OutboxFile) -> Outbox {
        let path = directory.path().join("outbox.json");
        let lock = lock_exclusive(&path.with_extension("lock")).expect("a fresh lock");
        Outbox {
            path,
            state,
            _lock: lock,
        }
    }

    /// A reading belongs to ONE endpoint, chain, link, account, operation,
    /// participant and conversation. Anything else is an answer about something
    /// the reader has left.
    #[test]
    fn a_reading_installs_only_in_the_scope_it_was_read_in() {
        let view = MessagingView {
            rpc: "http://node".into(),
            network: "duck-1".into(),
            link: 4,
            account: "7".into(),
            op: 11,
            participant: "claude-a".into(),
            conversation: "standup".into(),
            ..MessagingView::default()
        };
        let live = |rpc, network, link, account, op, participant, conversation| {
            messaging_in_scope(
                &view,
                rpc,
                network,
                link,
                account,
                op,
                participant,
                conversation,
            )
        };
        assert!(live(
            "http://node",
            "duck-1",
            4,
            "7",
            11,
            "claude-a",
            "standup"
        ));
        // every way a late answer stops being about what is on screen
        assert!(!live(
            "http://other",
            "duck-1",
            4,
            "7",
            11,
            "claude-a",
            "standup"
        ));
        assert!(!live(
            "http://node",
            "duck-2",
            4,
            "7",
            11,
            "claude-a",
            "standup"
        ));
        assert!(!live(
            "http://node",
            "duck-1",
            4,
            "7",
            11,
            "codex-b",
            "standup"
        ));
        assert!(!live(
            "http://node",
            "duck-1",
            4,
            "7",
            11,
            "claude-a",
            "release"
        ));
        // A RECONNECT TO THE SAME PLACE IS A DIFFERENT LINK. Every id above
        // still matches; the session this answer was read over does not exist.
        assert!(!live(
            "http://node",
            "duck-1",
            5,
            "7",
            11,
            "claude-a",
            "standup"
        ));
        // A SEAT CHANGE READS AS SOMEONE ELSE, at the same endpoint and chain.
        assert!(!live(
            "http://node",
            "duck-1",
            4,
            "8",
            11,
            "claude-a",
            "standup"
        ));
        // THE A -> B -> A RETURN. The reader left standup, came back, and the
        // panel read it again: the ids are identical and only the operation
        // number tells the first answer from the second.
        assert!(!live(
            "http://node",
            "duck-1",
            4,
            "7",
            12,
            "claude-a",
            "standup"
        ));
    }

    /// A send's outcome carries its own scope for the same reason: a refusal about
    /// conversation A must not surface under B, and an admission for A must not
    /// clear a draft written for B.
    #[test]
    fn a_send_outcome_installs_only_in_the_scope_it_was_sent_in() {
        let sent = MessagingSend {
            rpc: "http://node".into(),
            network: "duck-1".into(),
            link: 4,
            account: "7".into(),
            op: 2,
            participant: "claude-a".into(),
            conversation: "standup".into(),
            refusal: String::new(),
        };
        let live = |link, account, op, conversation| {
            messaging_send_in_scope(
                &sent,
                "http://node",
                "duck-1",
                link,
                account,
                op,
                "claude-a",
                conversation,
            )
        };
        assert!(live(4, "7", 2, "standup"));
        assert!(!live(4, "7", 2, "release"));
        assert!(!live(5, "7", 2, "standup"));
        assert!(!live(4, "8", 2, "standup"));
        // THE ADMISSION THAT WOULD CLEAR A NEWER DRAFT. This outcome is an
        // admission (`refusal` is empty), and installing it bumps the counter
        // the composer clears its draft on. Send #2 finishing after send #3 was
        // written must not throw #3's text away.
        assert!(!live(4, "7", 3, "standup"));
    }

    /// The panel bounds what it DRAWS. The stored message is not altered, and the
    /// row carries both numbers so the reader can see the bound bite.
    #[test]
    fn a_long_body_is_bounded_for_display_on_a_character_boundary() {
        let short = admitted("short enough", Vec::new(), None);
        let row = message_row(&short, Err("no receipt".into()), "claude-a");
        assert_eq!(row.body, "short enough");
        assert_eq!(row.body_bytes, 12);
        assert_eq!(row.shown_bytes, 12);

        // a multi-byte character straddling the cut must not be split in half
        let long = admitted(&"é".repeat(2000), Vec::new(), None);
        let row = message_row(&long, Err("no receipt".into()), "claude-a");
        assert_eq!(
            row.body_bytes, 4000,
            "the stored body's own size is reported"
        );
        assert!(row.shown_bytes < row.body_bytes, "the display bound bit");
        assert!(row.shown_bytes <= 1024);
        assert!(
            row.body.chars().all(|character| character == 'é'),
            "the slice is valid UTF-8 and unaltered"
        );
        assert_eq!(row.body.len() as i64, row.shown_bytes);
    }

    /// An unread receipt is UNKNOWN. `stored` is the most reassuring of the seven
    /// states, which makes defaulting to it the exact wrong guess.
    #[test]
    fn an_unresolved_receipt_leaves_the_delivery_state_empty() {
        let row = message_row(
            &admitted("hello", Vec::new(), None),
            Err("the read failed".into()),
            "claude-a",
        );
        assert_eq!(row.delivery, "");
        assert_eq!(row.delivery_reason, "");
    }

    /// References are structured in the record and bounded in the row: a
    /// `duck://` link may be a kilobyte, and 16 of them may not fill a screen.
    #[test]
    fn references_are_rendered_and_bounded() {
        let many: Vec<Reference> = (0..6)
            .map(|_| Reference::Blob {
                hash: "ab".repeat(32),
            })
            .collect();
        let row = message_row(&admitted("x", many, None), Err("none".into()), "claude-a");
        assert!(row.references.contains("blob abab"), "{}", row.references);
        assert!(row.references.ends_with("+2 more"), "{}", row.references);

        let long = vec![Reference::Duck {
            url: format!("duck://{}", "a".repeat(900)),
        }];
        let row = message_row(&admitted("x", long, None), Err("none".into()), "claude-a");
        assert!(row.references.ends_with('…'), "{}", row.references);
        assert!(row.references.chars().count() <= 161, "{}", row.references);
    }

    /// A task reference is carried whole — its id AND the attempt the sender
    /// addressed. The panel resolves neither into an execution state.
    #[test]
    fn a_task_reference_carries_the_attempt_it_was_sent_for() {
        let row = message_row(
            &admitted(
                "please review",
                Vec::new(),
                Some(TaskRef {
                    id: "job-19".into(),
                    expected_attempt: 2,
                }),
            ),
            Err("none".into()),
            "claude-a",
        );
        assert_eq!(row.task, "job-19");
        assert_eq!(row.task_attempt, 2);
    }

    /// The module's refusal vocabulary reaches the screen as the module's own
    /// tokens — the guest branches on these spellings.
    #[test]
    fn a_denial_keeps_the_modules_own_token() {
        assert_eq!(
            deny_token(collaboration::DenyReason::Unauthenticated),
            "unauthenticated"
        );
        assert_eq!(
            deny_token(collaboration::DenyReason::NotReader),
            "not_reader"
        );
        assert_eq!(
            deny_token(collaboration::DenyReason::NotPermitted),
            "not_permitted"
        );
    }

    /// Pressing send again after an ambiguous failure must reuse the message id
    /// AND the exact bytes — deadline included. Recomputing the deadline would be
    /// different bytes under the same id, which the module refuses outright; a
    /// fresh id would be a second message.
    #[test]
    fn a_retry_of_the_same_draft_reuses_the_pending_id_and_bytes() {
        let pending = request(8, "ship it", 90_000);
        let directory = tempfile::tempdir().expect("a temporary state directory");
        let outbox = held(
            &directory,
            OutboxFile {
                chain_id: "duck-1".into(),
                high_water: vec![HighWater {
                    participant: "claude-a".into(),
                    credential: 4,
                    sequence: 8,
                }],
                pending: vec![Pending {
                    participant: "claude-a".into(),
                    request: pending.clone(),
                }],
            },
        );
        let matched = outbox
            .pending_match(
                "claude-a",
                4,
                "standup",
                "codex-b",
                MessageKind::Question,
                "ship it",
                0,
            )
            .expect("the same draft finds its pending request");
        assert_eq!(
            matched, pending,
            "the whole request is reused, deadline included"
        );

        // a CHANGED draft is a different message and must not ride the pending id
        assert!(
            outbox
                .pending_match(
                    "claude-a",
                    4,
                    "standup",
                    "codex-b",
                    MessageKind::Question,
                    "ship it now",
                    0,
                )
                .is_none()
        );
        // and neither may another conversation's draft
        assert!(
            outbox
                .pending_match(
                    "claude-a",
                    4,
                    "release",
                    "codex-b",
                    MessageKind::Question,
                    "ship it",
                    0,
                )
                .is_none()
        );
        assert_eq!(outbox.high_water("claude-a", 4), 8);
        assert_eq!(
            outbox.high_water("claude-a", 5),
            0,
            "another credential is its own space"
        );
    }

    /// An empty chain id is refused, never used as a namespace: an empty fence is
    /// not a fence, and every unnamed network would share one outbox.
    #[test]
    fn an_unnamed_network_has_no_outbox() {
        let refused = Outbox::open("").expect_err("an unnamed chain is refused");
        assert!(refused.contains("has not named its network"), "{refused}");
    }

    /// An outbox this app cannot read is a set of sends whose outcome it cannot
    /// see. Starting empty would re-allocate their sequences and mint duplicates,
    /// so every failure to read an EXISTING file refuses the send instead.
    #[test]
    fn an_unreadable_outbox_refuses_the_send_rather_than_starting_empty() {
        let path = std::path::Path::new("/state/messaging/outbox-abc.json");
        let good = serde_json::to_vec(&OutboxFile {
            chain_id: "duck-1".into(),
            ..OutboxFile::default()
        })
        .expect("json");
        assert!(read_outbox(&good, "duck-1", path).is_ok());

        // truncated, half-written, or someone else's file entirely
        let torn = read_outbox(b"{\"chain_id\":\"duck-", "duck-1", path)
            .expect_err("unparsable bytes are not an empty outbox");
        assert!(torn.contains("unusable"), "{torn}");

        // THE PATH IS A DIGEST OF THE CHAIN ID, so a mismatch is a collision or
        // a tampered file — not a reason to overwrite what is there.
        let elsewhere = serde_json::to_vec(&OutboxFile {
            chain_id: "duck-2".into(),
            ..OutboxFile::default()
        })
        .expect("json");
        let moved = read_outbox(&elsewhere, "duck-1", path)
            .expect_err("another chain's outbox is not this one's");
        assert!(moved.contains("duck-2"), "{moved}");

        let huge = vec![b'x'; MAX_OUTBOX_BYTES + 1];
        let refused = read_outbox(&huge, "duck-1", path).expect_err("an oversized file is refused");
        assert!(refused.contains("past the"), "{refused}");

        // an unbounded pending list is an unbounded reconcile on the next send
        let piled = serde_json::to_vec(&OutboxFile {
            chain_id: "duck-1".into(),
            pending: (0..=MAX_OUTBOX_PENDING)
                .map(|seq| Pending {
                    participant: "claude-a".into(),
                    request: request(seq as u64 + 1, "x", 90_000),
                })
                .collect(),
            ..OutboxFile::default()
        })
        .expect("json");
        let refused = read_outbox(&piled, "duck-1", path).expect_err("too many pending is refused");
        assert!(refused.contains("pending sends"), "{refused}");
    }

    /// Two app processes share one state directory and one owner credential, and
    /// read-modify-probe-submit is not atomic. The second one is told to wait
    /// rather than allowed to allocate against a sequence space the first is in
    /// the middle of taking from.
    #[test]
    fn a_second_holder_of_the_outbox_is_refused_rather_than_queued() {
        let directory = tempfile::tempdir().expect("a temporary state directory");
        let path = directory.path().join("outbox.lock");
        let first = lock_exclusive(&path).expect("the first holder takes the lock");
        let refused = lock_exclusive(&path).expect_err("the second is refused, not blocked");
        assert!(
            refused.contains("sending as this device right now"),
            "{refused}"
        );
        // and the lock is released with the file, so the next opener gets it
        drop(first);
        lock_exclusive(&path).expect("released with its holder");
    }

    /// The staged entry survives a settle of a DIFFERENT id, and leaves on its
    /// own: a retry window that closed on the wrong entry would resend a message
    /// that already landed.
    #[test]
    fn settling_one_id_leaves_the_others_pending() {
        let directory = tempfile::tempdir().expect("a temporary state directory");
        let mut outbox = held(
            &directory,
            OutboxFile {
                chain_id: "duck-1".into(),
                ..OutboxFile::default()
            },
        );
        outbox
            .stage("claude-a", &request(8, "first", 90_000))
            .expect("staged");
        outbox
            .stage("claude-a", &request(9, "second", 90_000))
            .expect("staged");
        outbox
            .settle(
                "claude-a",
                MessageId {
                    generation: 4,
                    sequence: 8,
                },
            )
            .expect("settled");
        assert!(
            outbox
                .pending_match(
                    "claude-a",
                    4,
                    "standup",
                    "codex-b",
                    MessageKind::Question,
                    "first",
                    0
                )
                .is_none()
        );
        assert!(
            outbox
                .pending_match(
                    "claude-a",
                    4,
                    "standup",
                    "codex-b",
                    MessageKind::Question,
                    "second",
                    0
                )
                .is_some()
        );
        // the high-water mark never walks backwards: a settled 8 does not free 9
        assert_eq!(outbox.high_water("claude-a", 4), 9);

        // and it is on disk, not just in memory
        let reopened: OutboxFile =
            serde_json::from_slice(&std::fs::read(&outbox.path).expect("written"))
                .expect("valid json");
        assert_eq!(reopened.pending.len(), 1);
        assert_eq!(reopened.pending[0].request.body, "second");
    }
}
