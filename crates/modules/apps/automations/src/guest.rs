//! Store-backed wasm port over the same account-authorized automation logic.
//! The SDK shell owns initialization, per-dispatch staging and block finalization.

use crate::Automations;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "automations";
const CHAT_ID: &str = "chat";
const TASKS_ID: &str = "tasks";
const IDENTITY_ID: &str = "identity";
const ATTRIBUTION_ID: &str = "attribution";

use ducktape_module_sdk::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Automations,
    shape: ducktape_module_sdk::store_shape(),
    new: Automations::new(MODULE_ID, Box::new(WitStore), CHAT_ID, TASKS_ID, IDENTITY_ID, ATTRIBUTION_ID),
}
