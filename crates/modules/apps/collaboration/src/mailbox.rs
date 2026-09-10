//! admission, delivery receipts, expiry and retention — everything that moves
//! a message through the network.
//!
//! the split the spec insists on holds here: [`send`] admits an IMMUTABLE
//! record, [`acknowledge`] moves the recipient's SEPARATE delivery record, and
//! neither claims a task or asserts that a model read anything.

use sdk::{Ctx, Error, Origin, StagedStore};
use sha2::{Digest, Sha256};

use crate::interface::{
    Credential, DeliveryState, EventBody, Message, MessageKind, Participant, Receipt, Reference,
    Role, SendRequest, TaskRef, BLOB_HEX_LEN, COMMIT_HEX_LEN, DUCK_SCHEME, MAX_BODY_BYTES,
    MAX_MAILBOX_QUEUED_BYTES, MAX_MAILBOX_UNDELIVERED, MAX_REFERENCES, MAX_REFERENCE_BYTES,
    MAX_SEQUENCE, MAX_UNDELIVERED_PER_SENDER,
};
use crate::registry::{
    self, authenticates, live_binding, live_participant, require_conversation, roster_role,
    Advanced,
};
use crate::store::{self, Admission};
use crate::{hex, Party};

/// longest stable reason token an acknowledgement may carry.
pub const MAX_REASON_BYTES: usize = 64;
/// most events ONE [`crate::interface::CollaborationMsg::Prune`] may retire.
/// the walk is a store read per sequence, so the op states its own ceiling
/// rather than letting a caller ask for an unbounded scan.
pub const MAX_PRUNE_SPAN: u64 = 64;

/// the stable token a capacity refusal carries, so a caller can tell "retry
/// later, the queue is full" from "this message is wrong".
pub const QUEUE_FULL: &str = "queue_full";
/// the stable token a below-the-floor retry carries.
pub const RECEIPT_PRUNED: &str = "ReceiptPruned";

/// what [`send`] settled on. an idempotent retry answers with the sequence the
/// original admission holds and stages nothing.
pub struct Admitted {
    pub seq: u64,
    pub digest: String,
    pub replayed: bool,
}

/// the canonical dedup preimage: the send request's own wire bytes. identical
/// bytes under one `message_id` are the same invocation; anything else is a
/// refused reuse of that id.
pub fn request_digest(request: &SendRequest) -> String {
    hex(&Sha256::digest(sdk::wire::encode(request)))
}

fn lowercase_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// each arm is checked STRUCTURALLY, so nothing outside the closed set can be
/// spelled at all: no local scheme, no mutable branch alias, no free-form URL.
fn check_reference(reference: &Reference) -> Result<(), Error> {
    match reference {
        Reference::Commit { repo, commit } => {
            registry::check_id("reference repo", repo)?;
            if !lowercase_hex(commit, COMMIT_HEX_LEN) {
                return Err(Error::Module(format!(
                    "a commit reference is {COMMIT_HEX_LEN} lowercase hex characters; a branch or tag name moves and is not a reference"
                )));
            }
            Ok(())
        }
        Reference::Blob { hash } => {
            if !lowercase_hex(hash, BLOB_HEX_LEN) {
                return Err(Error::Module(format!(
                    "a blob reference is {BLOB_HEX_LEN} lowercase hex characters"
                )));
            }
            Ok(())
        }
        Reference::Duck { url } => {
            let shaped = url.starts_with(DUCK_SCHEME)
                && url.len() > DUCK_SCHEME.len()
                && url.len() <= MAX_REFERENCE_BYTES
                && !url.bytes().any(|b| b.is_ascii_whitespace() || b < 0x20);
            if !shaped {
                return Err(Error::Module(format!(
                    "a link reference is a {DUCK_SCHEME} url of at most {MAX_REFERENCE_BYTES} bytes with no whitespace"
                )));
            }
            Ok(())
        }
    }
}

/// the kind/task/reply_to invariants the spec states, as ONE match over the
/// discriminant rather than a ladder of ifs at the call site.
fn check_kind(kind: MessageKind, task: Option<&TaskRef>, reply_to: Option<u64>) -> Result<(), Error> {
    let satisfied = match kind {
        MessageKind::TaskUpdate => task.is_some(),
        MessageKind::Result => task.is_some() || reply_to.is_some(),
        // a question or a task request can be purely conversational, and a
        // notice never requires either.
        MessageKind::Notice | MessageKind::Question | MessageKind::TaskRequest => true,
    };
    if satisfied {
        return Ok(());
    }
    Err(Error::Module(match kind {
        MessageKind::TaskUpdate => "a task_update must name its task".into(),
        _ => "a result must name its task or the message it answers".into(),
    }))
}

