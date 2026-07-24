//! `ducktape user cred` — named, grantable, owner-hosted API credentials.
//!
//! `cred add <provider>` wraps the vendor's OWN login CLI (`claude setup-token`,
//! `codex login`) on a local pty, captures the login artifact directly into this
//! node's disk-backed gateway store (`<storage>/airlock-creds/<name>/`), and
//! registers the record on-chain (name → owner account, publisher node, kind,
//! seal_pk) so any granted node's broker can resolve the name and complete a
//! round-trip through the owner's co-hosted gateway WITHOUT ever holding the
//! secret. `grant`/`revoke` lend/rescind by account; `list` reads committed
//! records; `remove` tombstones one.
//!
//! Every verb runs CO-HOSTED with the node whose credential it manages: it reads
//! the node's own workspace (chain id, consensus key, storage) to build the
//! owner-signed statement, then submits it over the node's frameless
//! `/v1/submit` (which stamps the node as the op origin — the record's
//! `publisher_node`). Program output stays `println!` (a CLI's stdout is not
//! logging).

use std::io::BufRead;
use std::path::Path;

use commonware_cryptography::Signer as _;

use crate::config;
use crate::userkey_cli::{load_user_signer, redeem_node};

type CredResult = Result<(), Box<dyn std::error::Error>>;

/// `ducktape user cred <verb>` — the credential subfamily. `--node`/`-n` are the
/// same node-resolution pair `redeem-invite` carries, made `global` so they
/// attach in any position (`cred add claude -n net` reads naturally).
#[derive(Debug, clap::Args)]
pub(crate) struct CredArgs {
    #[command(subcommand)]
    cmd: CredCmd,
    /// the co-hosted node's http base (e.g. `http://host:port`)
    #[arg(long, value_name = "URL", global = true)]
    node: Option<String>,
    /// resolve the co-hosted node through a registered workspace's chain id
    #[arg(short = 'n', long = "network", value_name = "CHAIN-ID", global = true)]
    network: Option<String>,
    /// path to the user key file (defaults to the network workspace's `user.key`)
    #[arg(long, value_name = "PATH", global = true)]
    key: Option<std::path::PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum CredCmd {
    /// wrap the vendor login CLI, store the artifact, register the record
    Add {
        /// which vendor credential to capture
        provider: ProviderArg,
        /// the credential name (default `<display>-<provider>-<n>`)
        name: Option<String>,
    },
    /// list every registered credential record
    List {
        /// print the records as JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// tombstone one owned credential record
    Remove {
        /// the credential name
        name: String,
    },
    /// lend a credential to another account (owner-signed)
    Grant {
        /// the credential name
        name: String,
        /// the grantee: a hex account id or a display name
        account: String,
    },
    /// rescind a lend (owner-signed)
    Revoke {
        /// the credential name
        name: String,
        /// the grantee: a hex account id or a display name
        account: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ProviderArg {
    Claude,
    Codex,
}

impl ProviderArg {
    /// the on-chain kind this provider registers as.
    fn kind(self) -> gateway::CredentialKind {
        match self {
            Self::Claude => gateway::CredentialKind::Claude,
            Self::Codex => gateway::CredentialKind::Codex,
        }
    }

    /// the lowercase token used in `kind` files and default names.
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// the vendor login binary (also the which-preflight target).
    fn binary(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// the login subcommand argv the pty runs.
    fn login_args(self) -> &'static [&'static str] {
        match self {
            // `auth login`, NOT `setup-token`. setup-token mints an
            // inference-only token and PRINTS it to the tty — it writes no
            // `.credentials.json`, so the artifact watch never fires and
            // `cred add` hangs forever after the login completes. `auth login`
            // runs the full-scope OAuth and writes `$CLAUDE_CONFIG_DIR/.credentials.json`
            // (`claudeAiOauth` accessToken/refreshToken/expiresAt) — the exact
            // artifact the airlock reads and refreshes.
            Self::Claude => &["auth", "login"],
            // `--device-auth`: a device-code flow (enter a code on any browser)
            // instead of the default localhost:1455 redirect, which needs a
            // browser on THIS host — wrong for a headless / SSH operator node.
            Self::Codex => &["login", "--device-auth"],
        }
    }

    /// the env var pointing the vendor CLI at the per-credential store dir.
    fn config_env(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Codex => "CODEX_HOME",
        }
    }

    /// the login artifact filename the vendor CLI writes into the store dir.
    fn artifact(self) -> &'static str {
        match self {
            Self::Claude => ".credentials.json",
            Self::Codex => "auth.json",
        }
    }

    /// the install hint printed when the binary is missing.
    fn install_hint(self) -> &'static str {
        match self {
            Self::Claude => "npm install -g @anthropic-ai/claude-code",
            Self::Codex => "npm install -g @openai/codex",
        }
    }
}

/// Dispatch one `cred` verb. `stdin` is threaded to [`load_user_signer`], which
/// reads the key password from it only when the key file is encrypted.
pub(crate) fn run(args: CredArgs, stdin: &mut impl BufRead) -> CredResult {
    let CredArgs {
        cmd,
        node,
        network,
        key,
    } = args;
    let ctx = VerbCtx { node, network, key };
    match cmd {
        CredCmd::Add { provider, name } => cmd_add(&ctx, provider, name, stdin),
        CredCmd::List { json } => cmd_list(&ctx, json),
        CredCmd::Remove { name } => cmd_remove(&ctx, name, stdin),
        CredCmd::Grant { name, account } => cmd_grant(&ctx, name, account, stdin),
        CredCmd::Revoke { name, account } => cmd_revoke(&ctx, name, account, stdin),
    }
}

/// The shared node/key context every verb resolves against.
struct VerbCtx {
    node: Option<String>,
    network: Option<String>,
    key: Option<std::path::PathBuf>,
}

impl VerbCtx {
    /// the node's http base (explicit `--node` wins, else the workspace's).
    fn http_base(&self) -> Result<String, Box<dyn std::error::Error>> {
        redeem_node(self.node.as_deref(), self.network.as_deref())
    }

