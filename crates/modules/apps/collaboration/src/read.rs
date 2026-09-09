//! the ONE read path, and it is authenticated.
//!
//! Every read resolves its caller from the QUERY CONTEXT's origin and from
//! nothing else. There is no open lane: a participant record, a roster and a
//! binding are discovery surfaces the spec scopes to who may discover them,
//! and message bodies, receipts and mailbox accounting are content.
//!
//! Two separate questions, answered separately, because collapsing them is
//! how a scoped credential escalates:
//!
//! * [`authenticate`] — WHO the caller is, and **under what scope**. An owner
//!   key acts for the participant everywhere; a binding's service key acts for
//!   it on THAT ONE CONVERSATION ([`Scope::Binding`]).
//! * [`Reader::may_touch`] + [`seated`] — WHAT this read may see. Every
//!   conversation-scoped read checks the caller's scope against the
//!   conversation it names, and participant-wide projections are owner-only.
//!
//! Today the node's public `/v1/query` reaches a module as `Origin::System`
//! (`Host::query` builds it), so every read over that lane answers
//! [`DenyReason::Unauthenticated`] — the fail-closed default. An authenticated
//! query seam carrying `Origin::External(verified_key)` starts working here
//! with no change to this file.
//!
//! An `Origin::Program(account)` reads on the same terms as a service key: it
//! is a principal a binding can name (dispatch's call lane mints it only for
//! an account identity holds as a program at an unmoved generation), so an
//! agent reads exactly the conversation its participant's owner bound it to.
//! `Origin::Module` reads NOTHING — it names the module in the middle of a
//! follow-up, not a caller.
//!
//! What this is NOT: confidentiality. Committed module state is replicated in
//! plaintext to every validator, which reads its own store directly. This
//! bounds the RPC read lane.

use sdk::{Ctx, Error, Origin, StagedStore};

use crate::interface::{
    BindingView, CollaborationReply, Conversation, ConversationAccess, ConversationEvent,
    Credential, DeliveryEligibility, DeliveryState, DenyReason, EventBody, EventPage, Message,
    Participant, ProtectedRead, Role, SendState, MAX_PAGE_LIMIT,
};
use crate::registry::{authenticates, live_binding, roster_role};
use crate::store;

/// how far the caller's credential reaches.
#[derive(Debug, PartialEq, Eq)]
enum Scope {
    /// the participant's owner key: every conversation it is seated on, plus
    /// the participant-wide projections.
    Owner,
    /// one binding's scoped service key: THAT conversation and nothing else.
    Binding { conversation_id: String },
}

/// the caller, once the context origin has been checked against committed
/// state.
struct Reader {
    participant: Participant,
    /// the credential it authenticates as.
    credential: Credential,
    scope: Scope,
}

impl Reader {
    /// may this caller's credential reach `conversation_id` at all? asked
    /// BEFORE the roster question, because a c1-scoped service key reaching
    /// c2 is an escalation even when the participant is seated on both.
    fn may_touch(&self, conversation_id: &str) -> bool {
        match &self.scope {
            Scope::Owner => true,
            Scope::Binding {
                conversation_id: bound,
            } => bound == conversation_id,
        }
    }

    /// participant-wide state (the record itself, the aggregate mailbox) is
    /// the OWNER's. a conversation-scoped credential must not learn what its
    /// participant is doing elsewhere.
    fn is_owner(&self) -> bool {
        matches!(self.scope, Scope::Owner)
    }
}

/// resolve the caller and its scope, or the token that says why not. the ONLY
/// inputs are the context origin, the conversation the caller names as its
/// authority, and committed state — never a field asserting who the caller is.
///
/// a read FAILS CLOSED, it does not fail: the origin's shape is checked before
/// anything is resolved, so an origin that cannot resolve to an actor (the
/// public lane's `System`, a program account that no longer exists) answers a
/// deny token rather than erroring the query — and costs no sibling read.
async fn authenticate(
    staged: &StagedStore,
    ctx: &dyn Ctx,
    identity: &str,
    origin: &Origin,
    participant_id: &str,
    via: Option<&str>,
) -> Result<Result<Reader, DenyReason>, Error> {
    // WHICH ORIGINS NAME A PRINCIPAL AT ALL. one match, no wildcard: `System`
    // is the unauthenticated public lane and `Module` names the module in the
    // middle of a follow-up, not a caller — neither may read as somebody.
    let names_a_principal = match origin {
        Origin::External(key) => !key.is_empty(),
        // minted only by dispatch's call lane, for an account identity holds
        // as a program at an unmoved generation.
        Origin::Program(_) => true,
        Origin::Module(_) | Origin::System => false,
    };
    if !names_a_principal {
        return Ok(Err(DenyReason::Unauthenticated));
    }
    // an unknown participant and one the caller may not read answer the SAME
    // token: probing must not tell them apart.
    let Some(participant) = store::participant(staged, participant_id).await? else {
        return Ok(Err(DenyReason::NotReader));
    };
    // an origin that cannot resolve to an actor — a program account identity
    // no longer holds, an unreachable identity sibling — is simply NOT the
    // owner. it may still be a bound principal, and either way a read denies
    // rather than erroring the query.
    let owns = match crate::actor_from_origin(ctx, identity).await {
        Ok(actor) => crate::controls(&participant.owner, &actor, origin),
        Err(_) => false,
    };
    if owns {
        // the owner still reads a revoked participant's history — revocation
        // fences the future, it does not rewrite the past.
        let credential = participant.owner_credential;
        return Ok(Ok(Reader {
            participant,
            credential,
            scope: Scope::Owner,
        }));
    }
    // revocation retires EVERY scoped credential at once, so it is checked
    // here — at authentication — not only on the branches that read a roster.
    if participant.revoked {
        return Ok(Err(DenyReason::NotReader));
    }
    let Some(via) = via else {
        return Ok(Err(DenyReason::NotReader));
    };
    let Some(binding) = live_binding(staged, via, &participant.id).await? else {
        return Ok(Err(DenyReason::NotReader));
    };
    if !authenticates(origin, &binding.principal) {
        return Ok(Err(DenyReason::NotReader));
    }
    Ok(Ok(Reader {
        participant,
        credential: binding.credential,
        scope: Scope::Binding {
            conversation_id: via.to_string(),
        },
    }))
}

