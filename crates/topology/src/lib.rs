//! The module composition topology — ONE source for the module id universe
//! and the named genesis selections every composer draws from.
//!
//! No composer keeps a hand-counted id list of its own: the id universe lives
//! in one [`ModuleTopology`] value, and each backend's genesis set is a NAMED
//! SELECTION validated against it — [`PRODUCTION`] for the node, [`SIM_BASE`]
//! and [`SIM_VALSET`] for simnode.
//!
//! A row says WHICH module, WHERE ITS CODE COMES FROM, and whether it ships an
//! index guest — nothing more. Most host-facing facts (the substrate its
//! state lives on, the network config it seeds, its query mode) stay the
//! module's own declaration (a wasm component's `shape` export,
//! `wasm_host::Shape`) rather than a row here, since a table that repeated
//! those would be a second source of truth for something with no access
//! problem — nothing outside the crate needs to know it before the crate
//! itself is built, and it would hold nothing for a module the registry
//! admits after genesis.
//!
//! `has_index_guest` is the one exception: it MUST be readable by a composer
//! that only holds a directory of bare wasm files (`workspace_config::genesis`,
//! composing `node init --modules <dir>` over an arbitrary operator
//! directory) with no access to any crate's source tree to read
//! `src/index_guest.rs` from. `crates/noded/build.rs` cross-checks this flag
//! against that same file at every build, so the two declarations cannot
//! drift apart unnoticed.
//!
//! Inter-module wiring is NOT here either: a module's guest compiles in the
//! siblings it reads, so the guest is the wiring.
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
/// component the modules registry can swap at a height boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Native,
    Wasm,
}

/// A single module in the composition universe.
pub struct ModuleSpec {
    /// The consensus-visible module id (the key in the host registry / root-hash).
    pub id: &'static str,
    /// Where this module's code comes from.
    pub code: Code,
    /// Whether this module's crate carries an index guest (`src/index_guest.rs`,
    /// staged by `crates/noded/build.rs` as `<id>.index.wasm`). A `--modules
    /// <dir>` founding set is checked against this flag — present-but-not-declared
    /// and declared-but-absent are both refused (`workspace_config::genesis`).
    pub has_index_guest: bool,
}

/// The module composition topology: the id universe and the three named
/// genesis selections drawn from it.
pub struct ModuleTopology {
    /// Every module that appears in ANY selection.
    pub modules: &'static [ModuleSpec],
    /// node's production genesis set (wasm backend).
    pub production: &'static [&'static str],
    /// simnode's + the noded daemon's default set.
    pub sim_base: &'static [&'static str],
    /// the system modules simnode's `--with-valset` appends AFTER `sim_base`.
    pub sim_valset: &'static [&'static str],
}

impl ModuleTopology {
    /// The spec for `id`, if it is in the universe.
    pub fn spec(&self, id: &str) -> Option<&ModuleSpec> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// the `code == Wasm` ids of `selection`, in selection order.
    pub fn wasm_ids(&self, selection: &[&'static str]) -> Vec<&'static str> {
        selection
            .iter()
            .copied()
            .filter(|id| self.spec(id).is_some_and(|m| m.code == Code::Wasm))
            .collect()
    }

    /// the ids of `selection` whose crate declares an index guest, in
    /// selection order — what a founding set MUST carry an `<id>.index.wasm`
    /// for (`workspace_config::genesis::Genesis::compose` refuses a set that
    /// omits one, or that carries one for an id not in this list).
    pub fn index_guest_ids(&self, selection: &[&'static str]) -> Vec<&'static str> {
        selection
            .iter()
            .copied()
            .filter(|id| self.spec(id).is_some_and(|m| m.has_index_guest))
            .collect()
    }
}

const fn wasm(id: &'static str) -> ModuleSpec {
    ModuleSpec {
        id,
        code: Code::Wasm,
        has_index_guest: false,
    }
}

const fn wasm_indexed(id: &'static str) -> ModuleSpec {
    ModuleSpec {
        id,
        code: Code::Wasm,
        has_index_guest: true,
    }
}

const fn native(id: &'static str) -> ModuleSpec {
    ModuleSpec {
        id,
        code: Code::Native,
        has_index_guest: false,
    }
}

/// The module id universe. Alphabetical by id (order here is documentation
/// only — selections carry the composer orders).
const MODULES: &[ModuleSpec] = &[
    wasm("acl"),
    wasm("agent"),
    wasm("automations"),
    wasm("capability"),
    wasm_indexed("chat"),
    wasm("dispatch"),
    wasm("files"),
    wasm("forge"),
    wasm("gateway"),
    wasm("governance"),
    wasm("identity"),
    wasm_indexed("inbox"),
    native("kv"),
    native("modules"),
    wasm_indexed("pages"),
    wasm("runs"),
    wasm_indexed("saga"),
    wasm("tagging"),
    wasm_indexed("tasks"),
    native("valset"),
];

/// node's production genesis set (19) — every node runs exactly these, so the
/// set is in the root-hash. A module here is consensus
/// state forever; experiments live unwired in `crates/labs` and appear in no
/// selection.
pub const PRODUCTION: &[&str] = &[
    "pages",
    "chat",
    "forge",
    "valset",
    "acl",
    "governance",
    "modules",
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

/// the DEFAULT set (15) simnode and the noded daemon compose at genesis —
/// `bin/noded/tests/daemon_e2e.rs` pins the same `sim_base` against noded.
/// Changing it means changing the daemon.
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
/// AFTER `sim_base`: the KV store, the membership registry
/// seeded with the genesis validators, the acl policy table (empty =
/// allow-all), governance (the sole authorized author of valset and acl
/// change), and the modules registry — whose mere registration makes the
/// host-injected once-per-block boundary `Advance` ride every block.
pub const SIM_VALSET: &[&str] = &["kv", "valset", "acl", "governance", "modules"];

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
                "modules", "pages", "runs", "saga", "tagging", "tasks", "valset",
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
            sorted(&["acl", "governance", "kv", "modules", "valset"])
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
    /// so the universe and the selections cannot drift apart.
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

    #[test]
    fn accessors_resolve_specs() {
        assert!(TOPOLOGY.spec("chat").is_some());
        assert!(TOPOLOGY.spec("not-a-module").is_none());
    }

    /// The `code` column is consensus-adjacent: a wrong `code` sends a native
    /// module to the wasm loader, or a wasm tenant to a constructor the
    /// composer does not have.
    #[test]
    fn code_column_pins_the_natives() {
        let native: Vec<&str> = MODULES
            .iter()
            .filter(|m| m.code == Code::Native)
            .map(|m| m.id)
            .collect();
        assert_eq!(sorted(&native), ["kv", "modules", "valset"]);
    }

    #[test]
    fn wasm_ids_selects_only_wasm_specs_in_selection_order() {
        let ids = TOPOLOGY.wasm_ids(SIM_VALSET);
        assert_eq!(ids, ["acl", "governance"]);
    }

    /// pins today's index-guest-shipping set — the same 5 crates that carry
    /// `src/index_guest.rs` and that `crates/noded/build.rs` cross-checks this
    /// flag against at every build.
    #[test]
    fn index_guest_ids_selects_only_the_declared_shippers() {
        let ids = TOPOLOGY.index_guest_ids(PRODUCTION);
        assert_eq!(
            sorted(&ids),
            sorted(&["chat", "inbox", "pages", "saga", "tasks"])
        );
    }
}