    /// the user key path for the signing verbs: explicit `--key` wins, else the
    /// canonical `<workspace>/user.key` that `account-init` mints. A MISSING key
    /// is a loud error, never a cue to mint: cred always signs as an
    /// already-bound account, so an absent key means the wrong `-n`, or
    /// `account-init` was never run — silently minting a fresh unbound identity
    /// here is exactly the footgun that let a stray path clobber a real key.
    fn key_path(&self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let path = match &self.key {
            Some(explicit) => explicit.clone(),
            None => {
                let needle = self
                    .network
                    .as_deref()
                    .filter(|n| !n.is_empty())
                    .ok_or("cred needs -n/--network (or an explicit --key) to locate the user key")?;
                let (dir, _http) = config::resolve_network(needle)?;
                dir.join("user.key")
            }
        };
        if !path.exists() {
            return Err(format!(
                "no user key at {} — run `ducktape user account-init` first (or pass --key)",
                path.display()
            )
            .into());
        }
        Ok(path)
    }

    /// the co-hosted workspace resolved from `-n/--network`: chain id, the
    /// node's consensus key, and its storage dir. Required by every verb that
    /// mints an owner-signed statement or writes the store — `--node` alone
    /// cannot locate the on-disk workspace.
    fn workspace(&self) -> Result<config::Resolved, Box<dyn std::error::Error>> {
        let needle = self
            .network
            .as_deref()
            .filter(|n| !n.is_empty())
            .ok_or("cred needs -n/--network to locate the co-hosted node's workspace")?;
        let (dir, _http) = config::resolve_network(needle)?;
        Ok(config::resolve(&dir.join("node.toml"))?)
    }
}

// ============================================================================
// list
// ============================================================================

fn cmd_list(ctx: &VerbCtx, json: bool) -> CredResult {
    let base = ctx.http_base()?;
    let records = query_credentials(&base)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }
    if records.is_empty() {
        println!("no credentials registered");
        return Ok(());
    }
    println!("{:<24} {:<8} {:<20} grants", "name", "kind", "owner");
    for record in &records {
        let owner = config::hex_bytes(&record.owner_account);
        let owner_short = owner.get(..16).unwrap_or(&owner);
        let kind = match record.kind {
            gateway::CredentialKind::Claude => "claude",
            gateway::CredentialKind::Codex => "codex",
        };
        println!(
            "{:<24} {:<8} {:<20} {}",
            record.name,
            kind,
            owner_short,
            record.grants.len()
        );
    }
    Ok(())
}

// ============================================================================
// grant / revoke / remove — owner-signed statement, frameless submit
// ============================================================================

