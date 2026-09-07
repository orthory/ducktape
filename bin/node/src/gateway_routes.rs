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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalRoute {
    /// The account this label is bound FOR — the operator's consent, and the
    /// other half of the serve-time key.
    ///
    /// Consensus lets ANY account publish a route naming ANY node as its
    /// publisher (the account vouches for the node; the module never compares
    /// them). So a label alone cannot decide which loopback port a request
    /// reaches: a member who republishes a label this operator bound, under
    /// their own account with an audience of their choosing, would otherwise
    /// resolve the port bound for someone else. The bind is where the node
    /// operator consents to an (account, label) pair, and
    /// `gateway_plane::loopback_port` refuses every record whose account is
    /// not the one recorded here.
    pub account: u64,
    pub name: gateway::RouteName,
    pub port: u16,
}

/// The one serve-time and file-order key: an (account, label) pair.
fn key(route: &LocalRoute) -> (&gateway::RouteName, u64) {
    (&route.name, route.account)
}

/// `deny_unknown_fields` is the schema guard: a file this build does not
/// understand is refused outright (no version field, no migrations — the
/// remedy is re-binding the routes).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalRoutes {
    pub routes: Vec<LocalRoute>,
}

impl LocalRoutes {
    fn validate(&self) -> Result<(), String> {
        if self.routes.len() > gateway::MAX_ROUTES_PER_ACCOUNT {
            return Err(format!(
                "gateway routes: at most {} local routes",
                gateway::MAX_ROUTES_PER_ACCOUNT
            ));
        }
        let mut previous: Option<(&gateway::RouteName, u64)> = None;
        for route in &self.routes {
            route.name.validate()?;
            if route.port == 0 {
                return Err("gateway routes: loopback port must be non-zero".into());
            }
            if previous.is_some_and(|old| old >= key(route)) {
                return Err(
                    "gateway routes: (name, account) pairs must be unique and sorted".into(),
                );
            }
            previous = Some(key(route));
        }
        Ok(())
    }

    /// The port bound for THIS account's label, and nothing else — see
    /// [`LocalRoute::account`].
    pub fn port(&self, account: u64, name: &gateway::RouteName) -> Option<u16> {
        self.routes
            .binary_search_by(|route| key(route).cmp(&(name, account)))
            .ok()
            .map(|index| self.routes[index].port)
    }

    /// Is `name` bound here for some OTHER account? The one thing that
    /// separates "nobody bound this label" from a record trying to ride a bind
    /// the operator made for someone else.
    pub fn bound_for_another_account(&self, account: u64, name: &gateway::RouteName) -> bool {
        self.routes
            .iter()
            .any(|route| &route.name == name && route.account != account)
    }

    /// Insert or replace `(account, name) -> port`, keeping the sorted-unique
    /// invariant [`Self::validate`] enforces.
    fn upsert(&mut self, account: u64, name: gateway::RouteName, port: u16) {
        match self
            .routes
            .binary_search_by(|route| key(route).cmp(&(&name, account)))
        {
            Ok(index) => self.routes[index].port = port,
            Err(index) => self.routes.insert(
                index,
                LocalRoute {
                    account,
                    name,
                    port,
                },
            ),
        }
    }

    /// Drop `(account, name)` if it is present; absent is not an error.
    fn drop_route(&mut self, account: u64, name: &gateway::RouteName) {
        let Ok(index) = self
            .routes
            .binary_search_by(|route| key(route).cmp(&(name, account)))
        else {
            return;
        };
        self.routes.remove(index);
    }

