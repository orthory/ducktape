//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `acl` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `ducktape-module-sdk`, so the module's
//! logic is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the drain gate rides the query lane
//!
//! acl only HOLDS policy; the enforcement point is the kernel host's drain,
//! which consults `AclQuery::PolicyFor` before every `Origin::External` op
//! reaches its target. as a wasm tenant that consultation is a host-routed
//! guest query per gated op. the port stays equivalent because the native
//! query reads staged-over-committed store state — exactly the view the
//! host's staged overlay serves a guest — and policy writes are governance
//! module-origin follow-ups whose origin gate reads the wit `env.origin`
//! verbatim.

use crate::{Acl, DEFAULT_ACL_ID};

use ducktape_module_sdk::WitStore;

/// the sibling id this instance authorizes through — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): only governance's own follow-up may
/// author a policy change.
const GOVERNANCE_ID: &str = "governance";

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
// no genesis config: the policy table is EMPTY (= allow-all) at genesis and
// only tightens through governance follow-ups.
ducktape_module_sdk::store_guest! {
    id: DEFAULT_ACL_ID,
    module: Acl,
    shape: ducktape_module_sdk::store_shape(),
    new: Acl::new(DEFAULT_ACL_ID, Box::new(WitStore), GOVERNANCE_ID),
}
