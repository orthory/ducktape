//! participants, conversations, rosters and bindings — the identity and
//! addressing plane. Nothing here touches message content; everything here is
//! gated on the AUTHENTICATED origin, never on a name carried in the payload.
//!
//! Credentials are allocated here and nowhere else. A participant's owner key
//! admits under `owner_credential`; every [`Bind`](crate::interface::CollaborationMsg::Bind)
//! draws a FRESH number from the participant's monotonic allocator. That is
//! what keeps one participant attached on two devices from writing the same
//! dedup key: the two services hold different credentials, so their sequence
//! spaces are disjoint even though both start at 1.

use sdk::{Error, Origin, StagedStore};

use crate::interface::{
    Binding, Conversation, Credential, EventBody, Participant, Role, MAX_ID_BYTES,
    MAX_LABEL_BYTES, MAX_ROSTER,
};
use crate::store;
use crate::{controls, Party};

/// ids are compared byte-for-byte everywhere, so an empty or oversized one is
/// refused at the boundary rather than stored.
pub fn check_id(what: &str, id: &str) -> Result<(), Error> {
    let shaped = !id.is_empty() && id.len() <= MAX_ID_BYTES;
    if !shaped {
        return Err(Error::Module(format!(
            "{what} must be 1..={MAX_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

pub fn check_label(what: &str, label: &str) -> Result<(), Error> {
    if label.len() > MAX_LABEL_BYTES {
        return Err(Error::Module(format!(
            "{what} is {} bytes, over the {MAX_LABEL_BYTES}-byte cap",
            label.len()
        )));
    }
    Ok(())
}

/// the participant a mutation names, refusing an unknown or revoked one.
pub async fn live_participant(
    staged: &StagedStore,
    participant_id: &str,
) -> Result<Participant, Error> {
    let participant = store::participant(staged, participant_id)
        .await?
        .ok_or_else(|| Error::Module(format!("no participant {participant_id}")))?;
    if participant.revoked {
        return Err(Error::Module(format!(
            "participant {participant_id} is revoked"
        )));
    }
    Ok(participant)
}

pub async fn require_conversation(
    staged: &StagedStore,
    conversation_id: &str,
) -> Result<Conversation, Error> {
    store::conversation(staged, conversation_id)
        .await?
        .ok_or_else(|| Error::Module(format!("no conversation {conversation_id}")))
}

pub fn roster_role(conversation: &Conversation, participant_id: &str) -> Option<Role> {
    conversation.roster.get(participant_id).copied()
}

/// did THIS dispatch's authenticated key sign as `key`? only an external
/// origin carries a key: a module, program or system origin holds none, so
/// none of them can ever pass as a scoped service credential.
pub fn signed_by(origin: &Origin, key: &[u8]) -> bool {
    matches!(origin, Origin::External(signer) if !signer.is_empty() && signer == key)
}

/// the participant's live binding on one conversation, if it holds one that
/// has not been detached.
pub async fn live_binding(
    staged: &StagedStore,
    conversation_id: &str,
    participant_id: &str,
) -> Result<Option<Binding>, Error> {
    Ok(store::binding(staged, conversation_id, participant_id)
        .await?
        .filter(|binding| !binding.detached))
}

pub async fn register_participant(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    participant_id: String,
    display_name: String,
    agent_account: Option<sdk::AccountNumber>,
) -> Result<(), Error> {
    check_id("participant_id", &participant_id)?;
    check_label("display_name", &display_name)?;
    // a participant is an OWNER-AUTHORIZED identity: the system origin owns
    // nothing and authorizes nobody, so genesis cannot mint one.
    if matches!(origin, Origin::System) {
        return Err(Error::Module(
            "a participant needs an authenticated owner; system origin cannot register one".into(),
        ));
    }
    if store::participant(staged, &participant_id).await?.is_some() {
        return Err(Error::Module(format!(
            "participant {participant_id} already exists"
        )));
    }
    let participant = Participant {
        id: participant_id.clone(),
        owner: actor.clone(),
        display_name,
        agent_account,
        // credential 1 is the owner's; 0 stays reserved for "holds none", so
        // a caller naming generation 0 can never authenticate.
        owner_credential: 1,
        next_credential: 2,
        revoked: false,
        created_at: now,
        updated_at: now,
    };
    store::put(
        staged,
        store::participant_key(&participant_id),
        &participant,
        "participant",
    )
}

pub async fn revoke_participant(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    participant_id: String,
) -> Result<(), Error> {
    let mut participant = live_participant(staged, &participant_id).await?;
    if !controls(&participant.owner, actor, origin) {
        return Err(Error::Module(format!(
            "only the owner may revoke participant {participant_id}"
        )));
    }
    // one flag retires EVERY credential at once — the owner's and each
    // binding's. the allocator is left where it is, so no retired number can
    // be handed out again if the id is ever seated afresh.
    participant.revoked = true;
    participant.updated_at = now;
    store::put(
        staged,
        store::participant_key(&participant_id),
        &participant,
        "participant",
    )
}

pub async fn create_conversation(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    conversation_id: String,
    topic: String,
) -> Result<(), Error> {
    check_id("conversation_id", &conversation_id)?;
    check_label("topic", &topic)?;
    if matches!(origin, Origin::System) {
        return Err(Error::Module(
            "a conversation needs an authenticated owner".into(),
        ));
    }
    if store::conversation(staged, &conversation_id).await?.is_some() {
        return Err(Error::Module(format!(
            "conversation {conversation_id} already exists"
        )));
    }
    let conversation = Conversation {
        id: conversation_id.clone(),
        topic,
        owner: actor.clone(),
        roster: Default::default(),
        // sequences are 1-based, so `next_seq == floor_seq == 1` is an empty
        // stream with nothing pruned.
        next_seq: 1,
        floor_seq: 1,
        created_at: now,
        updated_at: now,
    };
    store::put(
        staged,
        store::conversation_key(&conversation_id),
        &conversation,
        "conversation",
    )
}

/// the outcome an op reports back so the module can stamp it and notify.
pub struct Advanced {
    pub seq: u64,
}

pub async fn set_roster(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    conversation_id: String,
    participant_id: String,
    role: Option<Role>,
) -> Result<Advanced, Error> {
    let mut conversation = require_conversation(staged, &conversation_id).await?;
    if !controls(&conversation.owner, actor, origin) {
        return Err(Error::Module(format!(
            "only the owner may edit conversation {conversation_id}"
        )));
    }
    let revoking = role.is_none();
    if revoking {
        if !conversation.roster.contains_key(&participant_id) {
            return Err(Error::Module(format!(
                "participant {participant_id} is not on conversation {conversation_id}"
            )));
        }
        conversation.roster.remove(&participant_id);
    } else {
        // an unknown or revoked participant cannot be seated: the roster holds
        // resolved, live ids.
        live_participant(staged, &participant_id).await?;
        let seating_a_newcomer = !conversation.roster.contains_key(&participant_id);
        if seating_a_newcomer && conversation.roster.len() >= MAX_ROSTER {
            return Err(Error::Module(format!(
                "conversation {conversation_id} is at the {MAX_ROSTER}-participant roster cap"
            )));
        }
        conversation
            .roster
            .insert(participant_id.clone(), role.expect("checked above"));
    }

    let seq = store::append_event(
        staged,
        &mut conversation,
        now,
        EventBody::RosterChanged {
            participant_id: participant_id.clone(),
            role,
        },
    )?;
    // roster revocation FENCES the binding with it: the service holding that
    // conversation's credential can no longer receive or acknowledge, and its
    // credential number is spent.
    if revoking {
        detach_binding(staged, &mut conversation, now, &participant_id).await?;
    }
    store::put(
        staged,
        store::conversation_key(&conversation_id),
        &conversation,
        "conversation",
    )?;
    Ok(Advanced { seq })
}

/// mark an existing binding detached without moving its credential — the
/// number stays spent so a returning stale service cannot reuse it.
async fn detach_binding(
    staged: &mut StagedStore,
    conversation: &mut Conversation,
    now: u64,
    participant_id: &str,
) -> Result<(), Error> {
    let Some(mut binding) = live_binding(staged, &conversation.id, participant_id).await? else {
        return Ok(());
    };
    binding.detached = true;
    store::append_event(
        staged,
        conversation,
        now,
        EventBody::BindingChanged {
            participant_id: participant_id.to_string(),
            credential: binding.credential,
            detached: true,
        },
    )?;
    store::put(
        staged,
        store::binding_key(&conversation.id, participant_id),
        &binding,
        "binding",
    )
}

/// the credential a replacement must name: the one the current binding holds,
/// or 0 before any attachment.
async fn current_credential(
    staged: &StagedStore,
    conversation_id: &str,
    participant_id: &str,
) -> Result<Credential, Error> {
    Ok(store::binding(staged, conversation_id, participant_id)
        .await?
        .map_or(0, |binding| binding.credential))
}

#[allow(clippy::too_many_arguments)]
pub async fn bind(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    conversation_id: String,
    participant_id: String,
    device: String,
    service_key: Vec<u8>,
    expected_credential: Credential,
) -> Result<Advanced, Error> {
    check_label("device", &device)?;
    if service_key.is_empty() {
        return Err(Error::Module(
            "a binding must authorize a non-empty scoped service key".into(),
        ));
    }
    let mut participant = live_participant(staged, &participant_id).await?;
    // the OWNER issues the scoped credential. a service cannot promote itself,
    // and cannot mint a second credential for its own key.
    if !controls(&participant.owner, actor, origin) {
        return Err(Error::Module(format!(
            "only the owner may bind participant {participant_id}"
        )));
    }
    let mut conversation = require_conversation(staged, &conversation_id).await?;
    if roster_role(&conversation, &participant_id).is_none() {
        return Err(Error::Module(format!(
            "participant {participant_id} is not on conversation {conversation_id}"
        )));
    }
    let current = current_credential(staged, &conversation_id, &participant_id).await?;
    if expected_credential != current {
        return Err(Error::Module(format!(
            "binding credential is {current}, not the expected {expected_credential}"
        )));
    }
    // a FRESH number off the participant's allocator — never `current + 1`,
    // which would collide with the credential another conversation's binding
    // already holds.
    let credential = participant.next_credential;
    participant.next_credential = credential
        .checked_add(1)
        .ok_or_else(|| Error::Module("participant credentials exhausted".into()))?;
    participant.updated_at = now;

    let binding = Binding {
        conversation_id: conversation_id.clone(),
        participant_id: participant_id.clone(),
        credential,
        service_key,
        device,
        attached_at: now,
        detached: false,
    };
    let seq = store::append_event(
        staged,
        &mut conversation,
        now,
        EventBody::BindingChanged {
            participant_id: participant_id.clone(),
            credential,
            detached: false,
        },
    )?;
    store::put(
        staged,
        store::binding_key(&conversation_id, &participant_id),
        &binding,
        "binding",
    )?;
    store::put(
        staged,
        store::participant_key(&participant_id),
        &participant,
        "participant",
    )?;
    store::put(
        staged,
        store::conversation_key(&conversation_id),
        &conversation,
        "conversation",
    )?;
    Ok(Advanced { seq })
}

pub async fn unbind(
    staged: &mut StagedStore,
    actor: &Party,
    origin: &Origin,
    now: u64,
    conversation_id: String,
    participant_id: String,
    expected_credential: Credential,
) -> Result<Advanced, Error> {
    let participant = store::participant(staged, &participant_id)
        .await?
        .ok_or_else(|| Error::Module(format!("no participant {participant_id}")))?;
    let Some(mut binding) = store::binding(staged, &conversation_id, &participant_id).await? else {
        return Err(Error::Module(format!(
            "participant {participant_id} has no binding on {conversation_id}"
        )));
    };
    let by_owner = controls(&participant.owner, actor, origin);
    let by_service = !binding.detached && signed_by(origin, &binding.service_key);
    if !(by_owner || by_service) {
        return Err(Error::Module(
            "only the owner or the bound service key may unbind".into(),
        ));
    }
    if expected_credential != binding.credential {
        return Err(Error::Module(format!(
            "binding credential is {}, not the expected {expected_credential}",
            binding.credential
        )));
    }
    if binding.detached {
        return Err(Error::Module("binding is already detached".into()));
    }
    binding.detached = true;
    let mut conversation = require_conversation(staged, &conversation_id).await?;
    let seq = store::append_event(
        staged,
        &mut conversation,
        now,
        EventBody::BindingChanged {
            participant_id: participant_id.clone(),
            credential: binding.credential,
            detached: true,
        },
    )?;
    store::put(
        staged,
        store::binding_key(&conversation_id, &participant_id),
        &binding,
        "binding",
    )?;
    store::put(
        staged,
        store::conversation_key(&conversation_id),
        &conversation,
        "conversation",
    )?;
    Ok(Advanced { seq })
}