fn cmd_grant(ctx: &VerbCtx, name: String, account: String, stdin: &mut impl BufRead) -> CredResult {
    let base = ctx.http_base()?;
    let resolved = ctx.workspace()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let owner_account = query_owner_account(&base, user.public_key().as_ref())?;
    let grantee = resolve_account(&base, &account)?;
    let statement = gateway::CredentialGrantStatement {
        chain_id: resolved.chain_id.clone(),
        owner_account,
        name,
        account: grantee,
    };
    let preimage = gateway::grant_credential_preimage(&statement)?;
    let message = gateway::GatewayMsg::GrantCredential {
        statement,
        authorization: authorize(&user, &preimage),
    };
    let height = submit_gateway(&base, &message)?;
    println!("granted at height {height}");
    Ok(())
}

fn cmd_revoke(
    ctx: &VerbCtx,
    name: String,
    account: String,
    stdin: &mut impl BufRead,
) -> CredResult {
    let base = ctx.http_base()?;
    let resolved = ctx.workspace()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let owner_account = query_owner_account(&base, user.public_key().as_ref())?;
    let grantee = resolve_account(&base, &account)?;
    let statement = gateway::CredentialGrantStatement {
        chain_id: resolved.chain_id.clone(),
        owner_account,
        name,
        account: grantee,
    };
    let preimage = gateway::revoke_credential_preimage(&statement)?;
    let message = gateway::GatewayMsg::RevokeCredential {
        statement,
        authorization: authorize(&user, &preimage),
    };
    let height = submit_gateway(&base, &message)?;
    println!("revoked at height {height}");
    Ok(())
}

fn cmd_remove(ctx: &VerbCtx, name: String, stdin: &mut impl BufRead) -> CredResult {
    let base = ctx.http_base()?;
    let resolved = ctx.workspace()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let owner_account = query_owner_account(&base, user.public_key().as_ref())?;
    let statement = gateway::RemoveCredentialStatement {
        chain_id: resolved.chain_id.clone(),
        owner_account,
        name,
    };
    let preimage = gateway::remove_credential_preimage(&statement)?;
    let message = gateway::GatewayMsg::RemoveCredential {
        statement,
        authorization: authorize(&user, &preimage),
    };
    let height = submit_gateway(&base, &message)?;
    println!("removed at height {height}");
    Ok(())
}

// ============================================================================
// add — preflight, pty login wrap, artifact capture, register
// ============================================================================

fn cmd_add(
    ctx: &VerbCtx,
    provider: ProviderArg,
    name: Option<String>,
    stdin: &mut impl BufRead,
) -> CredResult {
    preflight_binary(provider)?;

    let base = ctx.http_base()?;
    let resolved = ctx.workspace()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let user_pub = user.public_key().as_ref().to_vec();

    // owner account + display name (for the default name), existing names (for
    // the counter) — one identity query, one gateway query.
    let account = query_owner_account_view(&base, &user_pub)?;
    let display = account
        .display_name
        .clone()
        .ok_or("this account has no display name — pass an explicit credential name")?;
    let existing = query_credentials(&base)?;
    let existing_names: Vec<&str> = existing.iter().map(|r| r.name.as_str()).collect();
    let name = match name {
        Some(name) => name,
        None => derive_default_name(&display, provider, &existing_names),
    };
    gateway::validate_credential_name(&name)?;

    // capture the login artifact into the on-disk store, keyed by name.
    let store = crate::airlock_serve::cred_store_root(&resolved.storage_dir);
    let dir = store.join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    // `DUCKTAPE_CRED_REUSE_ARTIFACT=<path>` imports an ALREADY-authenticated
    // vendor login artifact (a `.credentials.json` / `auth.json` the operator
    // already produced) instead of driving the vendor's browser OAuth flow —
    // for headless hosts and re-registration without another auth round-trip.
    // Everything downstream (artifact check, kind, seal, record, sign, submit)
    // is identical to the browser path; the browser flow remains the default.
    match std::env::var("DUCKTAPE_CRED_REUSE_ARTIFACT") {
        Ok(src) if !src.is_empty() => {
            std::fs::copy(&src, dir.join(provider.artifact()))
                .map_err(|e| format!("reuse artifact {src}: {e}"))?;
        }
        _ => {
            // Start clean so the login-completion watch (the artifact appearing)
            // is unambiguous — a stale file from a prior attempt would otherwise
            // read as instant success.
            let _ = std::fs::remove_file(dir.join(provider.artifact()));
            run_vendor_login(provider, &dir)?
        }
    }

    let artifact = dir.join(provider.artifact());
    if !artifact.exists() {
        return Err(format!(
            "{} did not write {} — login did not complete; nothing registered",
            provider.binary(),
            provider.artifact()
        )
        .into());
    }
    std::fs::write(dir.join("kind"), format!("{}\n", provider.token()))
        .map_err(|e| format!("write kind marker: {e}"))?;

    // the seal PUBLIC key the node co-hosts under (minted on first add, then
    // stable) is what the record pins for the compute broker.
    let seal = crate::airlock_serve::load_or_create_seal_keypair(&store)?;
    let record = gateway::CredentialRecord {
        name: name.clone(),
        owner_account: account.account_id.clone(),
        publisher_node: resolved.signer.public_key().as_ref().to_vec(),
        kind: provider.kind(),
        seal_pk: seal.public_bytes(),
        grants: std::collections::BTreeSet::new(),
    };
    let statement = gateway::SetCredentialStatement {
        chain_id: resolved.chain_id.clone(),
        record,
    };
    let preimage = gateway::set_credential_preimage(&statement)?;
    let message = gateway::GatewayMsg::SetCredential {
        statement,
        authorization: authorize(&user, &preimage),
    };
    let height = submit_gateway(&base, &message)?;
    println!("registered {name} at height {height}");
    // A lent credential is only reachable once the co-hosted airlock gateway has
    // a signed on-chain route. That route is per-ACCOUNT (one `airlock` route
    // serves every credential this account co-hosts), so publish it once and
    // skip on later `cred add`s — the operator never hand-signs a RouteStatement.
    ensure_airlock_route(&base, &user, &resolved, &account.account_id)?;
    Ok(())
}