    /// Who holds `name` in THIS snapshot, from the point of view of the daemon
    /// serving `port`. Takes a loaded snapshot rather than a workspace on
    /// purpose: an ownership check that re-reads the file before acting on its
    /// own answer is decoration (see [`retire`]).
    fn owner(&self, account: u64, name: &gateway::RouteName, port: u16) -> RouteOwner {
        let Some(registered) = self.port(account, name) else {
            return RouteOwner::Vacant;
        };
        let is_ours = registered == port;
        if is_ours {
            return RouteOwner::Ours;
        }
        RouteOwner::Foreign
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
    let routes: LocalRoutes = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode {path:?}: {error}\n  {REMEDY}"))?;
    routes.validate()?;
    let canonical = serde_json::to_vec_pretty(&routes).expect("routes serialize");
    if canonical != bytes {
        return Err(format!("{path:?} is not canonical\n  {REMEDY}"));
    }
    Ok(routes)
}

/// Every way this file is refused has ONE remedy, and it is cheap: the file is
/// a cache of ports the daemons themselves chose, so deleting it loses nothing
/// a heartbeat does not put straight back.
///
/// Worth spelling out because the refusal reads like data loss when it is not.
/// A stale field name in here took down a whole `make dev` — the message named
/// the offending field, and nothing else — and the remedy people reached for
/// was hand-editing JSON under `~/.ducktape`.
const REMEDY: &str = "this file only caches which local port each daemon chose: \
                      delete it and restart the node, and each re-registers its own route";

fn temporary_path(workspace: &Path) -> PathBuf {
    workspace.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()))
}

fn save(workspace: &Path, routes: &LocalRoutes) -> Result<(), String> {
    routes.validate()?;
    std::fs::create_dir_all(workspace).map_err(|error| format!("create {workspace:?}: {error}"))?;
    let path = workspace.join(FILE_NAME);
    // Per-process temp name. This file is read-modify-written on a service
    // daemon's heartbeat, so a FIXED name lets that beat and an operator's
    // `gateway bind` interleave until one rename publishes the other's bytes.
    // The leftovers a crash leaves behind are reaped by
    // [`sweep_stale_temporaries`], which is what the fixed name gave for free.
    //
    // ponytail: the read-modify-write itself is still last-writer-wins across
    // processes — a lost update, not a torn file. Take a lock only if a second
    // route-writing daemon ever appears.
    let temporary = temporary_path(workspace);
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
    /// serve this label for that account's signed route (default: the account
    /// the active wallet is on). A node may host routes for accounts other
    /// than its operator's — the bind is where it says WHICH.
    #[arg(long, value_name = "ACCOUNT")]
    account: Option<u64>,
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
    let account = match args.account {
        Some(account) => account,
        None => consenting_account(&workspace)?,
    };
    // the verb IS `register` plus a printed key — its own copy of the
    // load/upsert/save was a duplicate waiting to diverge from the daemon path.
    register(&workspace, account, name.clone(), args.port.get())?;
    println!("{}", name.local_key());
    Ok(())
}

/// WHOSE routes this node agrees to serve on this workspace: the account the
/// operator's ACTIVE WALLET key is on, read from committed identity state over
/// the node's own loopback `/v1`. A bind is that operator's consent to one
/// (account, label) pair — see [`LocalRoute::account`] — so the account has to
/// come from the operator, never from the request.
fn consenting_account(workspace: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let key = crate::boot::surfaces::operator_wallet_key()
        .ok_or("no active wallet on this host — `ducktape wallet create` first")?;
    let base = config::http_base_in(workspace)?;
    Ok(crate::account_cli::own_account(&base, &key)?.number)
}

/// Register or update a node-local loopback route `(account, name) -> port` at
/// boot — the programmatic equivalent of the `gateway bind` CLI, for services
/// the node runs itself (e.g. the airlock lender on an ephemeral port).
pub fn register(
    workspace: &Path,
    account: u64,
    name: gateway::RouteName,
    port: u16,
) -> Result<(), String> {
    if port == 0 {
        return Err("gateway route port must be non-zero".into());
    }
    name.validate()?;
    let mut routes = load(workspace)?;
    routes.upsert(account, name, port);
    save(workspace, &routes)
}

