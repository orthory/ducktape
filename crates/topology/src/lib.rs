//! The module composition topology — ONE source for the module id universe,
//! its shape, its genesis-config schema, and the named genesis selections
//! every composer draws from.
//!
//! No composer keeps a hand-counted id list of its own: the id universe, its
//! shape and its config schema live in one [`ModuleTopology`] value, and each
//! backend's genesis set is a NAMED SELECTION validated against it —
//! [`PRODUCTION`] for the node, [`SIM_BASE`] and [`SIM_VALSET`] for simnode.
//!
//! Inter-module wiring is NOT here and must not come back: a module's guest
//! compiles in the siblings it reads, so the guest is the wiring. A table of
//! edges nothing loads is a second source of truth that cannot be caught
//! being wrong.
//!
//! This is a plan, NOT a root-hash. Every backend instantiates it through the
//! ONE composer (`noded::compose`) — each spec's `code` decides wasm component
//! or native struct, identically for node, noded and simnode — but their roots
//! still differ, because a root is composed from a SELECTION and its genesis
//! bindings, not from this catalog. One topology never means one root-hash — it
//! means one place the module SET (and the drift guard on it) lives.
//!
//! A leaf crate with no dependencies: the catalog is pure `&'static str`, and
//! the kernel (`host`) knows nothing of the product modules composed over it.

/// Where a module's CODE comes from: compiled into the binary, or a wasm
/// component the code registry (lifecycle) can swap at a height boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Native,
    Wasm,
}

/// Where a module's COMMITTED state lives — the substrate `root()` is computed
/// from. One per module by definition (`wasm_host::StateBacking` is an enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    /// host-KV map; root = sha256(canonical kv); rides the snapshot lane.
    Map,
    /// host-constructed authenticated store (qmdb); root = store merkle root.
    Store,
    /// host-side disk substrate (duckfs odb / git); root = sha256(refs image).
    Odb,
}

/// A single module in the composition universe: its id plus the metadata that
/// used to be scattered across the composer sites.
pub struct ModuleSpec {
    /// The consensus-visible module id (the key in the host registry / root-hash).
    pub id: &'static str,
    /// The per-network genesis-config keys this module needs (empty = none).
    /// The VALUES are runtime (a `NetworkBindings`: the invite namespace + the
    /// identity chain id), delivered per-backend (a native constructor arg, or
    /// the wasm `__config` store) — this is only the schema of WHICH modules are
    /// network-bound and on WHICH keys.
    pub config: &'static [&'static str],
    /// Where this module's code comes from.
    pub code: Code,
    /// Where this module's committed state lives.
    pub backing: Backing,
    /// the guest's query lane is COMMITTED-ONLY regardless of caller
    /// (`WasmModule::with_committed_queries`). dispatch only.
    pub committed_queries: bool,
}

/// The module composition topology: the id universe (with per-module shape +
/// config) and the three named genesis selections drawn from it.
pub struct ModuleTopology {
    /// Every module that appears in ANY selection, each with its shape/config.
    pub modules: &'static [ModuleSpec],
    /// node's production genesis set (wasm backend), in status-report order.
    pub production: &'static [&'static str],
    /// simnode's + the noded daemon's default native set, in registry order.
    pub sim_base: &'static [&'static str],
    /// the system modules simnode's `--with-valset` appends AFTER `sim_base`.
    pub sim_valset: &'static [&'static str],
}

