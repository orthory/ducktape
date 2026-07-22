//! The module composition topology — ONE source for the module id universe,
//! its logical wiring, its genesis-config schema, and the named genesis
//! selections every composer draws from.
//!
//! Before this module the same id lists lived in four places: node's
//! `MODULE_IDS`, simnode's `BASE_MODULE_IDS` + `VALSET_MODULE_IDS`, the noded
//! daemon's own `MODULE_IDS`, and demo's inline genesis vec. Each was a
//! hand-kept `[&str; N]` count array, and drift between them was the #706
//! accident class. Now the id universe + wiring + config live in one
//! [`ModuleTopology`] value and each backend's genesis set is a NAMED SELECTION
//! ([`PRODUCTION`], [`SIM_BASE`], [`SIM_VALSET`], [`DEMO`]) validated against it.
//!
//! This is a plan, NOT an app-hash. Instantiation stays per-backend on purpose:
//! node composes the selection over the wasm runtime, simnode/noded/demo compose
//! the SAME ids over native module structs, and the wasm and native roots differ
//! by design. One topology never means one app-hash — it means one place the
//! module SET (and the drift guard on it) lives.
//!
//! Home is `host` because every composer (node-bin, noded, simnode, demo)
//! already depends on it, `Host::genesis` and [`crate::LIFECYCLE_MODULE_ID`]
//! already live here, and the data is pure `&str` — host depends on no concrete
//! module impl, so a catalog of ids introduces no crate cycle.

/// A single module in the composition universe: its id plus the metadata that
/// used to be scattered across the composer sites.
pub struct ModuleSpec {
    /// The consensus-visible module id (the key in the host registry / app-hash).
    pub id: &'static str,
    /// The NATIVE sibling wiring the native composers (noded/simnode/demo) pass
    /// as constructor args — the concrete duplication this topology absorbs.
    /// A backend realizes an edge only when it also composes the target module
    /// (demo omits `pages`, so it does not realize `runs -> pages`). The wasm
    /// production guests compile in their OWN wiring, which can be richer than
    /// the native args (e.g. saga reads valset/capability inside the guest);
    /// that guest-internal wiring is not represented here.
    pub wiring: &'static [&'static str],
    /// The per-network genesis-config keys this module needs (empty = none).
    /// The VALUES are runtime (a `NetworkBindings`: the invite namespace + the
    /// identity chain id), delivered per-backend (a native constructor arg, or
    /// the wasm `__config` store) — this is only the schema of WHICH modules are
    /// network-bound and on WHICH keys.
    pub config: &'static [&'static str],
}

/// The module composition topology: the id universe (with per-module wiring +
/// config) and the four named genesis selections drawn from it.
pub struct ModuleTopology {
    /// Every module that appears in ANY selection, each with its wiring/config.
    pub modules: &'static [ModuleSpec],
    /// node's production genesis set (wasm backend), in status-report order.
    pub production: &'static [&'static str],
    /// simnode's + the noded daemon's default native set, in registry order.
    pub sim_base: &'static [&'static str],
    /// the system modules simnode's `--with-valset` appends AFTER `sim_base`.
    pub sim_valset: &'static [&'static str],
    /// the demo walkthrough's native genesis set.
    pub demo: &'static [&'static str],
}

impl ModuleTopology {
    /// The spec for `id`, if it is in the universe.
    pub fn spec(&self, id: &str) -> Option<&ModuleSpec> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// `id`'s native sibling wiring (empty if `id` is unknown or unwired).
    pub fn wiring(&self, id: &str) -> &'static [&'static str] {
        self.spec(id).map(|m| m.wiring).unwrap_or(&[])
    }

    /// `id`'s genesis-config keys (empty if `id` is unknown or not network-bound).
    pub fn config(&self, id: &str) -> &'static [&'static str] {
        self.spec(id).map(|m| m.config).unwrap_or(&[])
    }
}

/// genesis-config key: the per-network invite namespace (governance verifies
/// tokens/join proofs against it).
pub const CONFIG_INVITE: &str = "invite";
/// genesis-config key: the identity chain id (identity/gateway scope their
/// certificates and `.duck` routes to it).
pub const CONFIG_CHAIN_ID: &str = "chain_id";

