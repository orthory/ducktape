use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use commonware_utils::ordered::Set;

use host::Host;

/// read the valset module's current membership projection (committed state —
/// called between drains, outside any block).
pub(crate) async fn read_valset_members(host: &Host) -> Vec<Vec<u8>> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::Validators))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::Validators(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// read the valset module's current RESIDENT projection (committed state —
/// called between drains, outside any block; same read point as
/// [`read_valset_members`], so a boundary read sees one frozen state).
pub(crate) async fn read_valset_residents(host: &Host) -> Vec<Vec<u8>> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::Residents))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::Residents(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// read the clients module's current CLIENT set (committed state — called
/// between drains, outside any block). the submit door admits a client's
/// own-signed frame; this is a SEPARATE read from the valset residents (the
/// clients module is deliberately not valset), so a client's standing can never
/// leak into the statesync/mesh reads keyed off valset.
pub(crate) async fn read_clients(host: &Host) -> Vec<Vec<u8>> {
    use clients::{ClientsQuery, ClientsReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("clients", &encode_query(&ClientsQuery::Clients))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ClientsReply::Clients(v)) => v,
        Err(_) => Vec::new(),
    }
}

/// the transport-mesh set a parked joiner tracks at a manifest's epoch. it MUST
/// be the same set every member tracks at that epoch — a validator tracks
/// `descriptor_mesh ∪ members ∪ residents` (see the `mesh_at` closure in the
/// validator boot below) — because `authenticated::discovery` KILLS a peer
/// whose bit-vector length disagrees at a shared index. the manifest carries
/// members (`participants`) and residents (`residents`) as separate lists, so a
/// joiner that folds only `participants` tracks a SHORTER set than every member
/// the moment any resident is granted, and discovery tears the link down on
/// every gossip round (a resident redeeming its own grant is exactly this case:
/// the founder counts it, the joiner does not). the descriptor mesh already
/// carries the lobby key. undecodable keys are dropped (dead serving hints).
pub(crate) fn joiner_epoch_mesh(
    descriptor_mesh: &[ed25519::PublicKey],
    participants: &[Vec<u8>],
    residents: &[Vec<u8>],
) -> Set<ed25519::PublicKey> {
    let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
        descriptor_mesh.iter().cloned().collect();
    // fold BOTH lists: members AND residents. every validator tracks the epoch
    // as `descriptor_mesh ∪ members ∪ residents`; omitting residents here would
    // leave the joiner one short and get it killed on every discovery round.
    for k in participants.iter().chain(residents.iter()) {
        if let Ok(pk) = ed25519::PublicKey::decode(k.as_slice()) {
            union.insert(pk);
        }
    }
    Set::try_from(union.into_iter().collect::<Vec<_>>())
        .expect("a btree-set union has no duplicates")
}

/// read the upgrade module's committed state as the boundary snapshot the
/// orchestrator reads at a finalized boundary (committed state — called between
/// drains, outside any block). the readiness keys are projected into decoded
/// ed25519 pubkeys (an undecodable key is dropped — dead weight, exactly like the
/// module). falls back to the baseline (no pending) when the module is absent
/// (pre-retrofit) or the reply is unreadable, so this never forks on a decode slip
/// — matching `Host::effective_version`'s graceful fallback.
pub(crate) async fn read_upgrade_state(host: &Host) -> consensus::BoundaryUpgrade<ed25519::PublicKey> {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let baseline = || consensus::BoundaryUpgrade::baseline(host::BASELINE_VERSION);
    let Ok(reply) = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
    else {
        return baseline();
    };
    let reply = match decode_reply(&reply) {
        Ok(r) => r,
        Err(_) => return baseline(),
    };
    let UpgradeReply::Status(status) = reply;
    let pending = status.pending.map(|up| {
        let ready: std::collections::BTreeSet<ed25519::PublicKey> = status
            .ready
            .iter()
            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
            .collect();
        consensus::PendingUpgrade {
            name: up.name,
            activation_height: up.activation_height,
            to_version: up.to_version,
            ready,
        }
    });
    consensus::BoundaryUpgrade {
        current_version: status.current_version,
        pending,
    }
}

/// read the upgrade module's committed `current_version` + single pending upgrade
/// as the manifest MIRROR the recovery/statesync captures carry (committed state —
/// called between drains, outside any block). falls back to the baseline (version 0,
/// no pending) when the module is absent (pre-retrofit) or the reply is unreadable,
/// so a checkpoint is never mis-stamped into a decode slip. keeping this a pure
/// committed read (not the raw orchestrator state) means a checkpoint captures the
/// same fields a live node would derive at that height.
pub(crate) async fn read_upgrade_version_fields(host: &Host) -> (u32, Option<sdk::UpgradeCoords>) {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let baseline = (host::BASELINE_VERSION, None);
    let Ok(reply) = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
    else {
        return baseline;
    };
    let reply = match decode_reply(&reply) {
        Ok(r) => r,
        Err(_) => return baseline,
    };
    let UpgradeReply::Status(status) = reply;
    let pending = status.pending.map(|up| sdk::UpgradeCoords {
        name: up.name,
        activation_height: up.activation_height,
        to_version: up.to_version,
    });
    (status.current_version, pending)
}

/// read the upgrade module's raw committed [`UpgradeStatus`] (committed state,
/// between drains). `None` when the module is absent (pre-retrofit) or the reply
/// is unreadable — so the transition-marker latches degrade to silent on a
/// baseline net, never panicking.
pub(crate) async fn read_upgrade_status_raw(host: &Host) -> Option<upgrade::UpgradeStatus> {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let reply = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
        .ok()?;
    let UpgradeReply::Status(status) = decode_reply(&reply).ok()?;
    Some(status)
}

/// read the governance module's committed invite redemptions — the
/// exactly-once nonce set (committed+staged projection, between drains). an
/// unreadable reply degrades to empty: the lobby then simply cannot pre-empt
/// a spent invite, and the in-consensus exactly-once check still holds.
pub(crate) async fn read_redemptions_from_host(host: &Host) -> Vec<governance::RedemptionView> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("governance", &encode_query(&GovQuery::Redemptions))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(GovReply::Redemptions(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
}

pub(crate) fn resume_member_keys(
    resumed: Option<&recovery::Recovered>,
    validators: &[ed25519::PublicKey],
) -> Result<Vec<ed25519::PublicKey>, String> {
    let raw: Vec<Vec<u8>> = match resumed {
        Some(rec) => rec.participants.clone(),
        None => validators.iter().map(|k| k.as_ref().to_vec()).collect(),
    };
    let mut keys = Vec::with_capacity(raw.len());
    for k in &raw {
        keys.push(
            ed25519::PublicKey::decode(k.as_slice())
                .map_err(|e| format!("recovered participant set holds a non-ed25519 key: {e}"))?,
        );
    }
    Ok(keys)
}

/// the recovered epoch's RESIDENT keys — empty on a fresh boot (genesis has
/// no residents) and on checkpoints written before the staged-admission tier.
pub(crate) fn resume_resident_keys(
    resumed: Option<&recovery::Recovered>,
) -> Result<Vec<ed25519::PublicKey>, String> {
    let raw: Vec<Vec<u8>> = match resumed {
        Some(rec) => rec.residents.clone(),
        None => Vec::new(),
    };
    let mut keys = Vec::with_capacity(raw.len());
    for k in &raw {
        keys.push(
            ed25519::PublicKey::decode(k.as_slice())
                .map_err(|e| format!("recovered resident set holds a non-ed25519 key: {e}"))?,
        );
    }
    Ok(keys)
}