/// The `airlock` gateway route label the co-hosted gateway registers itself
/// under; the compute broker resolves `<AIRLOCK_ROUTE>.<handle>.duck` to it.
/// Mirrors `crate::cred_resolve::AIRLOCK_ROUTE`.
const AIRLOCK_ROUTE: &str = "airlock";

/// Publish the account's `airlock` gateway route if it is not already published
/// — the one signed statement that makes this node's co-hosted gateway reachable
/// over the overlay. Idempotent: a route already present is left untouched.
fn ensure_airlock_route(
    base: &str,
    user: &commonware_cryptography::ed25519::PrivateKey,
    resolved: &config::Resolved,
    account_id: &[u8],
) -> CredResult {
    let name = gateway::RouteName::named(AIRLOCK_ROUTE);
    let existing = query_gateway(
        base,
        &gateway::GatewayQuery::Get { account_id: account_id.to_vec(), name: name.clone() },
    )?;
    let already_published = matches!(existing, gateway::GatewayReply::Route(ref boxed) if boxed.is_some());
    if already_published {
        return Ok(());
    }
    // The airlock upstream is a streaming (SSE) loopback: unbounded response
    // (`max_response_bytes = 0`), GET+POST, and it forwards the scoped session
    // bearer (`allow_authorization`). Request cap is the module ceiling.
    let statement = gateway::RouteStatement {
        version: 1,
        chain_id: resolved.chain_id.clone(),
        account_id: account_id.to_vec(),
        name,
        publisher_node: resolved.signer.public_key().as_ref().to_vec(),
        revision: 1,
        route: Some(gateway::RouteDefinition {
            target: gateway::RouteTarget::LoopbackHttp,
            policy: gateway::RoutePolicy {
                audience: gateway::RouteAudience::Network,
                methods: vec![gateway::RouteMethod::Get, gateway::RouteMethod::Post],
                max_request_bytes: gateway::MAX_REQUEST_BODY_BYTES,
                max_response_bytes: 0,
                allow_authorization: true,
                allow_upgrade: false,
            },
        }),
    };
    let preimage = gateway::route_signing_preimage(&statement)?;
    let message = gateway::GatewayMsg::SetRoute {
        statement,
        authorization: gateway::MemberAuthorization {
            signer: user.public_key().as_ref().to_vec(),
            signature: user.sign(gateway::GATEWAY_ROUTE_NS, &preimage).as_ref().to_vec(),
        },
    };
    let height = submit_gateway(base, &message)?;
    println!("published airlock route at height {height}");
    Ok(())
}

