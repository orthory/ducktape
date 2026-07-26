//! Node-local loopback upstreams for globally signed gateway routes.
//!
//! Consensus stores account/name/publisher/policy. This canonical file stores
//! the one fact that must remain local: which exact loopback port the
//! publisher selected. It is reloaded for every request so bind/unbind is
//! immediate and needs no consensus or daemon restart.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config;

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
    // Per-process temp name. This file is now read-modify-written on a service
    // daemon's 10 s heartbeat, so a FIXED name lets that beat and an operator's
    // `gateway bind` interleave until one rename publishes the other's bytes.
    //
    // ponytail: the read-modify-write itself is still last-writer-wins across
    // processes — a lost update, not a torn file. Take a lock only if a second
    // route-writing daemon ever appears.
    let temporary = workspace.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
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


/// the `ducktape gateway` verbs — node-local loopback route management.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum GatewayCmd {
    /// register (or update) a loopback route (apex or --label) to --port
    Bind(BindArgs),
    /// remove a route (prints its local key)
    Unbind(RouteArgs),
    /// print the local routes as one JSON array
    List(WorkspaceArgs),
}

/// which workspace's `gateway-routes.json` a verb edits: an explicit
/// `--workspace` wins, else `-n/--network` resolves through the registry.
#[derive(Debug, clap::Args)]
pub(crate) struct WorkspaceArgs {
    /// explicit workspace dir (wins over -n)
    #[arg(long, value_name = "DIR")]
    workspace: Option<PathBuf>,
    /// a registered workspace's chain id (`ducktape node list`)
    #[arg(short = 'n', long = "network", value_name = "CHAIN-ID")]
    network: Option<String>,
}

impl WorkspaceArgs {
    fn dir(&self) -> Result<PathBuf, String> {
        if let Some(dir) = &self.workspace {
            return Ok(dir.clone());
        }
        if let Some(needle) = &self.network {
            let (dir, _http) = config::resolve_network(needle)?;
            return Ok(dir);
        }
        Err("gateway route command needs --workspace <dir> or -n/--network <id>".into())
    }
}

/// route addressing: `--label api` names a subdomain route, absent = apex.
#[derive(Debug, clap::Args)]
pub(crate) struct RouteArgs {
    /// route label (subdomain); omit for the apex route
    #[arg(long, value_name = "LABEL")]
    label: Option<String>,
    #[command(flatten)]
    workspace: WorkspaceArgs,
}

impl RouteArgs {
    fn name(&self) -> Result<gateway::RouteName, String> {
        let name = match &self.label {
            Some(label) => gateway::RouteName::named(label.clone()),
            None => gateway::RouteName::apex(),
        };
        name.validate()?;
        Ok(name)
    }
}

#[derive(Debug, clap::Args)]
pub(crate) struct BindArgs {
    #[command(flatten)]
    route: RouteArgs,
    /// the loopback port the route proxies to (zero is not a port)
    #[arg(long, value_name = "PORT")]
    port: std::num::NonZeroU16,
}

/// Run one verb of the `ducktape gateway` family.
pub(super) fn run(cmd: GatewayCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        GatewayCmd::Bind(args) => bind(args),
        GatewayCmd::Unbind(args) => unbind(args),
        GatewayCmd::List(args) => list(args),
    }
}

fn bind(args: BindArgs) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = args.route.workspace.dir()?;
    let name = args.route.name()?;
    let port = args.port.get();
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

/// Who holds a route right now, from the point of view of the daemon serving
/// `port`. The ONE discriminant both the heartbeat re-assert and the exit
/// retire branch on.
///
/// Nothing stops two daemons of one kind on one workspace — `service run` takes
/// no lock and keeps no pidfile — and NAME-scoped writes make that pair
/// pathological: they flap the route between their ports every beat, and the
/// first to stop deletes the survivor's live entry, 404ing authorized overlay
/// ingress. Port-scoped, the newcomer simply wins and the loser goes quiet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteOwner {
    /// no entry: nothing registered it, or the operator unbound it by hand.
    Vacant,
    /// the entry names this daemon's own port.
    Ours,
    /// the entry names a different port — another daemon owns the route now.
    Foreign,
}

fn owner_of(workspace: &Path, name: &gateway::RouteName, port: u16) -> Result<RouteOwner, String> {
    let Some(registered) = load(workspace)?.port(name) else {
        return Ok(RouteOwner::Vacant);
    };
    let is_ours = registered == port;
    if is_ours {
        return Ok(RouteOwner::Ours);
    }
    Ok(RouteOwner::Foreign)
}

/// Re-assert `name -> port` on a daemon's heartbeat, so the port the node
/// proxies to cannot disagree with the port this process serves on for longer
/// than one beat. Returns what the file said BEFORE: only `Vacant` writes — an
/// operator's `gateway unbind`, corrected within one beat.
///
/// `Ours` is already exactly the instruction we want, and rewriting it every
/// beat would re-open the read-modify-write window for no change. `Foreign`
/// yields to the newer daemon rather than flapping the entry between two ports.
pub fn reassert(
    workspace: &Path,
    name: &gateway::RouteName,
    port: u16,
) -> Result<RouteOwner, String> {
    let owner = owner_of(workspace, name, port)?;
    match owner {
        RouteOwner::Vacant => register(workspace, name.clone(), port)?,
        RouteOwner::Ours => {}
        RouteOwner::Foreign => {}
    }
    Ok(owner)
}

