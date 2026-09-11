//! the ONE read path, and it is authenticated.
//!
//! Every read resolves its caller from the QUERY CONTEXT's origin and from
//! nothing else. There is no open lane: a binding and a delivery record are
//! discovery surfaces the spec scopes to who may discover them, and mailbox
//! accounting is content.
//!
//! Two separate questions, answered separately, because collapsing them is
//! how a scoped credential escalates:
//!
//! * [`authenticate`] — WHO the caller is, and **under what scope**. The
//!   participant's own origin acts for it everywhere; a binding's service key
//!   acts for it on THAT ONE CHANNEL ([`Scope::Binding`]).
//! * [`Reader::may_touch`] + chat's `Access` — WHAT this read may see. Every
//!   channel-scoped read checks the caller's scope against the channel it
//!   names and then asks chat whether the participant may read it.
//!
//! Today the node's public `/v1/query` reaches a module as `Origin::System`
//! (`Host::query` builds it), so every read over that lane answers
//! [`DenyReason::Unauthenticated`] — the fail-closed default.
//!
//! An `Origin::Program(account)` IS its account, so an agent reads as the
//! participant its account is. `Origin::Module` reads NOTHING — it names the
//! module in the middle of a follow-up, not a caller.
//!
//! What this is NOT: confidentiality. Committed module state is replicated in
//! plaintext to every validator, which reads its own store directly. This
//! bounds the RPC read lane.

use sdk::{Ctx, Error, Origin, StagedStore};

use crate::bindings::{authenticates, live_binding};
use crate::interface::{
    BindingView, ChannelEvent, CollaborationReply, Delivery, DeliveryEligibility, DeliveryState,
    DenyReason, EventBody, EventPage, MAX_PAGE_LIMIT, Party, ProtectedRead,
};
use crate::store;

/// how far the caller's credential reaches.
#[derive(Debug, PartialEq, Eq)]
enum Scope {
    /// the participant itself: every channel it may read, plus the
    /// participant-wide projections.
    Owner,
    /// one binding's scoped service key: THAT channel and nothing else.
    Binding { channel_id: String },
}

/// the caller, once the context origin has been checked against committed
/// state.
struct Reader {
    participant: Party,
    scope: Scope,
}

impl Reader {
    /// may this caller's credential reach `channel_id` at all? asked BEFORE
    /// the roster question, because a c1-scoped service key reaching c2 is an
    /// escalation even when the participant may read both.
    fn may_touch(&self, channel_id: &str) -> bool {
        match &self.scope {
            Scope::Owner => true,
            Scope::Binding { channel_id: bound } => bound == channel_id,
        }
    }

    /// participant-wide state (the aggregate mailbox) is the OWNER's. a
    /// channel-scoped credential must not learn what its participant is doing
    /// elsewhere.
    fn is_owner(&self) -> bool {
        matches!(self.scope, Scope::Owner)
    }
}

/// resolve the caller and its scope, or the token that says why not. the ONLY
/// inputs are the context origin, the channel the caller names as its
/// authority, and committed state — never a field asserting who the caller is.
///
/// a read FAILS CLOSED, it does not fail: the origin's shape is checked before
/// anything is resolved, so an origin that cannot resolve to an actor (the
/// public lane's `System`, a program account that no longer exists) answers a
/// deny token rather than erroring the query.
async fn authenticate(
    staged: &StagedStore,
    ctx: &dyn Ctx,
    identity: &str,
    origin: &Origin,
    participant: &Party,
    via: Option<&str>,
) -> Result<Result<Reader, DenyReason>, Error> {
    // WHICH ORIGINS NAME A PRINCIPAL AT ALL. one match, no wildcard: `System`
    // is the unauthenticated public lane and `Module` names the module in the
    // middle of a follow-up, not a caller — neither may read as somebody.
    let names_a_principal = match origin {
        Origin::External(key) => !key.is_empty(),
        Origin::Program(_) => true,
        Origin::Module(_) | Origin::System => false,
    };
    if !names_a_principal {
        return Ok(Err(DenyReason::Unauthenticated));
    }
    // an origin that cannot resolve to an actor — a program account identity
    // no longer holds — is simply NOT the participant. it may still be a bound
    // principal, and either way a read denies rather than erroring the query.
    let is_participant = match crate::actor_from_origin(ctx, identity).await {
        Ok(actor) => &actor == participant,
        Err(_) => false,
    };
    if is_participant {
        return Ok(Ok(Reader {
            participant: participant.clone(),
            scope: Scope::Owner,
        }));
    }
    let Some(via) = via else {
        return Ok(Err(DenyReason::NotReader));
    };
    let Some(binding) = live_binding(staged, via, participant).await? else {
        return Ok(Err(DenyReason::NotReader));
    };
    if !authenticates(origin, &binding.principal) {
        return Ok(Err(DenyReason::NotReader));
    }
    Ok(Ok(Reader {
        participant: participant.clone(),
        scope: Scope::Binding {
            channel_id: via.to_string(),
        },
    }))
}