/// Refuse early (nonzero exit) when the vendor binary is absent, naming the
/// install command — running the pty against a missing binary would only fail
/// opaquely.
fn preflight_binary(provider: ProviderArg) -> CredResult {
    let ok = std::process::Command::new(provider.binary())
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return Err(format!(
            "install {} first ({})",
            provider.binary(),
            provider.install_hint()
        )
        .into());
    }
    Ok(())
}

/// Run the vendor login on a local pty, pointing its config home at `dir`. The
/// wrap is a TRANSPARENT terminal: this terminal goes raw, the vendor CLI's own
/// full-screen TUI is mirrored byte-for-byte, and keystrokes are forwarded
/// verbatim — so its interactive authorize-and-paste flow renders and behaves
/// exactly as if the vendor login were run directly.
fn run_vendor_login(provider: ProviderArg, dir: &Path) -> CredResult {
    let mut command = tokio::process::Command::new(provider.binary());
    command.args(provider.login_args());
    command.env(provider.config_env(), dir);
    let artifact = dir.join(provider.artifact());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("login runtime: {e}"))?;
    let result = rt.block_on(pump_login(command, &artifact));
    // `shutdown_background`, not the implicit drop: the stdin forwarder reads
    // `tokio::io::stdin()`, which parks a BLOCKING thread on `read(0)`. Aborting
    // the task can't interrupt that OS-level read, so once the login ends with no
    // further keypress the thread stays stuck — and a normal runtime drop WAITS
    // for its blocking pool, hanging `cred add` after the pty session is over.
    // Detach instead: the stuck reader dies with the process.
    rt.shutdown_background();
    result?;
    Ok(())
}

/// The Ctrl-C byte. Raw mode disables ISIG, so a Ctrl-C is delivered to us as
/// this byte rather than a SIGINT — we treat it as "cancel the login".
const CTRL_C: u8 = 0x03;

/// Pump the pty: child output → stdout (mirrored), this process's stdin → child.
/// Returns when the login artifact is written, the child exits, or the pty
/// closes; Ctrl-C cancels. `artifact` is the credential file the vendor login
/// writes — its appearance IS success.
async fn pump_login(command: tokio::process::Command, artifact: &Path) -> CredResult {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::AsyncReadExt as _;

    // Raw mode for the whole login: `claude setup-token` / `codex login` drive a
    // full-screen TUI, so this terminal must pass keystrokes through verbatim —
    // otherwise the pasted auth code is line-buffered, locally echoed, and its
    // Enter arrives as `\n` instead of the `\r` the child's prompt submits on, so
    // the code never registers. The guard restores the tty on return AND on panic.
    let _raw = crate::tty::RawGuard::enter();

    let session = Arc::new(capability_host::InteractiveSession::spawn_local(command)?);

    // Forward this terminal's stdin to the child until the session ends. A Ctrl-C
    // (byte, not SIGINT — ISIG is off in raw mode) CANCELS: without this it would
    // just flow to the child's TUI, which may ignore it, leaving no way out. It
    // closes the session (killing the child) so the pump unwinds, and flags the
    // cancel so `cred add` errors instead of registering an incomplete login.
    let cancelled = Arc::new(AtomicBool::new(false));
    let writer = session.clone();
    let cancel = cancelled.clone();
    let input = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            let n = match stdin.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if buf[..n].contains(&CTRL_C) {
                cancel.store(true, Ordering::SeqCst);
                writer.close().await;
                break;
            }
            if writer.write_all(&buf[..n]).await.is_err() {
                break;
            }
        }
    });

    // Mirror the vendor login's own full-screen TUI verbatim — it already
    // presents the authorize URL and prompts for the code interactively. We add
    // nothing to the stream (injecting a line mid-redraw would corrupt its
    // layout); raw mode above lets its UI render exactly as if run directly.
    let mirror = async {
        let mut buf = [0u8; 4096];
        loop {
            let n = session
                .read(&mut buf)
                .await
                .map_err(|e| format!("read login output: {e}"))?;
            if n == 0 {
                return CredResult::Ok(());
            }
            mirror_stdout(&buf[..n])?;
        }
    };

    // The vendor login has written the credential once this file is present and
    // its size has stopped growing — that IS success, and it is the signal we
    // trust: `claude setup-token` (a single process, no forked helper) prints
    // "created successfully" and then does NOT exit — it sits waiting on the tty
    // — so neither pty EOF nor child-exit ever fires and `cred add` would hang
    // forever before registering. Watching the artifact ends the login the moment
    // the token lands; `close` below kills the still-running child.
    let watch = async {
        let mut last = 0u64;
        loop {
            let size = std::fs::metadata(artifact).map(|m| m.len()).unwrap_or(0);
            if size > 0 && size == last {
                return;
            }
            last = size;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    };

    // End on whichever comes first: the credential written (`watch`), the child
    // exiting (some vendors do), or the pty closing (`mirror`). `close` then
    // terminates the process group, reaping any child still holding the tty.
    tokio::select! {
        result = mirror => result?,
        () = session.wait_child_exit() => {}
        () = watch => {}
    }
    input.abort();
    session.close().await;
    if cancelled.load(Ordering::SeqCst) {
        return Err("login cancelled (ctrl-c)".into());
    }
    Ok(())
}

