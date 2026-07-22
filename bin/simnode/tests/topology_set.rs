//! Genesis-composition parity: the sim's default (and `--with-valset`) genesis
//! sets ARE the `sim_base` (+ `sim_valset`) selections of the single-source
//! `host::topology`, and the default set's app-hash is byte-identical to the
//! pre-topology composition. The daemon parity lane pins the same `sim_base`
//! against noded; this pins the sim composer against the topology it now draws
//! from, and guards C4's "no construction change" invariant with a golden hash.

mod harness;

use harness::Sim;

/// The default 14-native-module sim genesis app-hash, captured from the
/// pre-C4 composition. C4 moved only the id-LIST source (into `host::topology`);
/// the native genesis vec is untouched, so this must stay byte-identical — a
/// change here means a construction change slipped in with the id-list swap.
const DEFAULT_GENESIS_APP_HASH: &str =
    "0a81571979950afc077681d9645f368eb8eed5a938c217245db7ef65f4e51b0d";

fn module_ids(status: &serde_json::Value) -> Vec<String> {
    status["modules"]
        .as_array()
        .expect("status carries a modules array")
        .iter()
        .map(|m| m["id"].as_str().expect("module id is a string").to_string())
        .collect()
}

#[test]
fn default_genesis_composes_topology_sim_base() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &[]);
    let status = sim.status();

    let want: Vec<String> = host::topology::SIM_BASE.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        module_ids(&status),
        want,
        "sim default genesis composes topology sim_base, in registry order"
    );
    assert_eq!(
        status["app_hash"].as_str().expect("app_hash is a string"),
        DEFAULT_GENESIS_APP_HASH,
        "default sim genesis app-hash must be byte-identical across the topology swap"
    );
}

#[test]
fn with_valset_genesis_appends_topology_sim_valset() {
    // any 32-byte value is an accepted genesis validator key (the binary checks
    // length, not curve membership).
    let key = "11".repeat(32);
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--with-valset", &key]);
    let status = sim.status();

    let want: Vec<String> = host::topology::SIM_BASE
        .iter()
        .chain(host::topology::SIM_VALSET)
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        module_ids(&status),
        want,
        "--with-valset appends topology sim_valset after sim_base, in registry order"
    );
}