fn check_shape(request: &SendRequest) -> Result<(), Error> {
    registry::check_id("conversation_id", &request.conversation_id)?;
    registry::check_id("sender_participant_id", &request.sender_participant_id)?;
    registry::check_id(
        "recipient_participant_id",
        &request.recipient_participant_id,
    )?;
    if request.body.len() > MAX_BODY_BYTES {
        return Err(Error::Module(format!(
            "body is {} bytes, over the {MAX_BODY_BYTES}-byte cap",
            request.body.len()
        )));
    }
    if request.references.len() > MAX_REFERENCES {
        return Err(Error::Module(format!(
            "{} references, over the {MAX_REFERENCES} cap",
            request.references.len()
        )));
    }
    for reference in &request.references {
        check_reference(reference)?;
    }
    if let Some(task) = &request.task {
        registry::check_id("task id", &task.id)?;
    }
    // the floor must always be able to advance PAST the last admitted
    // sequence, so the top of the space is not admissible. an exhausted
    // credential rotates explicitly rather than wrapping.
    if request.message_id.sequence > MAX_SEQUENCE {
        return Err(Error::Module(format!(
            "credential {} has exhausted its sequence space at {MAX_SEQUENCE}; rotate the binding for a fresh credential",
            request.message_id.generation
        )));
    }
    check_kind(request.kind, request.task.as_ref(), request.reply_to)
}

/// WHICH credential this dispatch's origin authenticates as, for this
/// participant on this conversation: the owner's, the live binding's scoped
/// service key's, or none.
///
/// This is the whole of the sender-identity check. A `message_id.generation`
/// must equal the answer, which is what gives each attached device its own
/// sequence space and what stops a service admitting under the owner's number.
pub async fn authenticating_credential(
    staged: &StagedStore,
    actor: &Party,
    origin: &Origin,
    conversation_id: &str,
    participant: &Participant,
) -> Result<Option<Credential>, Error> {
    if crate::controls(&participant.owner, actor, origin) {
        return Ok(Some(participant.owner_credential));
    }
    let binding = live_binding(staged, conversation_id, &participant.id).await?;
    Ok(binding
        .filter(|binding| authenticates(origin, &binding.principal))
        .map(|binding| binding.credential))
}

