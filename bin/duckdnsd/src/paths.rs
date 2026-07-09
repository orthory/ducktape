use std::net::SocketAddr;
use std::path::PathBuf;

pub const DEFAULT_CONTROL_ADDRESS: &str = "127.77.0.1:45853";

pub fn configured_state_dir() -> PathBuf {
    std::env::var_os("DUCKTAPE_DUCKDNS_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(default_state_dir)
}

pub fn configured_control_address() -> Result<SocketAddr, String> {
    std::env::var("DUCKTAPE_DUCKDNS_CONTROL")
        .unwrap_or_else(|_| DEFAULT_CONTROL_ADDRESS.into())
        .parse()
        .map_err(|error| format!("invalid DuckDNS control address: {error}"))
}

pub fn default_state_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
        return std::path::Path::new(&program_data)
            .join("Ducktape")
            .join("duckdnsd");
    }
    #[cfg(target_os = "macos")]
    {
        return PathBuf::from("/Library/Application Support/Ducktape/duckdnsd");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return PathBuf::from("/var/lib/ducktape/duckdnsd");
    }
    #[allow(unreachable_code)]
    PathBuf::from("duckdnsd-state")
}