/// Write raw child bytes straight through to this terminal.
fn mirror_stdout(bytes: &[u8]) -> CredResult {
    use std::io::Write as _;
    let mut out = std::io::stdout();
    out.write_all(bytes).map_err(|e| format!("stdout: {e}"))?;
    out.flush().map_err(|e| format!("stdout flush: {e}"))?;
    Ok(())
}

// ============================================================================
// pure helpers (unit-tested)
// ============================================================================

/// The default credential name `<display>-<provider>-<n>`, where `n` is one past
/// the highest existing counter for that display+provider prefix (1 when none).
fn derive_default_name(display: &str, provider: ProviderArg, existing: &[&str]) -> String {
    let prefix = format!("{display}-{}-", provider.token());
    let highest = existing
        .iter()
        .filter_map(|name| name.strip_prefix(&prefix)?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("{prefix}{}", highest + 1)
}

// ============================================================================
// node round-trips
// ============================================================================

/// Build the owner authorization over `preimage`, signed under the gateway
/// credential namespace — the exact primitive the gateway `SetRoute` family uses.
fn authorize(
    user: &commonware_cryptography::ed25519::PrivateKey,
    preimage: &[u8],
) -> gateway::MemberAuthorization {
    gateway::MemberAuthorization {
        signer: user.public_key().as_ref().to_vec(),
        signature: user
            .sign(gateway::GATEWAY_CREDENTIAL_NS, preimage)
            .as_ref()
            .to_vec(),
    }
}

/// Submit one gateway op over the node's frameless `/v1/submit` (the node stamps
/// itself as origin) and return the committed height.
fn submit_gateway(
    base: &str,
    message: &gateway::GatewayMsg,
) -> Result<u64, Box<dyn std::error::Error>> {
    crate::node_http::submit(base, "gateway", &serde_json::to_value(message)?)
}

/// Read every registered credential record from committed gateway state.
fn query_credentials(
    base: &str,
) -> Result<Vec<gateway::CredentialRecord>, Box<dyn std::error::Error>> {
    let reply = query_gateway(base, &gateway::GatewayQuery::Credentials {})?;
    match reply {
        gateway::GatewayReply::Credentials(records) => Ok(records),
        other => Err(format!("unexpected gateway reply: {other:?}").into()),
    }
}

fn query_gateway(
    base: &str,
    query: &gateway::GatewayQuery,
) -> Result<gateway::GatewayReply, Box<dyn std::error::Error>> {
    let value = query_node(base, "gateway", serde_json::to_value(query)?)?;
    Ok(serde_json::from_value(value)?)
}

/// The account id the local user key belongs to (owner of any record it signs).
fn query_owner_account(
    base: &str,
    member_key: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(query_owner_account_view(base, member_key)?.account_id)
}

fn query_owner_account_view(
    base: &str,
    member_key: &[u8],
) -> Result<identity::AccountView, Box<dyn std::error::Error>> {
    let query = identity::IdentityQuery::OfMember {
        member_key: member_key.to_vec(),
    };
    let value = query_node(base, "identity", serde_json::to_value(&query)?)?;
    match serde_json::from_value::<identity::IdentityReply>(value)? {
        identity::IdentityReply::Account(Some(account)) => Ok(account),
        identity::IdentityReply::Account(None) => {
            Err("this user key belongs to no account on this node — bind it first".into())
        }
        other => Err(format!("unexpected identity reply: {other:?}").into()),
    }
}

/// Resolve a grant target: a hex account id used directly, else a display name
/// matched against the account set (ambiguity and absence are loud errors).
fn resolve_account(base: &str, input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if let Ok(bytes) = config::unhex(input) {
        return Ok(bytes);
    }
    let query = identity::IdentityQuery::All {
        from: 0,
        limit: u64::MAX,
    };
    let value = query_node(base, "identity", serde_json::to_value(&query)?)?;
    let accounts = match serde_json::from_value::<identity::IdentityReply>(value)? {
        identity::IdentityReply::Accounts(accounts) => accounts,
        other => return Err(format!("unexpected identity reply: {other:?}").into()),
    };
    let matches: Vec<&identity::AccountView> = accounts
        .iter()
        .filter(|a| a.display_name.as_deref() == Some(input))
        .collect();
    match matches.as_slice() {
        [only] => Ok(only.account_id.clone()),
        [] => Err(format!("no account named {input:?} (nor a valid hex account id)").into()),
        many => {
            let ids: Vec<String> = many
                .iter()
                .map(|a| config::hex_bytes(&a.account_id))
                .collect();
            Err(format!(
                "account name {input:?} is ambiguous across {} accounts: {}",
                many.len(),
                ids.join(", ")
            )
            .into())
        }
    }
}

/// One `/v1/query` round-trip: `{target, query}` in, the module reply JSON out.
pub(crate) fn query_node(
    base: &str,
    target: &str,
    query: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    crate::node_http::query(base, target, query)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The login pump must return the moment the credential artifact lands, even
    /// when the vendor login process NEVER exits (the `claude setup-token` shape:
    /// it writes the token, prints success, then sits on the tty). A never-exiting
    /// `sleep` stands in; the artifact is written mid-flight from another task.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pump_login_returns_when_the_artifact_lands_even_if_the_child_never_exits() {
        let dir = std::env::temp_dir().join(format!("ducktape-cred-pump-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let artifact = dir.join("cred.json");
        let _ = std::fs::remove_file(&artifact);

        let mut cmd = tokio::process::Command::new("sleep");
        cmd.arg("300");

        let art = artifact.clone();
        let writer = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            std::fs::write(&art, b"{\"token\":\"x\"}").unwrap();
        });

        let outcome =
            tokio::time::timeout(std::time::Duration::from_secs(10), pump_login(cmd, &artifact))
                .await;

        writer.await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(outcome, Ok(Ok(()))),
            "pump_login should return Ok once the artifact lands; got {outcome:?}"
        );
    }

    #[test]
    fn default_name_is_display_provider_counter() {
        let existing = ["eddy-claude-1", "eddy-claude-2", "eddy-codex-1"];
        assert_eq!(
            derive_default_name("eddy", ProviderArg::Claude, &existing),
            "eddy-claude-3"
        );
        assert_eq!(
            derive_default_name("eddy", ProviderArg::Codex, &existing),
            "eddy-codex-2"
        );
        assert_eq!(
            derive_default_name("jess", ProviderArg::Claude, &[]),
            "jess-claude-1"
        );
    }

    /// Claude must log in with `auth login` (writes `.credentials.json`, which
    /// the artifact watch keys on and the airlock reads), NEVER `setup-token`
    /// (prints an inference-only token, writes no file → `cred add` hangs).
    #[test]
    fn claude_login_writes_the_credentials_artifact_not_setup_token() {
        assert_eq!(ProviderArg::Claude.login_args(), &["auth", "login"]);
        assert_eq!(ProviderArg::Claude.artifact(), ".credentials.json");
        assert_eq!(ProviderArg::Codex.login_args(), &["login", "--device-auth"]);
    }

    /// An explicit `--key` that EXISTS resolves to itself; one that is ABSENT
    /// is a loud error, not a silent fresh mint — the regression that let a
    /// stray path clobber a real, bound user key.
    #[test]
    fn key_path_uses_explicit_key_but_refuses_a_missing_one() {
        let dir = std::env::temp_dir().join(format!("ducktape-keypath-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let present = dir.join("user.key");
        std::fs::write(&present, b"deadbeef").unwrap();

        let ctx = VerbCtx { node: None, network: None, key: Some(present.clone()) };
        assert_eq!(ctx.key_path().unwrap(), present);

        let missing = dir.join("nope.key");
        let ctx = VerbCtx { node: None, network: None, key: Some(missing) };
        let err = ctx.key_path().unwrap_err().to_string();
        assert!(err.contains("no user key at"), "expected absent-key error, got {err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

}
