//! Genesis-composition parity: the sim's default (and `--with-valset`) genesis
//! sets ARE the `sim_base` (+ `sim_valset`) selections of the single-source
//! `topology`, composed through the SAME `noded::compose::compose` bin/node
//! runs. `bin/noded/tests/daemon_e2e.rs` pins the same `sim_base` against
//! noded; this pins the sim composer against the topology it draws from with a
//! golden hash,
//! and pins that composing `sim_valset` gives governance its code registry.

mod harness;

use harness::Sim;

/// The default 14-module sim genesis root-hash.
///
/// This is the SIM's number and only the sim's: `sim_base` excludes
/// `capability`, `hello`, `governance` and `lifecycle`, so it is NOT what a node
/// runs and it is NOT the consensus pin. That one is
/// `bin/node/src/host_state.rs`'s `GENESIS_ROOT_HASH`, over the production
/// module set — moving THAT is the flag day that matters. This constant guards
/// something narrower and still worth guarding: that composing the sim's
/// genesis is a pure function of the topology selection, so a change in how the
/// sim builds its host shows up here instead of silently under a scenario.
const DEFAULT_GENESIS_ROOT_HASH: &str =
    "49f49b10f7ee255509a02943c846cfb077d4a1081bf26d9f8e80716a9f4a323e";

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

    let want: Vec<String> = topology::SIM_BASE.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        module_ids(&status),
        want,
        "sim default genesis composes topology sim_base, in registry order"
    );
    let root = status["root_hash"].as_str().expect("root_hash is a string");
    assert_eq!(
        root, DEFAULT_GENESIS_ROOT_HASH,
        "the SIM genesis root hash moved.\n\
         \n\
         DID YOU MEAN TO? A module in `sim_base` was added/removed, a guest was \
         rebuilt, a genesis-seeded record changed — then yes: set \
         DEFAULT_GENESIS_ROOT_HASH to {root} in the SAME commit as the change \
         that moved it, and name that change in the commit message.\n\
         \n\
         DID YOU NOT? Then the sim's genesis CONSTRUCTION drifted from the \
         topology selection it is supposed to be a pure function of — look for a \
         change to how the sim builds its host, not to the module list (the id \
         list is already pinned by the assertion above).\n\
         \n\
         EITHER WAY this is NOT the consensus pin, and updating it proves \
         nothing about production: `sim_base` is 14 modules and excludes \
         capability/hello/governance/lifecycle. The number a network forks on is \
         GENESIS_ROOT_HASH in bin/node/src/host_state.rs — if that moved too, go \
         read its message instead."
    );
}

#[test]
fn with_valset_genesis_appends_topology_sim_valset_and_wires_the_code_registry() {
    // any 32-byte value is an accepted genesis validator key (the binary checks
    // length, not curve membership).
    let key = "11".repeat(32);
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto", "--with-valset", &key]);
    let status = sim.status();

    let want: Vec<String> = topology::SIM_BASE
        .iter()
        .chain(topology::SIM_VALSET)
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        module_ids(&status),
        want,
        "--with-valset appends topology sim_valset after sim_base, in registry order"
    );

    // governance composes as a WASM tenant now, so the lifecycle code registry
    // it is wired to comes with it: an UpdateModule proposal opens a ballot
    // instead of being refused at the door. the code hash names no component
    // the network has — lifecycle refuses that at execute, which is a different
    // (and later) gate; the claim here is only that a registry exists at all.
    let propose = governance::GovMsg::Propose {
        proposal_id: "u".into(),
        action: governance::GovAction::UpdateModule {
            name: "x".into(),
            module_id: "chat".into(),
            activation_height: 10_000,
            code_hash: vec![0; 32],
        },
        voting_period: 600_000,
    };
    let (code, reply) = sim.submit(
        "governance",
        serde_json::to_value(propose).expect("Propose serializes"),
        // the genesis validator's own `hex:` origin — the electorate gate wants
        // a member NODE key, and only the escape can express raw key bytes.
        Some(&format!("hex:{key}")),
    );
    assert_eq!(
        code, 200,
        "an UpdateModule proposal must open a ballot in the sim: {reply}"
    );

    // and the ballot is really THERE: governance took the action into committed
    // state, so the registry is wired all the way through, not just past the door.
    let proposal = sim.query(
        "governance",
        serde_json::json!({ "proposal": { "proposal_id": "u" } }),
    );
    assert_eq!(
        proposal["proposal"]["status"], "open",
        "the UpdateModule proposal is open for votes: {proposal}"
    );
    assert_eq!(
        proposal["proposal"]["action"]["update_module"]["module_id"], "chat",
        "the open ballot carries the proposed swap: {proposal}"
    );
}
