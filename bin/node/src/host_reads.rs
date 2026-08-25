use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;

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

/// read the valset module's retained mesh-generation window (committed state —
/// called between drains, outside any block; same read point as
/// [`read_valset_members`]). unreadable degrades to EMPTY: callers treat an
/// empty window per their role (a validator fail-stops, a poller retries).
pub(crate) async fn read_valset_mesh_window(host: &Host) -> Vec<valset::GenerationSet> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::MeshWindow))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::MeshWindow(window)) => window,
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// the committed mesh window in its sync-wire shape: `(latest generation,
/// entries)` — the fill for [`statesync::TipCoords`] / `BoundaryCoords`.
/// statesync stays valset-agnostic, so the conversion lives here.
pub(crate) async fn read_sync_mesh_window(host: &Host) -> (u64, Vec<statesync::MeshWindowEntry>) {
    let window = read_valset_mesh_window(host).await;
    let generation = window.last().map(|s| s.generation).unwrap_or(0);
    let entries = window
        .into_iter()
        .map(|s| statesync::MeshWindowEntry {
            generation: s.generation,
            validators: s.validators,
            residents: s.residents,
        })
        .collect();
    (generation, entries)
}

/// read ONE committed invite redemption by token nonce — the exactly-once
/// set's point read (committed+staged projection, between drains). an
/// unreadable reply degrades to `None`: the gate then simply cannot pre-empt
/// a spent invite, and the in-consensus exactly-once check still holds.
pub(crate) async fn read_redemption_from_host(
    host: &Host,
    nonce: &[u8],
) -> Option<governance::RedemptionView> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let reply = host
        .query(
            "governance",
            &encode_query(&GovQuery::Redemption {
                nonce: nonce.to_vec(),
            }),
        )
        .await
        .ok()?;
    match decode_reply(&reply) {
        Ok(GovReply::Redemption(view)) => view,
        Ok(_) | Err(_) => None,
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
