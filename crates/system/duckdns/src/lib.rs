//! `.duck` account naming service.
//!
//! The deterministic side owns canonical `.duck` account names, handle
//! ownership, query shapes, canonical state bytes, and root calculation;
//! resolution stops at the stable AccountId. The module adapter derives
//! AccountId from authenticated submit nodes through `identity` and gates
//! namespace mutations through `valset`.

mod codec;
mod module;
mod names;
mod registry;
mod wire;

pub use module::DuckDns;
pub use names::{parse_hostname, validate_handle};
pub use registry::Registry;
pub use wire::*;
