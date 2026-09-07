//! The module's pure logic over the host-owned authenticated store.
use crate::Modules;
use ducktape_module_sdk::WitStore;

const MODULE_ID: &str = "modules";

ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Modules,
    shape: ducktape_module_sdk::store_shape(),
    new: Modules::new(MODULE_ID, Box::new(WitStore), "valset", "governance"),
}