const CHAIN_ID: &[&str] = &[CONFIG_CHAIN_ID];
const INVITE: &[&str] = &[CONFIG_INVITE];
const NONE: &[&str] = &[];

/// The module id universe with per-module wiring + config. Alphabetical by id
/// (order here is documentation only — selections carry the composer orders).
const MODULES: &[ModuleSpec] = &[
    ModuleSpec { id: "agent", wiring: &["saga", "runs"], config: NONE },
    ModuleSpec { id: "automations", wiring: &["chat", "tasks", "inbox"], config: NONE },
    ModuleSpec { id: "capability", wiring: NONE, config: NONE },
    ModuleSpec { id: "chat", wiring: &["tagging"], config: NONE },
    ModuleSpec { id: "directory", wiring: NONE, config: NONE },
    ModuleSpec { id: "dispatch", wiring: &["saga"], config: NONE },
    ModuleSpec { id: "files", wiring: NONE, config: NONE },
    ModuleSpec { id: "forge", wiring: &["chat"], config: NONE },
    ModuleSpec { id: "gateway", wiring: &["identity"], config: CHAIN_ID },
    ModuleSpec { id: "governance", wiring: &["valset", "lifecycle", "identity"], config: INVITE },
    ModuleSpec { id: "greeter", wiring: NONE, config: NONE },
    ModuleSpec { id: "hello", wiring: NONE, config: NONE },
    ModuleSpec { id: "identity", wiring: NONE, config: CHAIN_ID },
    ModuleSpec { id: "inbox", wiring: NONE, config: NONE },
    ModuleSpec { id: "kv", wiring: NONE, config: NONE },
    ModuleSpec { id: "lifecycle", wiring: &["valset"], config: NONE },
    ModuleSpec { id: "pages", wiring: &["tagging"], config: NONE },
    ModuleSpec {
        id: "runs",
        wiring: &["chat", "saga", "tagging", "dispatch", "agent", "tasks", "files", "pages"],
        config: NONE,
    },
    ModuleSpec { id: "saga", wiring: NONE, config: NONE },
    ModuleSpec { id: "tagging", wiring: &["runs"], config: NONE },
    ModuleSpec { id: "tasks", wiring: NONE, config: NONE },
    ModuleSpec { id: "valset", wiring: NONE, config: NONE },
];

/// node's production genesis set (20), in status-report order — every node runs
/// exactly these, so the set is in the app-hash. A module here is consensus
/// state forever; experiments live unwired in `crates/labs` and appear in no
/// selection.
pub const PRODUCTION: &[&str] = &[
    "pages",
    "chat",
    "forge",
    "valset",
    "governance",
    "lifecycle",
    "hello",
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
    "directory",
    "automations",
    "files",
    "agent",
    "runs",
];

