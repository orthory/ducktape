//! bindings — which principal carries a participant's deliveries on one
//! channel. everything here is gated on the AUTHENTICATED origin, never on a
//! name carried in the payload, and the roster it consults is chat's.

use sdk::{Ctx, Error, Origin, StagedStore};

use crate::interface::{
    Binding, BoundPrincipal, Credential, EventBody, MAX_ID_BYTES, MAX_LABEL_BYTES,
    MAX_SERVICE_KEY_BYTES, Party,
};
use crate::store;

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

/// a participant is a PERSON party — an account or a bare key. a module or
/// the system holds no mailbox and binds no device.
pub fn check_participant(what: &str, party: &Party) -> Result<(), Error> {
    let shaped = match party {
        Party::Account(_) => true,
        Party::Key(key) => !key.is_empty(),
        Party::Module(_) | Party::System => false,
    };
    if !shaped {
        return Err(Error::Module(format!(
            "{what} must be an account or a non-empty key"
        )));
    }
    Ok(())
}

/// a principal must be one an origin can actually equal. an EMPTY service key
/// would otherwise sit in committed state matching nothing — a binding that
/// looks live and can never authenticate — and an unbounded one is a record
/// nobody bounded.
pub fn check_principal(principal: &BoundPrincipal) -> Result<(), Error> {
    let BoundPrincipal::ServiceKey(key) = principal else {
        return Ok(());
    };
    let shaped = !key.is_empty() && key.len() <= MAX_SERVICE_KEY_BYTES;
    if !shaped {
        return Err(Error::Module(format!(
            "a bound service key must be 1..={MAX_SERVICE_KEY_BYTES} bytes"
        )));
    }
    Ok(())
}

/// is THIS dispatch's origin the principal the binding authorized? the match
/// is against the ORIGIN THE HOST MINTED and nothing on the wire.
///
/// One `match` over the principal, and the arms are exhaustive on purpose:
/// `Origin::Module` and `Origin::System` fall through every arm, so neither
/// the module in the middle of a follow-up nor the public read lane can ever
/// pass as an attached principal.
pub fn authenticates(origin: &Origin, principal: &BoundPrincipal) -> bool {
    match principal {
        // an empty key would match an `External(vec![])` origin no honest
        // signer produces; refused at bind, and refused again here.
        BoundPrincipal::ServiceKey(key) => {
            matches!(origin, Origin::External(signer) if !signer.is_empty() && signer == key)
        }
        // only the dispatch call lane mints this origin, and only for an
        // account identity holds as a program at an unmoved generation.
        BoundPrincipal::Program(account) => {
            matches!(origin, Origin::Program(caller) if caller == account)
        }
    }
}

/// the participant's live binding on one channel, if it holds one that has
/// not been detached.
pub async fn live_binding(
    staged: &StagedStore,
    channel_id: &str,
    participant: &Party,
) -> Result<Option<Binding>, Error> {
    Ok(store::binding(staged, channel_id, participant)
        .await?
        .filter(|binding| !binding.detached))
}

/// the outcome an op reports back so the module can stamp it and notify.
pub struct Advanced {
    pub seq: u64,
}

/// a service key is seated on the channel for exactly as long as it is bound:
/// chat's `PostPolicy::MembersOnly` would otherwise refuse the device's posts.
/// a program principal is seated by whoever grants it a seat, never here.
fn seat(ctx: &mut dyn Ctx, chat: &str, channel_id: &str, principal: &BoundPrincipal, member: bool) {
    let BoundPrincipal::ServiceKey(key) = principal else {
        return;
    };
    ctx.emit_msg(sdk::Msg {
        target: chat.to_string(),
        payload: chat::encode_msg(&chat::ChatMsg::SetMembership {
            channel_id: channel_id.to_string(),
            party: Party::Key(key.clone()),
            member,
        }),
    });
}

#[allow(clippy::too_many_arguments)]
pub async fn bind(
    staged: &mut StagedStore,
    ctx: &mut dyn Ctx,
    chat: &str,
    now: u64,
    channel_id: String,
    participant: Party,
    device: String,
    principal: BoundPrincipal,
    expected_credential: Credential,
) -> Result<Advanced, Error> {
    check_id("channel_id", &channel_id)?;
    check_participant("participant", &participant)?;
    check_label("device", &device)?;
    check_principal(&principal)?;
    // any authenticated member issues the scoped credential, a service key
    // for itself included. what the binding authorizes is the PRINCIPAL it
    // names, never its issuer; the credential CAS below is what keeps one
    // binding per participant per channel.
    let access = crate::chat_access(ctx, chat, &channel_id, &participant).await?;
    if !access.may_read {
        return Err(Error::Module(format!(
            "participant may not read channel {channel_id}"
        )));
    }
    let current = store::binding(staged, &channel_id, &participant).await?;
    let current_credential = current.as_ref().map_or(0, |binding| binding.credential);
    if expected_credential != current_credential {
        return Err(Error::Module(format!(
            "binding credential is {current_credential}, not the expected {expected_credential}"
        )));
    }
    let credential = current_credential
        .checked_add(1)
        .ok_or_else(|| Error::Module("binding credentials exhausted".into()))?;

    let binding = Binding {
        channel_id: channel_id.clone(),
        participant: participant.clone(),
        credential,
        principal: principal.clone(),
        device,
        attached_at: now,
        detached: false,
    };
    let seq = store::append_event(
        staged,
        &channel_id,
        now,
        EventBody::BindingChanged {
            participant: participant.clone(),
            credential,
            detached: false,
        },
    )
    .await?;
    store::put(
        staged,
        store::binding_key(&channel_id, &participant),
        &binding,
        "binding",
    )?;
    // a replaced live service key loses its seat with its credential.
    if let Some(replaced) = current.filter(|binding| !binding.detached) {
        seat(ctx, chat, &channel_id, &replaced.principal, false);
    }
    seat(ctx, chat, &channel_id, &principal, true);
    Ok(Advanced { seq })
}

pub async fn unbind(
    staged: &mut StagedStore,
    ctx: &mut dyn Ctx,
    chat: &str,
    now: u64,
    channel_id: String,
    participant: Party,
    expected_credential: Credential,
) -> Result<Advanced, Error> {
    // any authenticated member releases any binding, naming the credential it
    // releases.
    let Some(mut binding) = store::binding(staged, &channel_id, &participant).await? else {
        return Err(Error::Module(format!(
            "participant has no binding on {channel_id}"
        )));
    };
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
    let seq = store::append_event(
        staged,
        &channel_id,
        now,
        EventBody::BindingChanged {
            participant: participant.clone(),
            credential: binding.credential,
            detached: true,
        },
    )
    .await?;
    store::put(
        staged,
        store::binding_key(&channel_id, &participant),
        &binding,
        "binding",
    )?;
    seat(ctx, chat, &channel_id, &binding.principal, false);
    Ok(Advanced { seq })
}
