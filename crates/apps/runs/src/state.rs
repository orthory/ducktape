use super::{
    AgentSession, BTreeMap, DelegationState, DelegationStatus, Digest, Error,
    MAX_ACTIONS_PER_SESSION, MAX_DELEGATION_REQUEST_ID_BYTES, MAX_DELEGATIONS_PER_RUN,
    PendingState, RUN_KEY_SEPARATOR, RunAuthority, SESSION_KEY_LEN, SagaOrigin, Sha256, StateRoot,
    TurnPolicy, delegation_id_for, dispatch_id_for,
};
use serde::de::DeserializeOwned;

// ---- canonical encoding -------------------------------------------------------
// u64-le counts, sorted keys, every field in declaration order: u64-le length
// prefixes for byte strings, single-byte discriminants for enums, a 0/1 tag
// byte for options, u64-le integers. this is the exact preimage
// [`Module::root`] hashes, so a snapshot and the root that must authenticate
// it cannot drift. no version byte — encoding changes are flag-day (design
// principle: no backwards compatibility).

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn put_opt_u64(out: &mut Vec<u8>, opt: Option<u64>) {
    match opt {
        None => out.push(0),
        Some(v) => {
            out.push(1);
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

fn put_opt_string(out: &mut Vec<u8>, opt: &Option<String>) {
    match opt {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            put_bytes(out, value.as_bytes());
        }
    }
}

fn put_opt_json<T: serde::Serialize>(out: &mut Vec<u8>, opt: &Option<T>) {
    match opt {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            put_bytes(
                out,
                &serde_json::to_vec(value).expect("committed state serializes"),
            );
        }
    }
}

fn put_origin(out: &mut Vec<u8>, origin: &SagaOrigin) {
    match origin {
        SagaOrigin::External(key) => {
            out.push(0);
            put_bytes(out, key);
        }
        SagaOrigin::Module(module) => {
            out.push(1);
            put_bytes(out, module.as_bytes());
        }
        SagaOrigin::System => out.push(2),
    }
}

pub(super) fn encode_committed(
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
    sessions: &BTreeMap<String, AgentSession>,
    delegations: &BTreeMap<String, DelegationState>,
) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(&(watches.len() as u64).to_le_bytes());
    for (channel, policy) in watches {
        put_bytes(&mut out, channel.as_bytes());
        match policy {
            TurnPolicy::Mention => out.push(0),
            TurnPolicy::All => out.push(1),
            TurnPolicy::Assigned(agent_id) => {
                out.push(2);
                put_bytes(&mut out, agent_id.as_bytes());
            }
            TurnPolicy::RoundRobin => out.push(3),
        }
    }

    out.extend_from_slice(&(pending.len() as u64).to_le_bytes());
    for (dispatch_id, p) in pending {
        put_bytes(&mut out, dispatch_id.as_bytes());
        put_bytes(&mut out, p.run_id.as_bytes());
        put_bytes(&mut out, p.agent_id.as_bytes());
        put_bytes(&mut out, p.workspace_agent_id.as_bytes());
        put_opt_json(&mut out, &p.authority);
        put_opt_string(&mut out, &p.delegation_id);
        put_bytes(&mut out, p.channel_id.as_bytes());
        out.extend_from_slice(&p.anchor_seq.to_le_bytes());
        put_opt_u64(&mut out, p.thread_root);
        put_opt_string(&mut out, &p.job_id);
        out.extend_from_slice(&p.job_claim_height.to_le_bytes());
        put_origin(&mut out, &p.requester);
        out.extend_from_slice(&p.created_at.to_le_bytes());
    }

    // the live agent sessions, keyed by run id. the action counter is the
    // spent budget AND the deterministic id salt, so it is committed like
    // every other field — a validator that replayed a different count would
    // mint different ids.
    out.extend_from_slice(&(sessions.len() as u64).to_le_bytes());
    for (run_id, s) in sessions {
        put_bytes(&mut out, run_id.as_bytes());
        put_bytes(&mut out, s.agent_id.as_bytes());
        put_bytes(&mut out, &s.session_key);
        out.extend_from_slice(&s.opened_at.to_le_bytes());
        out.extend_from_slice(&u64::from(s.actions).to_le_bytes());
    }

    out.extend_from_slice(&(delegations.len() as u64).to_le_bytes());
    for (delegation_id, delegation) in delegations {
        put_bytes(&mut out, delegation_id.as_bytes());
        put_bytes(
            &mut out,
            &serde_json::to_vec(delegation).expect("delegation state serializes"),
        );
    }

    out
}