/// the DEFAULT native set (14) simnode and the noded daemon compose at genesis,
/// in registry order — the daemon parity lane pins this against noded. Changing
/// it means changing the daemon.
pub const SIM_BASE: &[&str] = &[
    "chat",
    "saga",
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

/// the four system modules simnode's opt-in `--with-valset` genesis appends
/// AFTER `sim_base`, in registry order: the KV store, the membership registry
/// seeded with the genesis validators, governance (the sole authorized author
/// of valset change), and the lifecycle coordinator — whose mere registration
/// makes the host-injected once-per-block boundary `Advance` ride every block.
pub const SIM_VALSET: &[&str] = &["kv", "valset", "governance", "lifecycle"];

/// the demo walkthrough's native genesis set (17): the base collaboration set
/// plus kv/directory/greeter, minus the production-only wasm tenants
/// (pages/lifecycle/governance/capability/hello) the scripted demo does not
/// exercise.
pub const DEMO: &[&str] = &[
    "kv",
    "directory",
    "greeter",
    "forge",
    "chat",
    "valset",
    "saga",
    "dispatch",
    "tagging",
    "tasks",
    "identity",
    "gateway",
    "inbox",
    "files",
    "agent",
    "runs",
    "automations",
];

/// The one topology value composers read.
pub const TOPOLOGY: ModuleTopology = ModuleTopology {
    modules: MODULES,
    production: PRODUCTION,
    sim_base: SIM_BASE,
    sim_valset: SIM_VALSET,
    demo: DEMO,
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
        assert_eq!(PRODUCTION.len(), 20, "production is the 20-module set");
        assert_eq!(SIM_BASE.len(), 14, "sim_base is the default 14-module set");
        assert_eq!(SIM_VALSET.len(), 4, "sim_valset appends 4 system modules");
        assert_eq!(DEMO.len(), 17, "demo composes 17 modules");

        // exact membership (sorted — registration order is not consensus-relevant)
        assert_eq!(
            sorted(PRODUCTION),
            sorted(&[
                "agent", "automations", "capability", "chat", "directory", "dispatch", "files",
                "forge", "gateway", "governance", "hello", "identity", "inbox", "lifecycle",
                "pages", "runs", "saga", "tagging", "tasks", "valset",
            ])
        );
        assert_eq!(
            sorted(SIM_BASE),
            sorted(&[
                "agent", "automations", "chat", "dispatch", "files", "forge", "gateway",
                "identity", "inbox", "pages", "runs", "saga", "tagging", "tasks",
            ])
        );
        assert_eq!(sorted(SIM_VALSET), sorted(&["governance", "kv", "lifecycle", "valset"]));
        assert_eq!(
            sorted(DEMO),
            sorted(&[
                "agent", "automations", "chat", "dispatch", "directory", "files", "forge",
                "gateway", "greeter", "identity", "inbox", "kv", "runs", "saga", "tagging",
                "tasks", "valset",
            ])
        );
    }

    #[test]
    fn selections_have_no_duplicates() {
        for (name, sel) in [
            ("production", PRODUCTION),
            ("sim_base", SIM_BASE),
            ("sim_valset", SIM_VALSET),
            ("demo", DEMO),
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
    /// so the universe and the selections cannot drift apart, and wiring/config
    /// metadata exists for exactly the composed modules.
    #[test]
    fn universe_and_selections_cover_each_other() {
        let universe: BTreeSet<&str> = MODULES.iter().map(|m| m.id).collect();
        assert_eq!(universe.len(), MODULES.len(), "the module universe has a duplicate id");

        let used: BTreeSet<&str> = PRODUCTION
            .iter()
            .chain(SIM_BASE)
            .chain(SIM_VALSET)
            .chain(DEMO)
            .copied()
            .collect();
        assert_eq!(
            universe, used,
            "every spec must be composed by some selection and every composed id must have a spec"
        );
    }

    /// Wiring targets and config keys reference only real ids / known keys — a
    /// typo or a removed sibling fails the build's test gate, not a live node.
    #[test]
    fn wiring_and_config_are_referential() {
        let universe: BTreeSet<&str> = MODULES.iter().map(|m| m.id).collect();
        for spec in MODULES {
            for target in spec.wiring {
                assert!(
                    universe.contains(target),
                    "{}'s wiring target {target} is not a known module",
                    spec.id
                );
                assert_ne!(target, &spec.id, "{} wires to itself", spec.id);
            }
            for key in spec.config {
                assert!(
                    *key == CONFIG_CHAIN_ID || *key == CONFIG_INVITE,
                    "{}'s config key {key} is not a known network-binding key",
                    spec.id
                );
            }
        }
    }

    /// The network-bound modules are exactly identity/gateway (chain id) and
    /// governance (invite) — the `NetworkBindings` schema, pinned so a new
    /// network-bound module must register its keys here.
    #[test]
    fn network_bound_modules_are_pinned() {
        let bound: BTreeSet<&str> = MODULES
            .iter()
            .filter(|m| !m.config.is_empty())
            .map(|m| m.id)
            .collect();
        assert_eq!(bound, ["gateway", "governance", "identity"].into_iter().collect());
        assert_eq!(TOPOLOGY.config("identity"), CHAIN_ID);
        assert_eq!(TOPOLOGY.config("gateway"), CHAIN_ID);
        assert_eq!(TOPOLOGY.config("governance"), INVITE);
        assert_eq!(TOPOLOGY.config("chat"), NONE);
    }

    #[test]
    fn accessors_resolve_specs() {
        assert_eq!(TOPOLOGY.wiring("chat"), &["tagging"]);
        assert_eq!(TOPOLOGY.wiring("kv"), NONE);
        assert!(TOPOLOGY.spec("chat").is_some());
        assert!(TOPOLOGY.spec("not-a-module").is_none());
        assert_eq!(TOPOLOGY.wiring("not-a-module"), NONE);
    }
}