pub async fn serve(
    staged: &StagedStore,
    ctx: &dyn Ctx,
    identity: &str,
    participant_id: &str,
    via: Option<&str>,
    read: ProtectedRead,
) -> Result<CollaborationReply, Error> {
    let origin = ctx.env().origin.clone();
    let reader = match authenticate(staged, ctx, identity, &origin, participant_id, via).await? {
        Ok(reader) => reader,
        Err(reason) => return Ok(CollaborationReply::Denied(reason)),
    };

    match read {
        // ---- participant-wide: the owner's, never a scoped credential's ----
        ProtectedRead::Participant => {
            if !reader.is_owner() {
                return Ok(CollaborationReply::Denied(DenyReason::NotReader));
            }
            Ok(CollaborationReply::Participant(reader.participant))
        }
        ProtectedRead::Mailbox => {
            // the accounting aggregates every conversation, so a
            // conversation-scoped key would learn what its participant is
            // doing elsewhere. its own capacity answer is the `queue_full`
            // refusal a send returns.
            if !reader.is_owner() {
                return Ok(CollaborationReply::Denied(DenyReason::NotReader));
            }
            Ok(CollaborationReply::Mailbox(
                store::mailbox(staged, &reader.participant.id).await?,
            ))
        }
        // ---- credential-scoped: only ever the caller's own credential ------
        ProtectedRead::SendState {
            generation,
            sequence,
        } => {
            if generation != reader.credential {
                return Ok(CollaborationReply::Denied(DenyReason::NotReader));
            }
            Ok(CollaborationReply::SendState(
                send_state(staged, &reader.participant.id, generation, sequence).await?,
            ))
        }
        // ---- conversation-scoped: scope first, then roster ------------------
        ProtectedRead::Conversation { conversation_id } => {
            let Some(conversation) = reachable(staged, &reader, &conversation_id).await? else {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            };
            Ok(CollaborationReply::Conversation(conversation))
        }
        ProtectedRead::Access { conversation_id } => {
            let closed = ConversationAccess {
                may_read: false,
                may_send: false,
                binding_credential: 0,
            };
            let Some(conversation) = reachable(staged, &reader, &conversation_id).await? else {
                return Ok(CollaborationReply::Access(closed));
            };
            let role = roster_role(&conversation, &reader.participant.id);
            let binding = live_binding(staged, &conversation_id, &reader.participant.id).await?;
            Ok(CollaborationReply::Access(ConversationAccess {
                may_read: role.is_some(),
                may_send: matches!(role, Some(Role::Member)) && !reader.participant.revoked,
                binding_credential: binding.map_or(0, |binding| binding.credential),
            }))
        }
        ProtectedRead::Binding { conversation_id } => {
            if reachable(staged, &reader, &conversation_id).await?.is_none() {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            let binding = store::binding(staged, &conversation_id, &reader.participant.id).await?;
            Ok(CollaborationReply::Binding(
                binding.as_ref().map(BindingView::from),
            ))
        }
        ProtectedRead::Events {
            conversation_id,
            from_seq,
            limit,
        } => {
            let Some(conversation) = reachable(staged, &reader, &conversation_id).await? else {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            };
            Ok(CollaborationReply::Events(
                events(staged, &conversation, from_seq, limit).await?,
            ))
        }
        ProtectedRead::Receipt {
            conversation_id,
            seq,
        } => {
            if reachable(staged, &reader, &conversation_id).await?.is_none() {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            Ok(CollaborationReply::Receipt(
                store::receipt(staged, &conversation_id, seq).await?,
            ))
        }
        ProtectedRead::DeliveryEligibility {
            conversation_id,
            seq,
        } => {
            if reachable(staged, &reader, &conversation_id).await?.is_none() {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            // Revocation preserves owner history access, never authorization
            // for a new provider submission under a still-retained binding.
            if reader.participant.revoked {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            Ok(CollaborationReply::Eligibility(
                eligibility(staged, ctx, &reader, &conversation_id, seq).await?,
            ))
        }
    }
}

/// may a NEW delivery attempt be made for this message?
///
/// The time input is `ctx.env().consensus_time` — the block's AGREED time, the
/// same value admission measured `expires_at` against. It is never a caller's
/// clock, and it never reaches here unauthenticated: the public lane builds
/// `Origin::System` (whose `consensus_time` is 0), and `authenticate` has
/// already refused that before this runs.
async fn eligibility(
    staged: &StagedStore,
    ctx: &dyn Ctx,
    reader: &Reader,
    conversation_id: &str,
    seq: u64,
) -> Result<DeliveryEligibility, Error> {
    let (Some(message), Some(receipt)) = (
        store::message(staged, conversation_id, seq).await?,
        store::receipt(staged, conversation_id, seq).await?,
    ) else {
        return Ok(DeliveryEligibility::Unknown);
    };
    // the question is the RECIPIENT's: "may I still hand my participant's
    // message to a provider". A sender reading its own conversation learns
    // nothing about somebody else's mailbox here.
    if receipt.recipient != reader.participant.id {
        return Ok(DeliveryEligibility::Unknown);
    }
    if receipt.state.is_terminal() {
        return Ok(DeliveryEligibility::Settled {
            state: receipt.state,
        });
    }
    if receipt.state == DeliveryState::DeliveryUnknown {
        return Ok(DeliveryEligibility::NotReplayable);
    }
    if live_binding(staged, conversation_id, &reader.participant.id)
        .await?
        .is_none()
    {
        return Ok(DeliveryEligibility::Unbound);
    }
    // THE DEADLINE, and it is the only thing that decides this. `expire`
    // becomes admissible at exactly `expires_at`, so eligibility ends at
    // exactly `expires_at` — the two agree about one moment.
    let asked_at = ctx.env().consensus_time;
    let expires_at = message.expires_at;
    if asked_at >= expires_at {
        return Ok(DeliveryEligibility::Expired {
            expires_at,
            asked_at,
        });
    }
    Ok(DeliveryEligibility::Eligible {
        state: receipt.state,
        expires_at,
        asked_at,
    })
}

/// the conversation, if the caller's CREDENTIAL reaches it and its participant
/// is seated on it. both halves are load-bearing: the scope check stops a
/// c1-scoped service key reading c2 that its participant also sits on, and the
/// roster check stops anyone reading a conversation they are not on.
async fn reachable(
    staged: &StagedStore,
    reader: &Reader,
    conversation_id: &str,
) -> Result<Option<Conversation>, Error> {
    if !reader.may_touch(conversation_id) {
        return Ok(None);
    }
    seated(staged, &reader.participant.id, conversation_id).await
}

async fn seated(
    staged: &StagedStore,
    participant_id: &str,
    conversation_id: &str,
) -> Result<Option<Conversation>, Error> {
    let Some(conversation) = store::conversation(staged, conversation_id).await? else {
        return Ok(None);
    };
    if roster_role(&conversation, participant_id).is_none() {
        return Ok(None);
    }
    Ok(Some(conversation))
}

async fn send_state(
    staged: &StagedStore,
    participant_id: &str,
    generation: Credential,
    sequence: u64,
) -> Result<SendState, Error> {
    if let Some(admission) = store::admission(staged, participant_id, generation, sequence).await? {
        return Ok(SendState::Admitted {
            seq: admission.seq,
            digest: admission.digest,
        });
    }
    let floor = store::replay_floor(staged, participant_id, generation).await?;
    if sequence < floor {
        return Ok(SendState::ReceiptPruned);
    }
    Ok(SendState::Absent)
}

async fn events(
    staged: &StagedStore,
    conversation: &Conversation,
    from_seq: u64,
    limit: u64,
) -> Result<EventPage, Error> {
    // a cursor below the floor gets an explicit gap and resyncs; it never
    // advances while silently losing actionable events.
    if from_seq < conversation.floor_seq {
        return Ok(EventPage::HistoryGap {
            floor_seq: conversation.floor_seq,
        });
    }
    let limit = limit.clamp(1, MAX_PAGE_LIMIT);
    let mut events: Vec<ConversationEvent> = Vec::new();
    let mut messages: Vec<Message> = Vec::new();
    let mut seq = from_seq;
    while seq < conversation.next_seq && (events.len() as u64) < limit {
        if let Some(event) = store::event(staged, &conversation.id, seq).await? {
            let carries_message = matches!(event.body, EventBody::MessageAdmitted { .. });
            if carries_message
                && let Some(message) = store::message(staged, &conversation.id, seq).await?
            {
                messages.push(message);
            }
            events.push(event);
        }
        seq += 1;
    }
    Ok(EventPage::Page {
        events,
        messages,
        next_seq: seq,
    })
}