/// Retire a daemon's own loopback route on the way out — the programmatic twin
/// of [`unbind`], scoped to the `port` that proves ownership. Returns the owner
/// as it was BEFORE the removal: only `Ours` is removed. A route that is already
/// absent is success (the operator may have unbound it by hand), and a `Foreign`
/// one is LEFT ALONE — deleting it would 404 a live daemon's ingress.
pub fn retire(
    workspace: &Path,
    name: &gateway::RouteName,
    port: u16,
) -> Result<RouteOwner, String> {
    let owner = owner_of(workspace, name, port)?;
    match owner {
        RouteOwner::Vacant | RouteOwner::Foreign => {}
        RouteOwner::Ours => remove(workspace, name)?,
    }
    Ok(owner)
}

fn remove(workspace: &Path, name: &gateway::RouteName) -> Result<(), String> {
    let mut routes = load(workspace)?;
    let Ok(index) = routes.routes.binary_search_by(|route| route.name.cmp(name)) else {
        return Ok(());
    };
    routes.routes.remove(index);
    save(workspace, &routes)
}

fn unbind(args: RouteArgs) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = args.workspace.dir()?;
    let name = args.name()?;
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

fn list(args: WorkspaceArgs) -> Result<(), Box<dyn std::error::Error>> {
    let routes = load(&args.dir()?)?;
    println!("{}", serde_json::to_string(&routes.routes).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(dir: &Path, label: Option<&str>, network: Option<&str>) -> RouteArgs {
        RouteArgs {
            label: label.map(String::from),
            workspace: WorkspaceArgs {
                workspace: Some(dir.to_path_buf()),
                network: network.map(String::from),
            },
        }
    }

    fn bind_args(dir: &Path, label: Option<&str>, network: Option<&str>, port: u16) -> BindArgs {
        BindArgs {
            route: route(dir, label, network),
            port: std::num::NonZeroU16::new(port).expect("test port is non-zero"),
        }
    }

    #[test]
    fn apex_and_named_routes_are_canonical_and_leave_no_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        bind(bind_args(dir.path(), None, None, 3000)).unwrap();
        bind(bind_args(dir.path(), Some("api"), None, 4000)).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.port(&gateway::RouteName::apex()), Some(3000));
        assert_eq!(
            loaded.port(&gateway::RouteName::named("api")),
            Some(4000)
        );

        unbind(route(dir.path(), None, None)).unwrap();
        unbind(route(dir.path(), Some("api"), None)).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(bind(bind_args(dir.path(), Some("evil.name"), None, 1)).is_err());
    }

    /// A restarted daemon comes back on a FRESH ephemeral port, so registering
    /// the same route name twice must replace the entry rather than add one —
    /// the file refuses duplicate names outright.
    #[test]
    fn registering_a_route_name_twice_replaces_the_entry_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let name = gateway::RouteName::named("airlock");
        register(dir.path(), name.clone(), 4100).unwrap();
        register(dir.path(), name.clone(), 4100).unwrap();
        register(dir.path(), name.clone(), 4200).unwrap();
        assert_eq!(load(dir.path()).unwrap().routes.len(), 1);
        assert_eq!(load(dir.path()).unwrap().port(&name), Some(4200));
    }

    /// A daemon's port is a standing instruction to the node's reverse proxy,
    /// and two daemons on one workspace are prevented NOWHERE. So both the beat
    /// and the exit are scoped to the port that proves ownership: name-scoped,
    /// the pair would flap the route every beat and the first to stop would
    /// delete the survivor's live entry.
    #[test]
    fn a_route_is_owned_by_the_port_that_registered_it() {
        let dir = tempfile::tempdir().unwrap();
        let name = gateway::RouteName::named("airlock");

        // one daemon: its own beat holds the route.
        register(dir.path(), name.clone(), 4100).unwrap();
        assert_eq!(reassert(dir.path(), &name, 4100).unwrap(), RouteOwner::Ours);
        assert_eq!(load(dir.path()).unwrap().port(&name), Some(4100));

        // a second daemon starts and takes it. The first must now yield.
        register(dir.path(), name.clone(), 4200).unwrap();
        assert_eq!(reassert(dir.path(), &name, 4100).unwrap(), RouteOwner::Foreign);
        assert_eq!(
            load(dir.path()).unwrap().port(&name),
            Some(4200),
            "a yielded beat writes nothing — otherwise the two flap forever"
        );

        // ...and the first daemon's SIGTERM must not delete the survivor's entry.
        assert_eq!(retire(dir.path(), &name, 4100).unwrap(), RouteOwner::Foreign);
        assert_eq!(load(dir.path()).unwrap().port(&name), Some(4200));

        // the owner retires its own: gone, no husk file, and safe twice (the
        // operator may have unbound it by hand first).
        assert_eq!(retire(dir.path(), &name, 4200).unwrap(), RouteOwner::Ours);
        assert_eq!(load(dir.path()).unwrap().port(&name), None);
        assert!(!dir.path().join(FILE_NAME).exists());
        assert_eq!(retire(dir.path(), &name, 4200).unwrap(), RouteOwner::Vacant);

        // a hand `gateway unbind` is corrected on the next beat.
        assert_eq!(reassert(dir.path(), &name, 4200).unwrap(), RouteOwner::Vacant);
        assert_eq!(load(dir.path()).unwrap().port(&name), Some(4200));
    }

    #[test]
    fn explicit_workspace_wins_over_network() {
        // --workspace short-circuits before the registry, so a bogus -n never
        // resolves: the route lands in the explicit dir.
        let dir = tempfile::tempdir().unwrap();
        bind(bind_args(dir.path(), None, Some("no-such-workspace"), 3000)).unwrap();
        assert_eq!(
            load(dir.path()).unwrap().port(&gateway::RouteName::apex()),
            Some(3000)
        );
    }
}
