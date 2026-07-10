use super::{
    BTreeMap, Digest, Error, PendingState, RUN_KEY_SEPARATOR, SagaOrigin, Sha256, StateRoot,
    TurnPolicy, dispatch_id_for,
};

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
        put_bytes(&mut out, p.agent_id.as_bytes());
        put_bytes(&mut out, p.channel_id.as_bytes());
        out.extend_from_slice(&p.anchor_seq.to_le_bytes());
        put_opt_u64(&mut out, p.thread_root);
        put_opt_string(&mut out, &p.job_id);
        out.extend_from_slice(&p.job_claim_height.to_le_bytes());
        put_origin(&mut out, &p.requester);
        out.extend_from_slice(&p.created_at.to_le_bytes());
    }

    out
}

/// the state-based commitment over the committed maps — shared by `root()`
/// and `install()` so the verification a snapshot must pass is definitionally
/// the same algorithm the live module answers with.
pub(super) fn committed_root(
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(watches, pending)).into())
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

type Committed = (BTreeMap<String, TurnPolicy>, BTreeMap<String, PendingState>);

pub(super) fn decode_committed(mut buf: &[u8]) -> Result<Committed, String> {
    // per-entry minimum sizes: a watch costs its id prefix and a policy
    // discriminant; a pending entry its three length prefixes, anchor, two
    // option tags, claim height, origin discriminant, and created_at.
    const MIN_WATCH_BYTES: u64 = 8 + 1;
    const MIN_PENDING_BYTES: u64 = 8 + 8 + 8 + 8 + 1 + 1 + 8 + 1 + 8;

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
        let agent_id = take_lp_string(&mut buf)?;
        let channel_id = take_lp_string(&mut buf)?;
        let anchor_seq = take_u64(&mut buf)?;
        let thread_root = take_opt_u64(&mut buf)?;
        let job_id = take_opt_string(&mut buf)?;
        let job_claim_height = take_u64(&mut buf)?;
        let requester = take_origin(&mut buf)?;
        let created_at = take_u64(&mut buf)?;
        let entry = PendingState {
            agent_id,
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

    if !buf.is_empty() {
        return Err("snapshot has trailing bytes".into());
    }
    Ok((watches, pending))
}
