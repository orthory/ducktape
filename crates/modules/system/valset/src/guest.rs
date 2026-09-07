//! The module's pure logic over the host-owned authenticated store.
use crate::Valset;
use ducktape_module_sdk::WitStore;

const MODULE_ID: &str = "valset";

ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Valset,
    shape: ducktape_module_sdk::store_shape(),
    new: Valset::new(MODULE_ID, Box::new(WitStore), "governance"),
}
