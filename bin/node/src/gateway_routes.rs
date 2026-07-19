//! Node-local loopback upstreams for globally signed gateway routes.
//!
//! Consensus stores account/name/publisher/policy. This canonical file stores
//! the one fact that must remain local: which exact loopback port the
//! publisher selected. It is reloaded for every request so bind/unbind is
//! immediate and needs no consensus or daemon restart.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli_flags::parse_flags;

pub const FILE_NAME: &str = "gateway-routes.json";
const FORMAT_VERSION: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalRoute {
    pub name: gateway::RouteName,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalRoutes {
    pub version: u8,
    pub routes: Vec<LocalRoute>,
}

impl Default for LocalRoutes {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            routes: Vec::new(),
        }
    }
}

impl LocalRoutes {
    fn validate(&self) -> Result<(), String> {
        if self.version != FORMAT_VERSION {
            return Err(format!(
                "gateway routes: unsupported format version {}",
                self.version
            ));
        }
        if self.routes.len() > gateway::MAX_ROUTES_PER_ACCOUNT {
            return Err(format!(
                "gateway routes: at most {} local routes",
                gateway::MAX_ROUTES_PER_ACCOUNT
            ));
        }
        let mut previous: Option<&gateway::RouteName> = None;
        for route in &self.routes {
            route.name.validate()?;
            if route.port == 0 {
                return Err("gateway routes: loopback port must be non-zero".into());
            }
            if previous.is_some_and(|old| old >= &route.name) {
                return Err("gateway routes: names must be unique and sorted".into());
            }
            previous = Some(&route.name);
        }
        Ok(())
    }

    pub fn port(&self, name: &gateway::RouteName) -> Option<u16> {
        self.routes
            .binary_search_by(|route| route.name.cmp(name))
            .ok()
            .map(|index| self.routes[index].port)
    }
}

pub fn load(workspace: &Path) -> Result<LocalRoutes, String> {
    let path = workspace.join(FILE_NAME);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LocalRoutes::default());
        }
        Err(error) => return Err(format!("read {path:?}: {error}")),
    };
    let routes: LocalRoutes =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode {path:?}: {error}"))?;
    routes.validate()?;
    let canonical = serde_json::to_vec_pretty(&routes).expect("routes serialize");
    if canonical != bytes {
        return Err(format!("{path:?} is not canonical; re-bind its routes"));
    }
    Ok(routes)
}

fn save(workspace: &Path, routes: &LocalRoutes) -> Result<(), String> {
    routes.validate()?;
    std::fs::create_dir_all(workspace).map_err(|error| format!("create {workspace:?}: {error}"))?;
    let path = workspace.join(FILE_NAME);
    let temporary = workspace.join(format!(".{FILE_NAME}.tmp"));
    if routes.routes.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove {path:?}: {error}")),
        }
        let _ = std::fs::remove_file(&temporary);
        return Ok(());
    }
    let bytes = serde_json::to_vec_pretty(routes).expect("routes serialize");
    std::fs::write(&temporary, bytes).map_err(|error| format!("write {temporary:?}: {error}"))?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("replace {path:?}: {error}"));
    }
    Ok(())
}

fn workspace(flags: &std::collections::BTreeMap<String, String>) -> Result<PathBuf, String> {
    flags
        .get("workspace")
        .map(PathBuf::from)
        .ok_or_else(|| "gateway route command needs --workspace <dir>".into())
}

fn name(flags: &std::collections::BTreeMap<String, String>) -> Result<gateway::RouteName, String> {
    let name = match flags.get("label") {
        Some(label) => gateway::RouteName::named(label.clone()),
        None => gateway::RouteName::apex(),
    };
    name.validate()?;
    Ok(name)
}

pub fn dispatch(command: &str, args: &[String]) -> Option<Result<(), Box<dyn std::error::Error>>> {
    let result = match command {
        "gateway-route-bind" => bind(args),
        "gateway-route-unbind" => unbind(args),
        "gateway-route-list" => list(args),
        _ => return None,
    };
    Some(result)
}

fn bind(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (positional, flags) = parse_flags(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected args: {positional:?}").into());
    }
    let workspace = workspace(&flags)?;
    let name = name(&flags)?;
    let port: u16 = flags
        .get("port")
        .ok_or("gateway-route-bind needs --port <loopback-port>")?
        .parse()
        .map_err(|error| format!("gateway route port: {error}"))?;
    if port == 0 {
        return Err("gateway route port must be non-zero".into());
    }
    let mut routes = load(&workspace)?;
    match routes
        .routes
        .binary_search_by(|route| route.name.cmp(&name))
    {
        Ok(index) => routes.routes[index].port = port,
        Err(index) => routes
            .routes
            .insert(index, LocalRoute { name: name.clone(), port }),
    }
    save(&workspace, &routes)?;
    println!("{}", name.local_key());
    Ok(())
}

/// Register or update a node-local loopback route `name -> port` at boot — the
/// programmatic equivalent of the `gateway-route-bind` CLI, for services the node
/// runs itself (e.g. an embedded airlock gateway on an ephemeral port).
pub fn register(
    workspace: &Path,
    name: gateway::RouteName,
    port: u16,
) -> Result<(), String> {
    if port == 0 {
        return Err("gateway route port must be non-zero".into());
    }
    name.validate()?;
    let mut routes = load(workspace)?;
    match routes.routes.binary_search_by(|route| route.name.cmp(&name)) {
        Ok(index) => routes.routes[index].port = port,
        Err(index) => routes.routes.insert(index, LocalRoute { name, port }),
    }
    save(workspace, &routes)
}

fn unbind(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (positional, flags) = parse_flags(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected args: {positional:?}").into());
    }
    let workspace = workspace(&flags)?;
    let name = name(&flags)?;
    let mut routes = load(&workspace)?;
    let index = routes
        .routes
        .binary_search_by(|route| route.name.cmp(&name))
        .map_err(|_| format!("gateway route {:?} does not exist", name.label))?;
    routes.routes.remove(index);
    save(&workspace, &routes)?;
    println!("{}", name.local_key());
    Ok(())
}

fn list(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (positional, flags) = parse_flags(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected args: {positional:?}").into());
    }
    let routes = load(&workspace(&flags)?)?;
    println!("{}", serde_json::to_string(&routes.routes).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn apex_and_named_routes_are_canonical_and_leave_no_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy();
        bind(&args(&["--workspace", &root, "--port", "3000"])).unwrap();
        bind(&args(&[
            "--workspace",
            &root,
            "--label",
            "api",
            "--port",
            "4000",
        ]))
        .unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.port(&gateway::RouteName::apex()), Some(3000));
        assert_eq!(
            loaded.port(&gateway::RouteName::named("api")),
            Some(4000)
        );

        unbind(&args(&["--workspace", &root])).unwrap();
        unbind(&args(&["--workspace", &root, "--label", "api"])).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(
            bind(&args(&[
                "--workspace",
                &root,
                "--label",
                "evil.name",
                "--port",
                "1",
            ]))
            .is_err()
        );
    }
}