/// the state-based commitment over the committed maps — shared by `root()`
/// and `install()` so the verification a snapshot must pass is definitionally
/// the same algorithm the live module answers with.
pub(super) fn committed_root(
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
    sessions: &BTreeMap<String, AgentSession>,
    delegations: &BTreeMap<String, DelegationState>,
) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(watches, pending, sessions, delegations)).into())
}

// ---- canonical decoding (UNTRUSTED input) ---------------------------------
// bounds are validated against the remaining input BEFORE any allocation,
// keys must be strictly ascending (one encoding per state, uniqueness for
// free), unknown discriminants/tags and trailing bytes are rejected. never
// panics on malformed input.

/// pull `n` bytes off the front of `buf`, checked before any slicing.
fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if n > buf.len() {
        return Err("snapshot truncated".into());
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn take_u64(buf: &mut &[u8]) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        take(buf, 8)?.try_into().expect("8 bytes"),
    ))
}

/// a length prefix, validated against the remaining input before the caller
/// allocates anything of that size.
fn take_len(buf: &mut &[u8]) -> Result<usize, String> {
    let n = take_u64(buf)?;
    if n > buf.len() as u64 {
        return Err("snapshot length prefix exceeds input".into());
    }
    Ok(n as usize)
}

fn take_lp_bytes(buf: &mut &[u8]) -> Result<Vec<u8>, String> {
    let len = take_len(buf)?;
    Ok(take(buf, len)?.to_vec())
}

fn take_lp_string(buf: &mut &[u8]) -> Result<String, String> {
    let len = take_len(buf)?;
    Ok(std::str::from_utf8(take(buf, len)?)
        .map_err(|_| "snapshot string is not utf-8".to_string())?
        .to_owned())
}

fn take_opt_u64(buf: &mut &[u8]) -> Result<Option<u64>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => Ok(Some(take_u64(buf)?)),
        t => Err(format!("snapshot has unknown option tag {t}")),
    }
}

fn take_opt_string(buf: &mut &[u8]) -> Result<Option<String>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => Ok(Some(take_lp_string(buf)?)),
        t => Err(format!("snapshot has unknown option tag {t}")),
    }
}

fn take_opt_json<T: DeserializeOwned>(buf: &mut &[u8]) -> Result<Option<T>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => serde_json::from_slice(&take_lp_bytes(buf)?)
            .map(Some)
            .map_err(|error| format!("snapshot json value failed to decode: {error}")),
        tag => Err(format!("snapshot has unknown option tag {tag}")),
    }
}

fn take_origin(buf: &mut &[u8]) -> Result<SagaOrigin, String> {
    match take(buf, 1)?[0] {
        0 => Ok(SagaOrigin::External(take_lp_bytes(buf)?)),
        1 => Ok(SagaOrigin::Module(take_lp_string(buf)?)),
        2 => Ok(SagaOrigin::System),
        d => Err(format!("snapshot has unknown origin discriminant {d}")),
    }
}

/// a section count, bounded by what the remaining input could possibly hold
/// given each entry's minimum encoded size — rejected before the loop builds
/// anything.
fn take_count(buf: &mut &[u8], min_entry_bytes: u64, what: &str) -> Result<u64, String> {
    let count = take_u64(buf)?;
    if count
        .checked_mul(min_entry_bytes)
        .is_none_or(|need| need > buf.len() as u64)
    {
        return Err(format!("snapshot {what} count exceeds input"));
    }
    Ok(count)
}

/// enforce strictly-ascending map keys while inserting.
fn insert_ascending<V>(map: &mut BTreeMap<String, V>, key: String, value: V) -> Result<(), String> {
    if let Some((last, _)) = map.iter().next_back()
        && last.as_str() >= key.as_str()
    {
        return Err("snapshot keys not strictly ascending".into());
    }
    map.insert(key, value);
    Ok(())
}

pub(super) fn contains_run_separator(value: &str) -> bool {
    value.contains(RUN_KEY_SEPARATOR)
}

pub(super) fn reject_run_separator(field: &str, value: &str) -> Result<(), Error> {
    if contains_run_separator(value) {
        return Err(Error::Module(format!(
            "{field} must not contain the reserved unit separator"
        )));
    }
    Ok(())
}

