//! the RESIDENT capability-announce lane, end to end on the network-shape
//! cluster: a fresh identity JOINS the founder's network with a live invite
//! (the product flow — no manual ceremony), lands RESIDENT standing, and —
//! without ever being promoted — publishes `grant ∩ live hello` into the
//! COMMITTED capability registry over the submit-relay lane, then settles its
//! own frame's consensus fate back through the resident tier's `on_outcome`
//! route. The founder is hermetically capability-free (no grant, no hello), so
//! the only possible provider is the resident.
//!
//! The offered half is a real `POST /v1/services/hello` against the resident's
//! own app surface, refreshed on a heartbeat — a service daemon's entire
//! contribution to THIS lane. The daemon PROCESS is deliberately not spawned:
//! `ducktape service run compute` resolves and `probe()`s its sandbox backend
//! before its first hello, so with no `[sandbox]` table in node.toml (no
//! generated test config writes one) and no podman + pasta + nft on PATH it
//! FATALs before signaling — a spawned daemon dies before this lane is
//! reached. The dispatch-EXECUTION leg this file used to carry moved into that
//! daemon with it and has no runnable home until the sandbox harness gains one.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_announce_e2e -- --nocapture --test-threads=1

mod common;

use std::time::Duration;

use capability::{CapabilityQuery, CapabilityReply};
use common::{NetworkShapeCluster, poll_until, serial};

/// generous like the sibling network-shape legs: standing → follow-arm sync →
/// announce relay → registry commit is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);

/// the tag the resident's grant consents to and its hello offers — the
/// intersection of the two is what must appear on chain.
const TAG: &str = "quack-resident";

/// the tag's committed provider pool on `idx`, sorted by key.
fn providers(cluster: &NetworkShapeCluster, idx: usize, tag: &str) -> Option<Vec<Vec<u8>>> {
    let reply = cluster.query(
        idx,
        "capability",
        &capability::encode_query(&CapabilityQuery::Providers {
            capability: tag.into(),
        }),
    )?;
    match capability::decode_reply(&reply) {
        Ok(CapabilityReply::Providers(p)) => Some(p),
        _ => None,
    }
}

/// serving is opt-in (default OFF), and the grant is the ONLY switch —
/// node.toml carries no announce key any more. It is also only HALF of it: the
/// offered set is `grant ∩ live hello`, so this pairs with `signal_service`.
fn opt_in_serving(cluster: &NetworkShapeCluster, idx: usize, tag: &str) {
    let workspace = cluster
        .config_file(idx)
        .parent()
        .expect("node.toml has a parent")
        .to_path_buf();
    std::fs::write(
        workspace.join("services.toml"),
        format!(
            "version = 1\n\n[[service]]\nkind = \"compute\"\ninstance = \"{}\"\n\
             nonce = \"{}\"\ngranted_unix = 1700000000\ncapabilities = [{tag:?}]\n\
             scopes = []\n",
            "11".repeat(32),
            "22".repeat(16),
        ),
    )
    .expect("write services.toml");
}

#[test]
fn a_joined_resident_announces_into_the_committed_registry() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("resident-announce");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the PRODUCT join flow, token kept: the parked joiner announces the
    // invite, a member redeems it automatically, resident standing lands.
    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(friend_key_hex.len(), 64, "join prints the friend's pubkey hex");
    let friend_key = common::unhex(&friend_key_hex);
    opt_in_serving(&cluster, 1, TAG);
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));
    cluster.wait_admitted(1, CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // the grant alone announces NOTHING — the live half has to exist too.
    assert_eq!(
        providers(&cluster, 0, TAG),
        Some(Vec::new()),
        "a grant with no daemon signaling puts nobody in the pool"
    );

    // the resident's compute plane starts signaling.
    cluster.signal_service(1, "compute", &[TAG]);

    // THE ANNOUNCE: without promotion, `grant ∩ live hello` reaches the
    // COMMITTED registry — relayed to the founder, admitted by the relaxed
    // member gate, applied in consensus.
    poll_until(
        "the resident's announce to land in the founder's registry",
        CONVERGE,
        || {
            let pool = providers(&cluster, 0, TAG)?;
            (pool == vec![friend_key.clone()]).then_some(())
        },
    );

    // and the KIND is announced beside the executor tag, which is the whole of
    // defect 2: "which nodes run compute" is a registry query now.
    assert_eq!(
        providers(&cluster, 0, "compute"),
        Some(vec![friend_key.clone()]),
        "the granted-and-signaling kind is a capability tag in its own right"
    );

    // the pump's own settle log confirms the reply lane round-tripped and the
    // resident tier's `on_outcome` route ran: the APPLIED line, the only one
    // that is true. (The submit-time line is `debug` on both tiers now —
    // relayed is not announced.)
    cluster.wait_marker(1, "resident: announced capabilities", CONVERGE);

    cluster.kill(1);
    cluster.kill(0);
}
