//! Device-local DuckDNS helper.
//!
//! `duckdnsd` owns the narrow machine boundary: authoritative split DNS for
//! `.duck`, a token-restricted active-workspace lease, the protected
//! device CA, SNI leaf certificates, and TLS-to-node HTTP handoff. It never
//! holds consensus state or DuckFS bytes.

mod control;
mod paths;
mod state;

#[cfg(feature = "server")]
mod ca;
#[cfg(feature = "server")]
mod dns;
#[cfg(feature = "server")]
mod https;
#[cfg(feature = "server")]
mod install;

#[cfg(feature = "server")]
pub use ca::CaStore;
pub use control::{
    ControlClient, ControlReply, ControlRequest, control_token_path, install_token,
    load_or_create_token, read_token, run_control,
};
#[cfg(feature = "server")]
pub use dns::{DnsHandler, run_dns};
#[cfg(feature = "server")]
pub use https::{LeafResolver, run_https, tls_config};
#[cfg(feature = "server")]
pub use install::{InstallationStatus, install, installation_status, uninstall};
pub use paths::{
    DEFAULT_CONTROL_ADDRESS, configured_control_address, configured_state_dir, default_state_dir,
};
pub use state::{ActiveWorkspace, IngressRoute, SharedState, SnapshotStatus};

pub const ROOT_CERT_FILE: &str = "root-ca.pem";
pub const ROOT_KEY_FILE: &str = "root-ca-key.der";