/// a decoded pending entry must derive exactly its own key: the dispatch id
/// is the hex sha256 of the run id its fields produce, and the field shapes
/// must match the chat/job keyspace they claim.
fn validate_decoded_pending(dispatch_id: &str, p: &PendingState) -> Result<(), String> {
    if contains_run_separator(&p.agent_id) {
        return Err("snapshot agent_id contains reserved unit separator".into());
    }
    if p.run_id.is_empty() {
        return Err("snapshot pending run id is empty".into());
    }
    if contains_run_separator(&p.workspace_agent_id) {
        return Err("snapshot workspace agent id contains reserved unit separator".into());
    }
    if p.delegation_id.is_some() && p.authority.is_none() {
        return Err("snapshot delegated run has an edge without scoped authority".into());
    }
    match &p.job_id {
        Some(job_id) => {
            if contains_run_separator(job_id) {
                return Err("snapshot job_id contains reserved unit separator".into());
            }
            if !p.channel_id.is_empty() || p.anchor_seq != 0 || p.thread_root.is_some() {
                return Err("snapshot job entry carries chat coordinates".into());
            }
        }
        None => {
            if p.job_claim_height != 0 {
                return Err("snapshot chat entry has non-zero job claim height".into());
            }
            if contains_run_separator(&p.channel_id) {
                return Err("snapshot channel_id contains reserved unit separator".into());
            }
        }
    }
    if dispatch_id != dispatch_id_for(&p.run_id()) {
        return Err("snapshot dispatch id does not match its run fields".into());
    }
    Ok(())
}

fn validate_decoded_delegations(
    pending: &BTreeMap<String, PendingState>,
    delegations: &BTreeMap<String, DelegationState>,
) -> Result<(), String> {
    let mut roots = BTreeMap::<&str, usize>::new();
    for (id, state) in delegations {
        let view = &state.view;
        if id != &view.delegation_id
            || id != &delegation_id_for(&view.caller_run_id, &view.request_id)
        {
            return Err("snapshot delegation id does not match its caller request".into());
        }
        if view.request_id.is_empty()
            || view.request_id.len() > MAX_DELEGATION_REQUEST_ID_BYTES
            || contains_run_separator(&view.request_id)
        {
            return Err("snapshot delegation request id is invalid".into());
        }
        if view.callee_agent_id != state.request.agent_id {
            return Err("snapshot delegation callee does not match its request".into());
        }
        if !pending.contains_key(&dispatch_id_for(&view.root_run_id)) {
            return Err("snapshot delegation root is not in flight".into());
        }
        if view.status == DelegationStatus::Pending {
            let child = pending
                .get(&dispatch_id_for(&view.callee_run_id))
                .ok_or_else(|| "snapshot pending delegation has no callee run".to_string())?;
            if child.delegation_id.as_deref() != Some(id.as_str()) {
                return Err("snapshot callee run points at a different delegation".into());
            }
        }
        *roots.entry(&view.root_run_id).or_default() += 1;
    }
    if roots.values().any(|count| *count > MAX_DELEGATIONS_PER_RUN) {
        return Err("snapshot delegation tree exceeds its hard budget".into());
    }
    for pending_run in pending.values() {
        let Some(id) = pending_run.delegation_id.as_deref() else {
            continue;
        };
        let edge = delegations
            .get(id)
            .ok_or_else(|| "snapshot delegated run names no call edge".to_string())?;
        if edge.view.status != DelegationStatus::Pending
            || edge.view.callee_run_id != pending_run.run_id
        {
            return Err("snapshot delegated run does not match its pending call edge".into());
        }
    }
    Ok(())
}

/// a decoded session must be one a live module could have staged: the key IS
/// the run id, the key length is the one this module admits, and the counter is
/// within the budget the action lane enforces. `pending` is the run's own
/// existence check — a session may never outlive its run, so a snapshot whose
/// session names no in-flight run is not one any honest node could produce.
fn validate_decoded_session(
    pending: &BTreeMap<String, PendingState>,
    s: &AgentSession,
) -> Result<(), String> {
    if s.session_key.len() != SESSION_KEY_LEN {
        return Err("snapshot session key is not a 32-byte ed25519 key".into());
    }
    if contains_run_separator(&s.agent_id) {
        return Err("snapshot session agent_id contains reserved unit separator".into());
    }
    if s.actions > MAX_ACTIONS_PER_SESSION {
        return Err("snapshot session has spent more than its action budget".into());
    }
    let entry = pending
        .get(&dispatch_id_for(&s.run_id))
        .ok_or_else(|| "snapshot session names no in-flight run".to_string())?;
    if entry.agent_id != s.agent_id {
        return Err("snapshot session agent does not match its run".into());
    }
    Ok(())
}

