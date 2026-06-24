mod hydrator;
mod journal;
mod config;
mod hydratable;

pub use hydrator::*;
pub use hydratable::*;
// re-exported so downstream crates (e.g. engine's Node) can name the hydrator's
// config and the `Batch` handed to an `OnHydrate` callback. these are pure api
// surface, not a domain dependency — hydration stays op-agnostic.
pub use config::Config;
pub use config::cadence::Config as CadenceConfig;
pub use config::journal::Config as JournalConfig;
pub use journal::Batch;