/// Reap `save`'s leftovers: a writer killed between its `write` and its `rename`
/// leaves its temp behind, and a per-process temp name means nothing overwrites
/// it (the old fixed name self-healed on the next write — that is the one thing
/// it was good for). One `read_dir` where a daemon starts restores that, at a
/// cadence no hot path pays for.
///
/// ponytail: a temp belonging to a LIVE writer mid-`save` is removed too, whose
/// rename then fails loudly and whose caller retries — a failed command, never a
/// corrupt file. Checking liveness would need `/proc`, which is not portable to
/// the macOS boxes this runs on.
pub fn sweep_stale_temporaries(workspace: &Path) {
    let Ok(entries) = std::fs::read_dir(workspace) else {
        return;
    };
    let ours = temporary_path(workspace);
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let is_a_temp = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with(&format!(".{FILE_NAME}.")) && name.ends_with(".tmp")
            });
        if !is_a_temp || path == ours {
            continue;
        }
        let _ = std::fs::remove_file(&path);
    }
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
    account: u64,
    name: &gateway::RouteName,
    port: u16,
) -> Result<RouteOwner, String> {
    // ONE load, and the write comes out of that same snapshot — see [`retire`].
    let mut routes = load(workspace)?;
    let owner = routes.owner(account, name, port);
    match owner {
        RouteOwner::Vacant => {
            routes.upsert(account, name.clone(), port);
            save(workspace, &routes)?;
        }
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
///
/// The ownership check and the removal act on ONE loaded snapshot. Re-reading
/// the file to perform the delete would make the check decoration: a second
/// daemon registering between the two reads meant we deleted ITS live entry by
/// name, which is exactly the failure [`RouteOwner`] exists to close.
///
/// ponytail: this narrows the window to the load->save any writer of this file
/// already has, and does NOT eliminate it — two writers can still lose one
/// update, because a plain read-modify-write cannot be made atomic without a
/// lock. Take an advisory lock here if a second route-writing daemon ever
/// appears; today the only concurrent writer is an operator typing
/// `ducktape gateway bind`.
pub fn retire(
    workspace: &Path,
    account: u64,
    name: &gateway::RouteName,
    port: u16,
) -> Result<RouteOwner, String> {
    let mut routes = load(workspace)?;
    let owner = routes.owner(account, name, port);
    match owner {
        RouteOwner::Vacant | RouteOwner::Foreign => {}
        RouteOwner::Ours => {
            routes.drop_route(account, name);
            save(workspace, &routes)?;
        }
    }
    Ok(owner)
}

fn unbind(args: RouteArgs) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = args.workspace.dir()?;
    let name = args.name()?;
    let mut routes = load(&workspace)?;
    // Withdrawing consent is per LABEL, not per account: the operator is saying
    // "this node stops serving that label", and every account it was bound for
    // goes with it. Deliberately not the bind's twin — unbind must never need a
    // running node to answer whose account the active wallet is on.
    let before = routes.routes.len();
    routes.routes.retain(|route| route.name != name);
    if routes.routes.len() == before {
        return Err(format!("gateway route {:?} does not exist", name.label).into());
    }
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

    /// the account every fixture binds for — the operator's own.
    const ACCOUNT: u64 = 7;

    fn route(dir: &Path, label: Option<&str>, network: Option<&str>) -> RouteArgs {
        RouteArgs {
            label: label.map(String::from),
            workspace: WorkspaceArgs {
                workspace: Some(dir.to_path_buf()),
                network: network.map(String::from),
            },
        }
    }

    #[test]
    fn apex_and_named_routes_are_canonical_and_leave_no_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        register(dir.path(), ACCOUNT, gateway::RouteName::apex(), 3000).unwrap();
        register(dir.path(), ACCOUNT, gateway::RouteName::named("api"), 4000).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            loaded.port(ACCOUNT, &gateway::RouteName::apex()),
            Some(3000)
        );
        assert_eq!(
            loaded.port(ACCOUNT, &gateway::RouteName::named("api")),
            Some(4000)
        );

        unbind(route(dir.path(), None, None)).unwrap();
        unbind(route(dir.path(), Some("api"), None)).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(
            register(
                dir.path(),
                ACCOUNT,
                gateway::RouteName::named("evil.name"),
                1
            )
            .is_err()
        );
    }

    /// Withdrawing consent is per LABEL: `unbind` takes every account's entry
    /// for it, and needs no node to say whose the operator's own is.
    #[test]
    fn unbind_withdraws_a_label_for_every_account_it_was_bound_for() {
        let dir = tempfile::tempdir().unwrap();
        let name = gateway::RouteName::named("api");
        register(dir.path(), ACCOUNT, name.clone(), 4000).unwrap();
        register(dir.path(), ACCOUNT + 1, name.clone(), 4001).unwrap();
        unbind(route(dir.path(), Some("api"), None)).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(unbind(route(dir.path(), Some("api"), None)).is_err());
    }

    /// A restarted daemon comes back on a FRESH ephemeral port, so registering
    /// the same route name twice must replace the entry rather than add one —
    /// the file refuses duplicate names outright.
    #[test]
    fn registering_a_route_name_twice_replaces_the_entry_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let name = gateway::RouteName::named("airlock");
        register(dir.path(), ACCOUNT, name.clone(), 4100).unwrap();
        register(dir.path(), ACCOUNT, name.clone(), 4100).unwrap();
        register(dir.path(), ACCOUNT, name.clone(), 4200).unwrap();
        assert_eq!(load(dir.path()).unwrap().routes.len(), 1);
        assert_eq!(load(dir.path()).unwrap().port(ACCOUNT, &name), Some(4200));
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
        register(dir.path(), ACCOUNT, name.clone(), 4100).unwrap();
        assert_eq!(
            reassert(dir.path(), ACCOUNT, &name, 4100).unwrap(),
            RouteOwner::Ours
        );
        assert_eq!(load(dir.path()).unwrap().port(ACCOUNT, &name), Some(4100));

        // a second daemon starts and takes it. The first must now yield.
        register(dir.path(), ACCOUNT, name.clone(), 4200).unwrap();
        assert_eq!(
            reassert(dir.path(), ACCOUNT, &name, 4100).unwrap(),
            RouteOwner::Foreign
        );
        assert_eq!(
            load(dir.path()).unwrap().port(ACCOUNT, &name),
            Some(4200),
            "a yielded beat writes nothing — otherwise the two flap forever"
        );

        // ...and the first daemon's SIGTERM must not delete the survivor's entry.
        assert_eq!(
            retire(dir.path(), ACCOUNT, &name, 4100).unwrap(),
            RouteOwner::Foreign
        );
        assert_eq!(load(dir.path()).unwrap().port(ACCOUNT, &name), Some(4200));

        // the owner retires its own: gone, no husk file, and safe twice (the
        // operator may have unbound it by hand first).
        assert_eq!(
            retire(dir.path(), ACCOUNT, &name, 4200).unwrap(),
            RouteOwner::Ours
        );
        assert_eq!(load(dir.path()).unwrap().port(ACCOUNT, &name), None);
        assert!(!dir.path().join(FILE_NAME).exists());
        assert_eq!(
            retire(dir.path(), ACCOUNT, &name, 4200).unwrap(),
            RouteOwner::Vacant
        );

        // a hand `gateway unbind` is corrected on the next beat.
        assert_eq!(
            reassert(dir.path(), ACCOUNT, &name, 4200).unwrap(),
            RouteOwner::Vacant
        );
        assert_eq!(load(dir.path()).unwrap().port(ACCOUNT, &name), Some(4200));
    }

    /// The ownership check must act on the SAME bytes it read. `retire` used to
    /// re-load and delete by NAME, so a second daemon registering between the
    /// two reads had its live entry deleted anyway — the check narrowed the
    /// failure to a microsecond instead of closing it. This pins the property at
    /// the only layer that can hold it: given a snapshot, ownership and the edit
    /// it authorizes come from that one snapshot.
    #[test]
    fn ownership_and_the_edit_it_authorizes_read_one_snapshot() {
        let name = gateway::RouteName::named("airlock");
        let mut routes = LocalRoutes::default();
        routes.upsert(ACCOUNT, name.clone(), 4100);

        assert_eq!(routes.owner(ACCOUNT, &name, 4100), RouteOwner::Ours);
        assert_eq!(routes.owner(ACCOUNT, &name, 4200), RouteOwner::Foreign);
        assert_eq!(
            routes.owner(ACCOUNT, &gateway::RouteName::apex(), 4100),
            RouteOwner::Vacant
        );

        // the survivor's entry is what a foreign retire must not touch, and the
        // snapshot is the only thing that can say whose it is.
        routes.upsert(ACCOUNT, name.clone(), 4200);
        assert_eq!(routes.owner(ACCOUNT, &name, 4100), RouteOwner::Foreign);
        routes.drop_route(ACCOUNT, &name);
        assert_eq!(routes.owner(ACCOUNT, &name, 4200), RouteOwner::Vacant);
        routes
            .validate()
            .expect("upsert/drop keep the sorted-unique invariant");
    }

    /// The TOCTOU is invisible to a value test — reading the file twice is only
    /// wrong under a concurrent writer, and there is no seam to inject one. The
    /// SHAPE is what holds it, so the shape is what gets guarded.
    #[test]
    fn a_port_scoped_write_loads_the_route_file_exactly_once() {
        let source = include_str!("gateway_routes.rs");
        for signature in ["pub fn reassert(", "pub fn retire("] {
            let body = source
                .split_once(signature)
                .expect("the function exists")
                .1
                .split_once("\n}\n")
                .expect("the function ends")
                .0;
            assert_eq!(
                body.matches("load(workspace)").count(),
                1,
                "{signature} must read the file ONCE — a second read is the TOCTOU \
                 RouteOwner exists to close: the entry it deletes may no longer be the \
                 one it checked"
            );
        }
    }

    /// A writer killed between its `write` and its `rename` leaves a temp that
    /// nothing overwrites, because the name carries its pid. The fixed name
    /// self-healed; this restores that without giving back the clobber it cost.
    #[test]
    fn a_killed_writers_temp_is_reaped_and_the_route_file_is_not() {
        let dir = tempfile::tempdir().unwrap();
        register(
            dir.path(),
            ACCOUNT,
            gateway::RouteName::named("airlock"),
            4100,
        )
        .unwrap();
        let orphan = dir.path().join(format!(".{FILE_NAME}.999999.tmp"));
        std::fs::write(&orphan, b"a dead writer's leftovers").unwrap();
        // our own in-flight temp must survive a sweep we ourselves run.
        let ours = temporary_path(dir.path());
        std::fs::write(&ours, b"in flight").unwrap();

        sweep_stale_temporaries(dir.path());

        assert!(
            !orphan.exists(),
            "another process's leftover temp is reaped"
        );
        assert!(ours.exists(), "our own in-flight temp is not");
        assert_eq!(
            load(dir.path())
                .unwrap()
                .port(ACCOUNT, &gateway::RouteName::named("airlock")),
            Some(4100),
            "the sweep must never touch the route file itself"
        );
        std::fs::remove_file(&ours).unwrap();
    }

    /// A file this build does not understand is refused — and the refusal has
    /// to carry its own remedy, because it reads like data loss and is not. A
    /// stale `version` field written by an old tool took a whole `make dev`
    /// down with `unknown field 'version'` and nothing else, and the reflex it
    /// produced was hand-editing JSON under `~/.ducktape`.
    #[test]
    fn a_file_this_build_cannot_read_is_refused_with_its_remedy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);

        for bytes in [
            // a field this schema does not have (deny_unknown_fields)
            br#"{"version": 1, "routes": []}"#.as_slice(),
            // valid, decodable, but not the bytes `save` would have written
            br#"{"routes":[]}"#.as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let refusal = load(dir.path()).unwrap_err();
            assert!(
                refusal.contains(REMEDY),
                "every refusal names the remedy: {refusal}"
            );
        }
    }

    #[test]
    fn explicit_workspace_wins_over_network() {
        // --workspace short-circuits before the registry, so a bogus -n never
        // resolves: every verb edits the explicit dir.
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            route(dir.path(), None, Some("no-such-workspace"))
                .workspace
                .dir()
                .unwrap(),
            dir.path()
        );
    }
}