type Committed = (
    BTreeMap<String, TurnPolicy>,
    BTreeMap<String, PendingState>,
    BTreeMap<String, AgentSession>,
    BTreeMap<String, DelegationState>,
);

pub(super) fn decode_committed(mut buf: &[u8]) -> Result<Committed, String> {
    // per-entry minimum sizes: a watch costs its id prefix and a policy
    // discriminant; a pending entry its three length prefixes, anchor, two
    // option tags, claim height, origin discriminant, and created_at; a session
    // its three length prefixes, opened_at, and the action counter.
    const MIN_WATCH_BYTES: u64 = 8 + 1;
    const MIN_PENDING_BYTES: u64 = 8 + 8 + 8 + 8 + 1 + 1 + 8 + 8 + 1 + 1 + 8 + 1 + 8;
    const MIN_SESSION_BYTES: u64 = 8 + 8 + 8 + 8 + 8;
    const MIN_DELEGATION_BYTES: u64 = 8 + 8;

    let mut watches: BTreeMap<String, TurnPolicy> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_WATCH_BYTES, "watch")?;
    for _ in 0..count {
        let channel = take_lp_string(&mut buf)?;
        if contains_run_separator(&channel) {
            return Err("snapshot channel_id contains reserved unit separator".into());
        }
        let policy = match take(&mut buf, 1)?[0] {
            0 => TurnPolicy::Mention,
            1 => TurnPolicy::All,
            2 => TurnPolicy::Assigned(take_lp_string(&mut buf)?),
            3 => TurnPolicy::RoundRobin,
            d => return Err(format!("snapshot has unknown turn policy {d}")),
        };
        insert_ascending(&mut watches, channel, policy)?;
    }

    let mut pending: BTreeMap<String, PendingState> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_PENDING_BYTES, "pending")?;
    for _ in 0..count {
        let dispatch_id = take_lp_string(&mut buf)?;
        let run_id = take_lp_string(&mut buf)?;
        let agent_id = take_lp_string(&mut buf)?;
        let workspace_agent_id = take_lp_string(&mut buf)?;
        let authority = take_opt_json::<RunAuthority>(&mut buf)?;
        let delegation_id = take_opt_string(&mut buf)?;
        let channel_id = take_lp_string(&mut buf)?;
        let anchor_seq = take_u64(&mut buf)?;
        let thread_root = take_opt_u64(&mut buf)?;
        let job_id = take_opt_string(&mut buf)?;
        let job_claim_height = take_u64(&mut buf)?;
        let requester = take_origin(&mut buf)?;
        let created_at = take_u64(&mut buf)?;
        let entry = PendingState {
            run_id,
            agent_id,
            workspace_agent_id,
            authority,
            delegation_id,
            channel_id,
            anchor_seq,
            thread_root,
            job_id,
            job_claim_height,
            requester,
            created_at,
        };
        validate_decoded_pending(&dispatch_id, &entry)?;
        insert_ascending(&mut pending, dispatch_id, entry)?;
    }

    let mut sessions: BTreeMap<String, AgentSession> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_SESSION_BYTES, "session")?;
    for _ in 0..count {
        let run_id = take_lp_string(&mut buf)?;
        let agent_id = take_lp_string(&mut buf)?;
        let session_key = take_lp_bytes(&mut buf)?;
        let opened_at = take_u64(&mut buf)?;
        let actions = u32::try_from(take_u64(&mut buf)?)
            .map_err(|_| "snapshot session action count exceeds u32".to_string())?;
        let session = AgentSession {
            run_id: run_id.clone(),
            agent_id,
            session_key,
            opened_at,
            actions,
        };
        validate_decoded_session(&pending, &session)?;
        insert_ascending(&mut sessions, run_id, session)?;
    }

    let mut delegations: BTreeMap<String, DelegationState> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_DELEGATION_BYTES, "delegation")?;
    for _ in 0..count {
        let id = take_lp_string(&mut buf)?;
        let state: DelegationState = serde_json::from_slice(&take_lp_bytes(&mut buf)?)
            .map_err(|error| format!("snapshot delegation failed to decode: {error}"))?;
        insert_ascending(&mut delegations, id, state)?;
    }
    validate_decoded_delegations(&pending, &delegations)?;

    if !buf.is_empty() {
        return Err("snapshot has trailing bytes".into());
    }
    Ok((watches, pending, sessions, delegations))
}
