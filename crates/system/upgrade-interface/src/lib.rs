//! the upgrade module's public wire surface — types only.
//!
//! the upgrade module holds the agreed `current_version`, the single pending
//! `Upgrade { name, activation_height, to_version }`, and the per-validator
//! readiness set — all folded into the app-hash. governance authorizes a
//! schedule/cancel by emitting a host-drained follow-up; each validator emits
//! `SignalReady` once running the new binary; the boundary `Advance` tick arms
//! or aborts. this crate carries the shapes and the ONE shared, pure
//! `effective_version` derivation so the module, host, and orchestrator never
//! hand-copy the arming predicate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// the coordinates of a scheduled upgrade. **at most one** is ever pending.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Upgrade {
    pub name: String,
    pub activation_height: u64,
    pub to_version: u32,
}

/// what an ingested op DOES to the upgrade module. the ORIGIN is the authority,
/// not the variant: schedule/cancel are governance/system-authored, `SignalReady`
/// is validator-authored, `Advance` is the system-injected boundary tick.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeMsg {
    /// authorize a pending upgrade. `Origin::Module("governance") | System` only.
    Schedule {
        name: String,
        activation_height: u64,
        to_version: u32,
    },
    /// clear a pending upgrade before its boundary. `Origin::Module | System` only.
    Cancel { name: String },
    /// a validator records readiness for the pending upgrade.
    /// `Origin::External(pubkey)` of a CURRENT boundary member only.
    SignalReady {
        name: String,
        to_version: u32,
        commitment: Option<Vec<u8>>,
    },
    /// the system-injected boundary tick, keyed on `env.height`: arm (flip) or
    /// abort (clear) the pending upgrade. `Origin::System` only.
    Advance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeQuery {
    /// the current version, the pending upgrade, and the readiness verdict.
    Status,
}

/// the readable projection of the upgrade module's state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpgradeStatus {
    pub current_version: u32,
    pub pending: Option<Upgrade>,
    /// the boundary member set the verdict was computed against, sorted. lets a
    /// caller (e.g. the host stamping seam) re-run the shared `effective_version`.
    pub members: Vec<Vec<u8>>,
    /// readiness keys, sorted.
    pub ready: Vec<Vec<u8>>,
    pub member_count: u64,
    pub ready_count: u64,
    /// `pending.is_some() && members non-empty && every boundary member ∈ ready`,
    /// derived from the shared `effective_version` predicate (no hand-copied logic).
    pub armed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeReply {
    Status(UpgradeStatus),
}

pub fn encode_msg(m: &UpgradeMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<UpgradeMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &UpgradeQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<UpgradeQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &UpgradeReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<UpgradeReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// the ONE arming predicate, as a pure/total function of committed coordinates:
/// no IO, clock, or RNG; identical on every node (live, replay, state-sync). the
/// module's `Advance` handler and its `effective_version(&self, ..)` helper both
/// route through this, so the version derivation can never drift from the arm
/// check (risk R4). returns `pending.to_version` when the pending upgrade is
/// ARMED at `height` — `pending.is_some()`, `height >= activation_height`,
/// `boundary_members` non-empty, and every boundary member present in `ready` —
/// otherwise `current`.
pub fn effective_version(
    height: u64,
    current: u32,
    pending: Option<&Upgrade>,
    boundary_members: &[Vec<u8>],
    ready: &BTreeMap<Vec<u8>, ()>,
) -> u32 {
    match pending {
        Some(up)
            if height >= up.activation_height
                && !boundary_members.is_empty()
                && boundary_members.iter().all(|m| ready.contains_key(m)) =>
        {
            up.to_version
        }
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt_msg(m: UpgradeMsg) {
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
    }

    #[test]
    fn msg_query_reply_round_trip_every_variant() {
        rt_msg(UpgradeMsg::Schedule {
            name: "forge-multi-repo".into(),
            activation_height: 100,
            to_version: 2,
        });
        rt_msg(UpgradeMsg::Cancel {
            name: "forge-multi-repo".into(),
        });
        rt_msg(UpgradeMsg::SignalReady {
            name: "forge-multi-repo".into(),
            to_version: 2,
            commitment: None,
        });
        rt_msg(UpgradeMsg::SignalReady {
            name: "x".into(),
            to_version: 3,
            commitment: Some(vec![1, 2, 3]),
        });
        rt_msg(UpgradeMsg::Advance);

        let q = UpgradeQuery::Status;
        assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);

        let r = UpgradeReply::Status(UpgradeStatus {
            current_version: 1,
            pending: Some(Upgrade {
                name: "n".into(),
                activation_height: 9,
                to_version: 2,
            }),
            members: vec![vec![1u8; 32], vec![2u8; 32]],
            ready: vec![vec![1u8; 32], vec![2u8; 32]],
            member_count: 2,
            ready_count: 2,
            armed: true,
        });
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }

    #[test]
    fn effective_version_truth_table() {
        let m1 = vec![1u8; 32];
        let m2 = vec![2u8; 32];
        let members = vec![m1.clone(), m2.clone()];
        let up = Upgrade {
            name: "n".into(),
            activation_height: 10,
            to_version: 2,
        };

        // no pending -> current.
        let empty: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        assert_eq!(effective_version(10, 1, None, &members, &empty), 1);

        // armed & height < activation -> current.
        let mut all: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        all.insert(m1.clone(), ());
        all.insert(m2.clone(), ());
        assert_eq!(effective_version(9, 1, Some(&up), &members, &all), 1);

        // armed & height >= activation & all members ready -> to_version.
        assert_eq!(effective_version(10, 1, Some(&up), &members, &all), 2);

        // a boundary member missing -> current.
        let mut partial: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        partial.insert(m1.clone(), ());
        assert_eq!(effective_version(10, 1, Some(&up), &members, &partial), 1);

        // empty boundary set -> current (never arm against no members).
        assert_eq!(effective_version(10, 1, Some(&up), &[], &all), 1);
    }
}