/// the attempt fence: a task reference must name the job's CURRENT attempt.
/// this module reads `tasks`; it never creates, claims or schedules work.
async fn check_attempt(
    ctx: &dyn Ctx,
    tasks_id: &str,
    task: &TaskRef,
    what: &str,
) -> Result<(), Error> {
    let request = tasks::encode_job_query(&tasks::JobsQuery::Get {
        job_id: task.id.clone(),
    });
    let bytes = ctx.query(tasks_id, &request).await?;
    let tasks::JobsReply::Job(job) = tasks::decode_job_reply(&bytes).map_err(Error::Module)?;
    let job = job.ok_or_else(|| Error::Module(format!("no task {}", task.id)))?;
    if job.attempt != task.expected_attempt {
        return Err(Error::Module(format!(
            "{what}: task {} is on attempt {}, not the expected {}",
            task.id, job.attempt, task.expected_attempt
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn send(
    staged: &mut StagedStore,
    ctx: &dyn Ctx,
    actor: &Party,
    origin: &Origin,
    now: u64,
    max_delivery_ttl: u64,
    tasks_id: &str,
    request: SendRequest,
) -> Result<Admitted, Error> {
    check_shape(&request)?;

    let sender = live_participant(staged, &request.sender_participant_id).await?;
    let credential =
        authenticating_credential(staged, actor, origin, &request.conversation_id, &sender)
            .await?
            .ok_or_else(|| {
                Error::Module(format!(
                    "not authorized to send as participant {}",
                    sender.id
                ))
            })?;
    // the id must name the credential the caller actually holds. a retired
    // binding's number, another device's number and the owner's number are all
    // refused here — each has its own sequence space and none may be borrowed.
    if request.message_id.generation != credential {
        return Err(Error::Module(format!(
            "message_id names credential {}, but this caller authenticates as {credential}",
            request.message_id.generation
        )));
    }

    let digest = request_digest(&request);
    let floor = store::replay_floor(staged, &sender.id, credential).await?;
    let retained = store::admission(
        staged,
        &sender.id,
        credential,
        request.message_id.sequence,
    )
    .await?;
    if let Some(retained) = retained {
        // the SAME bytes under the same id answer with the existing record; a
        // relay retrying must not mint a new one. different bytes are a
        // refused reuse of that id, never a second admission.
        if retained.digest != digest {
            return Err(Error::Module(format!(
                "message id {{{credential}, {}}} was admitted with different bytes",
                request.message_id.sequence
            )));
        }
        return Ok(Admitted {
            seq: retained.seq,
            digest: retained.digest,
            replayed: true,
        });
    }
    if request.message_id.sequence < floor {
        return Err(Error::Module(format!(
            "{RECEIPT_PRUNED}: sequence {} is below the retained replay floor {floor}",
            request.message_id.sequence
        )));
    }

    let mut conversation = require_conversation(staged, &request.conversation_id).await?;
    // an observer subscribes; it does not become an input owner.
    let sends = matches!(roster_role(&conversation, &sender.id), Some(Role::Member));
    if !sends {
        return Err(Error::Module(format!(
            "participant {} may not send on conversation {}",
            sender.id, conversation.id
        )));
    }
    let recipient = live_participant(staged, &request.recipient_participant_id).await?;
    if roster_role(&conversation, &recipient.id).is_none() {
        return Err(Error::Module(format!(
            "participant {} is not on conversation {}",
            recipient.id, conversation.id
        )));
    }

    // a reply answers a MESSAGE, not merely a sequence number. every committed
    // change takes an event sequence — a roster change, a delivery advance, a
    // prune — so a range check would let a `SetRoster` event be the parent of a
    // reply, and a reader threading by `reply_to` would find no message there.
    // the existence check subsumes the retention floor: a pruned message's
    // record is gone, so its sequence stops being answerable with its body.
    if let Some(reply_to) = request.reply_to {
        let parent = store::message(staged, &conversation.id, reply_to).await?;
        if parent.is_none() {
            return Err(Error::Module(format!(
                "reply_to {reply_to} is not a retained message of conversation {}",
                conversation.id
            )));
        }
    }
    if let Some(task) = &request.task {
        check_attempt(ctx, tasks_id, task, "stale task target").await?;
    }

    if request.expires_at <= now {
        return Err(Error::Module(format!(
            "expires_at {} is not after the block's agreed time {now}",
            request.expires_at
        )));
    }
    if request.expires_at - now > max_delivery_ttl {
        return Err(Error::Module(format!(
            "expires_at {} is more than {max_delivery_ttl} time units out",
            request.expires_at
        )));
    }

    // the message occupies the event sequence its admission takes, so a
    // receipt, a `reply_to` and a cursor all name the same number.
    let seq = conversation.next_seq;
    let message = Message {
        seq,
        message_id: request.message_id,
        conversation_id: conversation.id.clone(),
        sender: sender.id.clone(),
        recipient: recipient.id.clone(),
        kind: request.kind,
        reply_to: request.reply_to,
        task: request.task.clone(),
        body: request.body.clone(),
        references: request.references.clone(),
        expires_at: request.expires_at,
        admitted_at: now,
        digest: digest.clone(),
    };
    let encoded = store::check_message(&message)?;
    let queued_bytes = encoded.len() as u64;

    // the caps apply at admission INCLUDING while the recipient is
    // disconnected, and replacing a binding does not reset the accounting.
    let mut usage = store::mailbox(staged, &recipient.id).await?;
    if usage.undelivered >= MAX_MAILBOX_UNDELIVERED {
        return Err(Error::Module(format!(
            "{QUEUE_FULL}: {} holds the {MAX_MAILBOX_UNDELIVERED} undelivered message cap",
            recipient.id
        )));
    }
    if usage.queued_bytes + queued_bytes > MAX_MAILBOX_QUEUED_BYTES {
        return Err(Error::Module(format!(
            "{QUEUE_FULL}: {} would exceed the {MAX_MAILBOX_QUEUED_BYTES}-byte queued payload cap",
            recipient.id
        )));
    }
    let quota = store::sender_quota(staged, &recipient.id, &sender.id).await?;
    if quota >= MAX_UNDELIVERED_PER_SENDER {
        return Err(Error::Module(format!(
            "{QUEUE_FULL}: {} already holds {MAX_UNDELIVERED_PER_SENDER} undelivered messages from {}",
            recipient.id, sender.id
        )));
    }

    let receipt = Receipt {
        conversation_id: conversation.id.clone(),
        seq,
        recipient: recipient.id.clone(),
        state: DeliveryState::Stored,
        advanced_by: 0,
        reason: None,
        updated_at: now,
    };
    let admission = Admission {
        conversation_id: conversation.id.clone(),
        seq,
        digest,
    };
    usage.undelivered += 1;
    usage.queued_bytes += queued_bytes;

    // CHECK everything, THEN stage everything: a path that stages a write and
    // then returns Err leaves overlay residue on the native side and none in
    // the wasm port, and the two roots diverge.
    let receipt_bytes = sdk::wire::encode(&receipt);
    store::check_record(&receipt_bytes, "receipt")?;

    let event_seq = store::append_event(
        staged,
        &mut conversation,
        now,
        EventBody::MessageAdmitted {
            sender: sender.id.clone(),
            recipient: recipient.id.clone(),
        },
    )?;
    debug_assert_eq!(event_seq, seq, "a message occupies its own event sequence");
    staged.stage(store::message_key(&conversation.id, seq), encoded);
    staged.stage(store::receipt_key(&conversation.id, seq), receipt_bytes);
    store::put(
        staged,
        store::conversation_key(&conversation.id),
        &conversation,
        "conversation",
    )?;
    store::put(
        staged,
        store::admission_key(&sender.id, credential, request.message_id.sequence),
        &admission,
        "admission",
    )?;
    store::put(staged, store::mailbox_key(&recipient.id), &usage, "mailbox")?;
    store::put_counter(
        staged,
        store::sender_quota_key(&recipient.id, &sender.id),
        quota + 1,
    );
    Ok(Admitted {
        seq,
        digest: message.digest,
        replayed: false,
    })
}

fn check_reason(reason: Option<&String>) -> Result<(), Error> {
    let Some(reason) = reason else {
        return Ok(());
    };
    // a stable snake_case token: greppable and countable, never prose and
    // never somewhere a path or a credential could hide.
    let shaped = !reason.is_empty()
        && reason.len() <= MAX_REASON_BYTES
        && reason
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if shaped {
        return Ok(());
    }
    Err(Error::Module(format!(
        "reason must be a snake_case token of at most {MAX_REASON_BYTES} bytes"
    )))
}

/// release the recipient's queue accounting once a record reaches a terminal
/// state. saturating on purpose: an accounting record that somehow lags must
/// not wedge the block.
async fn release(staged: &mut StagedStore, message: &Message, encoded_len: u64) -> Result<(), Error> {
    let mut usage = store::mailbox(staged, &message.recipient).await?;
    usage.undelivered = usage.undelivered.saturating_sub(1);
    usage.queued_bytes = usage.queued_bytes.saturating_sub(encoded_len);
    let quota = store::sender_quota(staged, &message.recipient, &message.sender).await?;
    store::put(
        staged,
        store::mailbox_key(&message.recipient),
        &usage,
        "mailbox",
    )?;
    store::put_counter(
        staged,
        store::sender_quota_key(&message.recipient, &message.sender),
        quota.saturating_sub(1),
    );
    Ok(())
}

async fn load_pair(
    staged: &StagedStore,
    conversation_id: &str,
    seq: u64,
) -> Result<(Message, Receipt), Error> {
    let message = store::message(staged, conversation_id, seq)
        .await?
        .ok_or_else(|| Error::Module(format!("no message {seq} on {conversation_id}")))?;
    let receipt = store::receipt(staged, conversation_id, seq)
        .await?
        .ok_or_else(|| Error::Module(format!("no receipt for {seq} on {conversation_id}")))?;
    Ok((message, receipt))
}

/// move a receipt and record the transition as its own committed event, so a
/// consumer resuming from a cursor sees `Held`/`AdapterAccepted` too.
async fn advance(
    staged: &mut StagedStore,
    message: &Message,
    mut receipt: Receipt,
    now: u64,
    state: DeliveryState,
    reason: Option<String>,
    advanced_by: Credential,
) -> Result<Advanced, Error> {
    let was_open = !receipt.state.is_terminal();
    receipt.state = state;
    receipt.advanced_by = advanced_by;
    receipt.reason = reason.clone();
    receipt.updated_at = now;
    let receipt_bytes = sdk::wire::encode(&receipt);
    store::check_record(&receipt_bytes, "receipt")?;

    let mut conversation = require_conversation(staged, &message.conversation_id).await?;
    let seq = store::append_event(
        staged,
        &mut conversation,
        now,
        EventBody::DeliveryAdvanced {
            message_seq: message.seq,
            state,
            reason,
        },
    )?;
    staged.stage(
        store::receipt_key(&message.conversation_id, message.seq),
        receipt_bytes,
    );
    store::put(
        staged,
        store::conversation_key(&message.conversation_id),
        &conversation,
        "conversation",
    )?;
    if was_open && state.is_terminal() {
        let encoded_len = store::check_message(message)?.len() as u64;
        release(staged, message, encoded_len).await?;
    }
    Ok(Advanced { seq })
}

#[allow(clippy::too_many_arguments)]
pub async fn acknowledge(
    staged: &mut StagedStore,
    ctx: &dyn Ctx,
    now: u64,
    tasks_id: &str,
    conversation_id: String,
    seq: u64,
    binding_credential: Credential,
    state: DeliveryState,
    reason: Option<String>,
) -> Result<Advanced, Error> {
    check_reason(reason.as_ref())?;
    let (message, receipt) = load_pair(staged, &conversation_id, seq).await?;

    // permission is rechecked at DELIVERY time, not only at admission: a
    // participant removed from the roster since must not keep writing
    // receipts. `SetRoster` also detaches the binding, so this is the second
    // of two fences rather than the only one — kept local so the invariant
    // does not depend on reading another file to see it.
    let conversation = require_conversation(staged, &conversation_id).await?;
    if roster_role(&conversation, &receipt.recipient).is_none() {
        return Err(Error::Module(format!(
            "participant {} is no longer on conversation {conversation_id}",
            receipt.recipient
        )));
    }

    let binding = store::binding(staged, &conversation_id, &receipt.recipient)
        .await?
        .ok_or_else(|| {
            Error::Module(format!(
                "participant {} has no binding on {conversation_id}",
                receipt.recipient
            ))
        })?;
    // ONLY the currently authorized binding advances the record. a stale
    // service may report history for inspection; it cannot overwrite this.
    // the credential names the binding the receipt is for, not the caller:
    // any authenticated member reports under the live one.
    if binding.detached || binding.credential != binding_credential {
        return Err(Error::Module(format!(
            "binding credential {binding_credential} is stale; the current one is {}{}",
            binding.credential,
            if binding.detached { " (detached)" } else { "" }
        )));
    }
    // EXPIRY IS THE DEADLINE'S, NOT A REPORTER'S. `expire` is permissionless
    // precisely because it checks the clock — every caller asking gets the same
    // answer. Reaching the same terminal state through an acknowledgement would
    // route around that check: a bound service (or the participant's owner)
    // could declare a still-live message expired, settle it, and free its queue
    // slot before its deadline.
    //
    // A LATE report is a different thing and stays admissible: the receipt
    // records what a bound service observed, and a provider that really did
    // accept the input said so (see
    // `a_late_authentic_acceptance_beats_the_sweeper_and_is_not_relabelled`).
    // The deadline bounds the resource, not the truth — anyone may sweep an
    // unsettled record once it passes.
    if state == DeliveryState::Expired {
        return Err(Error::Module(format!(
            "expiry is not reported: {seq} on {conversation_id} expires at {} by the deadline alone",
            message.expires_at
        )));
    }
    if !receipt.state.may_advance_to(state) {
        return Err(Error::Module(format!(
            "delivery cannot move from {} to {}",
            receipt.state.as_str(),
            state.as_str()
        )));
    }
    // a previous attempt cannot publish as the current one after returning:
    // the task's attempt is rechecked HERE, not only at admission.
    if let Some(task) = &message.task {
        check_attempt(ctx, tasks_id, task, "stale attempt acknowledgement").await?;
    }
    advance(
        staged,
        &message,
        receipt,
        now,
        state,
        reason,
        binding_credential,
    )
    .await
}

/// permissionless: the deadline is agreed network time, so every caller asking
/// gets the same answer and none of them needs authority to ask.
pub async fn expire(
    staged: &mut StagedStore,
    now: u64,
    conversation_id: String,
    seq: u64,
) -> Result<Advanced, Error> {
    let (message, receipt) = load_pair(staged, &conversation_id, seq).await?;
    if now < message.expires_at {
        return Err(Error::Module(format!(
            "message {seq} on {conversation_id} expires at {}, not yet at {now}",
            message.expires_at
        )));
    }
    if receipt.state.is_terminal() {
        return Err(Error::Module(format!(
            "delivery of {seq} on {conversation_id} already settled as {}",
            receipt.state.as_str()
        )));
    }
    let advanced_by = receipt.advanced_by;
    advance(
        staged,
        &message,
        receipt,
        now,
        DeliveryState::Expired,
        Some("deadline_passed".into()),
        advanced_by,
    )
    .await
}

/// retire events and message bodies below `through_seq`, advancing the
/// senders' replay floors with them, so expired request bytes can never be
/// admitted as a new message after their body is pruned.
pub async fn prune(
    staged: &mut StagedStore,
    now: u64,
    conversation_id: String,
    through_seq: u64,
) -> Result<u64, Error> {
    // any authenticated member prunes any conversation; the floor only ever
    // rises, so a prune is idempotent whoever asks for it.
    let mut conversation = require_conversation(staged, &conversation_id).await?;
    if through_seq <= conversation.floor_seq {
        return Err(Error::Module(format!(
            "through_seq {through_seq} is at or below the floor {}",
            conversation.floor_seq
        )));
    }
    if through_seq > conversation.next_seq {
        return Err(Error::Module(format!(
            "through_seq {through_seq} is past the head {}",
            conversation.next_seq
        )));
    }
    if through_seq - conversation.floor_seq > MAX_PRUNE_SPAN {
        return Err(Error::Module(format!(
            "a prune retires at most {MAX_PRUNE_SPAN} events per operation"
        )));
    }

    // gather first: an undelivered record anywhere in the span refuses the
    // WHOLE op, so retention never silently drops actionable mail.
    let from_seq = conversation.floor_seq;
    let mut retiring = Vec::new();
    for seq in from_seq..through_seq {
        let Some(message) = store::message(staged, &conversation_id, seq).await? else {
            continue;
        };
        let receipt = store::receipt(staged, &conversation_id, seq)
            .await?
            .ok_or_else(|| Error::Module(format!("no receipt for {seq} on {conversation_id}")))?;
        if !receipt.state.is_terminal() {
            return Err(Error::Module(format!(
                "message {seq} on {conversation_id} is still {} — it stays until delivery, refusal or expiry",
                receipt.state.as_str()
            )));
        }
        retiring.push(message);
    }

    conversation.floor_seq = through_seq;
    conversation.updated_at = now;
    let conversation_bytes = sdk::wire::encode(&conversation);
    store::check_record(&conversation_bytes, "conversation")?;

    // the floor must clear the retired sequence. `check_shape` refuses
    // `MAX_SEQUENCE + 1` at admission so this cannot be reached, but a stored
    // record is not a place to rely on an invariant held elsewhere: refuse
    // before staging any delete rather than wrap the floor backwards.
    for message in &retiring {
        if message.message_id.sequence.checked_add(1).is_none() {
            return Err(Error::Module(format!(
                "message {} carries an unadvanceable replay sequence",
                message.seq
            )));
        }
    }

    let retired = retiring.len() as u64;
    for seq in from_seq..through_seq {
        staged.delete(store::event_key(&conversation_id, seq));
    }
    for message in retiring {
        staged.delete(store::message_key(&conversation_id, message.seq));
        staged.delete(store::receipt_key(&conversation_id, message.seq));
        // the dedup record goes with the body, and the floor rises past it: a
        // retry of that sequence now answers ReceiptPruned rather than
        // becoming a second admission of bytes nobody can compare any more.
        staged.delete(store::admission_key(
            &message.sender,
            message.message_id.generation,
            message.message_id.sequence,
        ));
        let floor = store::replay_floor(staged, &message.sender, message.message_id.generation)
            .await?
            .max(message.message_id.sequence + 1);
        store::put_counter(
            staged,
            store::replay_floor_key(&message.sender, message.message_id.generation),
            floor,
        );
    }
    staged.stage(store::conversation_key(&conversation_id), conversation_bytes);
    Ok(retired)
}