impl ModuleTopology {
    /// The spec for `id`, if it is in the universe.
    pub fn spec(&self, id: &str) -> Option<&ModuleSpec> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// `id`'s genesis-config keys (empty if `id` is unknown or not network-bound).
    pub fn config(&self, id: &str) -> &'static [&'static str] {
        self.spec(id).map(|m| m.config).unwrap_or(&[])
    }

    /// the `code == Wasm` ids of `selection`, in selection order.
    pub fn wasm_ids(&self, selection: &[&'static str]) -> Vec<&'static str> {
        selection
            .iter()
            .copied()
            .filter(|id| self.spec(id).is_some_and(|m| m.code == Code::Wasm))
            .collect()
    }
}

/// genesis-config key: the per-network invite namespace (governance verifies
/// tokens/join proofs against it).
pub const CONFIG_INVITE: &str = "invite";
/// genesis-config key: the identity chain id (identity/gateway scope their
/// certificates and `.duck` routes to it; `runs` stamps the `?net=` half of
/// every `duck://` link it renders into an agent's context with it).
pub const CONFIG_CHAIN_ID: &str = "chain_id";

const CHAIN_ID: &[&str] = &[CONFIG_CHAIN_ID];
const INVITE: &[&str] = &[CONFIG_INVITE];
const NONE: &[&str] = &[];

const fn store(id: &'static str, config: &'static [&'static str]) -> ModuleSpec {
    ModuleSpec { id, config, code: Code::Wasm, backing: Backing::Store, committed_queries: false }
}

/// The module id universe with per-module shape + config. Alphabetical by id
/// (order here is documentation only — selections carry the composer orders).
const MODULES: &[ModuleSpec] = &[
    store("acl", NONE),
    store("agent", NONE),
    store("automations", NONE),
    store("capability", NONE),
    store("chat", NONE),
    ModuleSpec { id: "dispatch", config: NONE, code: Code::Wasm, backing: Backing::Store, committed_queries: true },
    ModuleSpec { id: "files", config: NONE, code: Code::Wasm, backing: Backing::Odb, committed_queries: false },
    ModuleSpec { id: "forge", config: NONE, code: Code::Wasm, backing: Backing::Odb, committed_queries: false },
    store("gateway", CHAIN_ID),
    store("governance", INVITE),
    store("identity", CHAIN_ID),
    store("inbox", NONE),
    ModuleSpec { id: "kv", config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
    ModuleSpec { id: "lifecycle", config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
    store("pages", NONE),
    ModuleSpec { id: "runs", config: CHAIN_ID, code: Code::Wasm, backing: Backing::Map, committed_queries: false },
    store("saga", NONE),
    store("tagging", NONE),
    store("tasks", NONE),
    ModuleSpec { id: "valset", config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
];

/// node's production genesis set (19), in status-report order — every node runs
/// exactly these, so the set is in the root-hash. A module here is consensus
/// state forever; experiments live unwired in `crates/labs` and appear in no
/// selection.
pub const PRODUCTION: &[&str] = &[
    "pages",
    "chat",
    "forge",
    "valset",
    "acl",
    "governance",
    "lifecycle",
    "saga",
    "capability",
    "dispatch",
    "tagging",
    "tasks",
    "identity",
    // the MERGED gateway owns the whole `.duck` name -> AccountId -> route
    // pipeline: the route plane PLUS the human-name handle plane the retired
    // `duckdns` module used to own separately.
    "gateway",
    "inbox",
    "automations",
    "files",
    "agent",
    "runs",
];

/// the DEFAULT set (15) simnode and the noded daemon compose at genesis, in
/// registry order — `bin/noded/tests/daemon_e2e.rs` pins the same `sim_base`
/// against noded. Changing it means changing the daemon.
pub const SIM_BASE: &[&str] = &[
    "chat",
    "saga",
    // saga's guest is wired to `capability` (Strict lease over ANNOUNCED providers);
    // without it every tagged saga degrades to accept-any and the sim cannot
    // reproduce production's provider gate.
    "capability",
    "dispatch",
    "tagging",
    "tasks",
    "inbox",
    "automations",
    "agent",
    "runs",
    "pages",
    "forge",
    "files",
    "identity",
    // the MERGED gateway owns both the `.duck` handle plane and the route plane.
    "gateway",
];

/// the five system modules simnode's opt-in `--with-valset` genesis appends
/// AFTER `sim_base`, in registry order: the KV store, the membership registry
/// seeded with the genesis validators, the acl policy table (empty =
/// allow-all), governance (the sole authorized author of valset and acl
/// change), and the lifecycle coordinator — whose mere registration makes the
/// host-injected once-per-block boundary `Advance` ride every block.
pub const SIM_VALSET: &[&str] = &["kv", "valset", "acl", "governance", "lifecycle"];

/// The one topology value composers read.
pub const TOPOLOGY: ModuleTopology = ModuleTopology {
    modules: MODULES,
    production: PRODUCTION,
    sim_base: SIM_BASE,
    sim_valset: SIM_VALSET,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn has_dups(sel: &[&str]) -> bool {
        sel.len() != sel.iter().collect::<BTreeSet<_>>().len()
    }

    fn sorted(s: &[&'static str]) -> Vec<&'static str> {
        let mut v = s.to_vec();
        v.sort_unstable();
        v
    }

    /// The named selections pin to today's sets — the derivation guard: a change
    /// to a composer's genesis set that forgets this file (or vice versa) fails
    /// here. Counts AND membership, so neither a stray add nor a silent drop slips.
    #[test]
    fn selections_pin_to_todays_sets() {
        assert_eq!(PRODUCTION.len(), 19, "production is the 19-module set");
        assert_eq!(SIM_BASE.len(), 15, "sim_base is the default 15-module set");
        assert_eq!(SIM_VALSET.len(), 5, "sim_valset appends 5 system modules");

        // exact membership (sorted — registration order is not consensus-relevant)
        assert_eq!(
            sorted(PRODUCTION),
            sorted(&[
                "acl", "agent", "automations", "capability", "chat", "dispatch",
                "files", "forge", "gateway", "governance", "identity", "inbox",
                "lifecycle", "pages", "runs", "saga", "tagging", "tasks", "valset",
            ])
        );
        assert_eq!(
            sorted(SIM_BASE),
            sorted(&[
                "agent", "automations", "capability", "chat", "dispatch", "files", "forge",
                "gateway", "identity", "inbox", "pages", "runs", "saga", "tagging", "tasks",
            ])
        );
        assert_eq!(
            sorted(SIM_VALSET),
            sorted(&["acl", "governance", "kv", "lifecycle", "valset"])
        );
    }

    #[test]
    fn selections_have_no_duplicates() {
        for (name, sel) in [
            ("production", PRODUCTION),
            ("sim_base", SIM_BASE),
            ("sim_valset", SIM_VALSET),
        ] {
            assert!(!has_dups(sel), "{name} has a duplicate id");
        }
    }

    /// `sim_valset` is APPENDED to `sim_base` under `--with-valset`, so the two
    /// must be disjoint or the concatenation would double-register a module.
    #[test]
    fn sim_base_and_valset_are_disjoint() {
        let base: BTreeSet<&str> = SIM_BASE.iter().copied().collect();
        for id in SIM_VALSET {
            assert!(!base.contains(id), "sim_valset id {id} is already in sim_base");
        }
    }

    /// Every selection id has a spec, and every spec is used by some selection —
    /// so the universe and the selections cannot drift apart, and shape/config
    /// metadata exists for exactly the composed modules.
    #[test]
    fn universe_and_selections_cover_each_other() {
        let universe: BTreeSet<&str> = MODULES.iter().map(|m| m.id).collect();
        assert_eq!(universe.len(), MODULES.len(), "the module universe has a duplicate id");

        let used: BTreeSet<&str> = PRODUCTION
            .iter()
            .chain(SIM_BASE)
            .chain(SIM_VALSET)
            .copied()
            .collect();
        assert_eq!(
            universe, used,
            "every spec must be composed by some selection and every composed id must have a spec"
        );
    }

    /// Config keys reference only known keys — a typo fails the build's test
    /// gate, not a live node whose module reads an absent `__config` entry.
    #[test]
    fn config_keys_are_referential() {
        for spec in MODULES {
            for key in spec.config {
                assert!(
                    *key == CONFIG_CHAIN_ID || *key == CONFIG_INVITE,
                    "{}'s config key {key} is not a known network-binding key",
                    spec.id
                );
            }
        }
    }

    /// The network-bound modules are exactly identity/gateway/runs (chain id)
    /// and governance (invite) — the `NetworkBindings` schema, pinned so a new
    /// network-bound module must register its keys here.
    #[test]
    fn network_bound_modules_are_pinned() {
        let bound: BTreeSet<&str> = MODULES
            .iter()
            .filter(|m| !m.config.is_empty())
            .map(|m| m.id)
            .collect();
        assert_eq!(bound, ["gateway", "governance", "identity", "runs"].into_iter().collect());
        assert_eq!(TOPOLOGY.config("identity"), CHAIN_ID);
        assert_eq!(TOPOLOGY.config("gateway"), CHAIN_ID);
        assert_eq!(TOPOLOGY.config("runs"), CHAIN_ID);
        assert_eq!(TOPOLOGY.config("governance"), INVITE);
        assert_eq!(TOPOLOGY.config("chat"), NONE);
    }

    #[test]
    fn accessors_resolve_specs() {
        assert!(TOPOLOGY.spec("chat").is_some());
        assert!(TOPOLOGY.spec("not-a-module").is_none());
        assert_eq!(TOPOLOGY.config("not-a-module"), NONE);
    }

    /// The shape table is consensus-adjacent: a wrong `backing` composes the
    /// wrong root, a wrong `code` sends a native module to the wasm loader.
    #[test]
    fn shape_table_pins_native_odb_map_and_committed_queries() {
        let native: Vec<&str> = MODULES.iter().filter(|m| m.code == Code::Native).map(|m| m.id).collect();
        assert_eq!(sorted(&native), ["kv", "lifecycle", "valset"]);
        let odb: Vec<&str> = MODULES.iter().filter(|m| m.backing == Backing::Odb).map(|m| m.id).collect();
        assert_eq!(sorted(&odb), ["files", "forge"]);
        let map: Vec<&str> = MODULES.iter().filter(|m| m.backing == Backing::Map).map(|m| m.id).collect();
        assert_eq!(sorted(&map), ["runs"]);
        let committed: Vec<&str> = MODULES.iter().filter(|m| m.committed_queries).map(|m| m.id).collect();
        assert_eq!(committed, ["dispatch"]);
    }

    #[test]
    fn wasm_ids_selects_only_wasm_specs_in_selection_order() {
        let ids = TOPOLOGY.wasm_ids(SIM_VALSET);
        assert_eq!(ids, ["acl", "governance"]);
    }
}