pub async fn serve(
    staged: &StagedStore,
    ctx: &dyn Ctx,
    identity: &str,
    chat: &str,
    participant: &Party,
    via: Option<&str>,
    read: ProtectedRead,
) -> Result<CollaborationReply, Error> {
    let origin = ctx.env().origin.clone();
    let reader = match authenticate(staged, ctx, identity, &origin, participant, via).await? {
        Ok(reader) => reader,
        Err(reason) => return Ok(CollaborationReply::Denied(reason)),
    };

    match read {
        // ---- participant-wide: the owner's, never a scoped credential's ----
        ProtectedRead::Mailbox => {
            if !reader.is_owner() {
                return Ok(CollaborationReply::Denied(DenyReason::NotReader));
            }
            Ok(CollaborationReply::Mailbox(
                store::mailbox(staged, &reader.participant).await?,
            ))
        }
        // ---- channel-scoped: scope first, then chat's roster ---------------
        ProtectedRead::Binding { channel_id } => {
            if !reachable(ctx, chat, &reader, &channel_id).await? {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            let binding = store::binding(staged, &channel_id, &reader.participant).await?;
            Ok(CollaborationReply::Binding(
                binding.as_ref().map(BindingView::from),
            ))
        }
        ProtectedRead::Events {
            channel_id,
            from_seq,
            limit,
        } => {
            if !reachable(ctx, chat, &reader, &channel_id).await? {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            Ok(CollaborationReply::Events(
                events(staged, &channel_id, from_seq, limit).await?,
            ))
        }
        ProtectedRead::Delivery { channel_id, seq } => {
            if !reachable(ctx, chat, &reader, &channel_id).await? {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            Ok(CollaborationReply::Delivery(
                store::delivery(staged, &channel_id, seq, &reader.participant).await?,
            ))
        }
        ProtectedRead::DeliveryEligibility { channel_id, seq } => {
            if !reachable(ctx, chat, &reader, &channel_id).await? {
                return Ok(CollaborationReply::Denied(DenyReason::NotPermitted));
            }
            Ok(CollaborationReply::Eligibility(
                eligibility(staged, ctx, &reader, &channel_id, seq).await?,
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
    channel_id: &str,
    seq: u64,
) -> Result<DeliveryEligibility, Error> {
    // the question is the RECIPIENT's: "may I still hand my participant's
    // message to a provider". the record is keyed by recipient, so a sender
    // asking learns nothing about somebody else's mailbox here.
    let Some(delivery) = store::delivery(staged, channel_id, seq, &reader.participant).await?
    else {
        return Ok(DeliveryEligibility::Unknown);
    };
    if delivery.state.is_terminal() {
        return Ok(DeliveryEligibility::Settled {
            state: delivery.state,
        });
    }
    if delivery.state == DeliveryState::DeliveryUnknown {
        return Ok(DeliveryEligibility::NotReplayable);
    }
    if !crate::delivery::carrier(staged, channel_id, &reader.participant).await? {
        return Ok(DeliveryEligibility::Unbound);
    }
    // THE DEADLINE, and it is the only thing that decides this. `expire`
    // becomes admissible at exactly `expires_at`, so eligibility ends at
    // exactly `expires_at` — the two agree about one moment.
    let asked_at = ctx.env().consensus_time;
    let expires_at = delivery.expires_at;
    if asked_at >= expires_at {
        return Ok(DeliveryEligibility::Expired {
            expires_at,
            asked_at,
        });
    }
    Ok(DeliveryEligibility::Eligible {
        state: delivery.state,
        expires_at,
        asked_at,
    })
}

/// may the caller's CREDENTIAL reach this channel, and may its participant
/// read it? both halves are load-bearing: the scope check stops a c1-scoped
/// service key reading c2 that its participant may also read, and chat's gate
/// stops anyone reading a channel they are not on.
async fn reachable(
    ctx: &dyn Ctx,
    chat: &str,
    reader: &Reader,
    channel_id: &str,
) -> Result<bool, Error> {
    if !reader.may_touch(channel_id) {
        return Ok(false);
    }
    let access = crate::chat_access(ctx, chat, channel_id, &reader.participant).await?;
    Ok(access.may_read)
}

async fn events(
    staged: &StagedStore,
    channel_id: &str,
    from_seq: u64,
    limit: u64,
) -> Result<EventPage, Error> {
    let head = store::head(staged, channel_id).await?;
    let limit = limit.clamp(1, MAX_PAGE_LIMIT);
    let mut events: Vec<ChannelEvent> = Vec::new();
    let mut deliveries: Vec<Delivery> = Vec::new();
    // sequences are 1-based; a cursor of 0 is "from the beginning".
    let mut seq = from_seq.max(1);
    while seq < head && (events.len() as u64) < limit {
        if let Some(event) = store::event(staged, channel_id, seq).await? {
            if let EventBody::DeliveryRequested {
                message_seq,
                recipient,
                ..
            } = &event.body
                && let Some(delivery) =
                    store::delivery(staged, channel_id, *message_seq, recipient).await?
            {
                deliveries.push(delivery);
            }
            events.push(event);
        }
        seq += 1;
    }
    Ok(EventPage {
        events,
        deliveries,
        next_seq: seq,
    })
}
