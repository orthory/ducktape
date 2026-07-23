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
    /// path to the user key file (a fresh path mints a plain identity)
    #[arg(long, value_name = "PATH", global = true, default_value = "user.key")]
    key: std::path::PathBuf,
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
            Self::Claude => &["setup-token"],
            Self::Codex => &["login"],
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
    key: std::path::PathBuf,
}

impl VerbCtx {
    /// the node's http base (explicit `--node` wins, else the workspace's).
    fn http_base(&self) -> Result<String, Box<dyn std::error::Error>> {
        redeem_node(self.node.as_deref(), self.network.as_deref())
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
    let user = load_user_signer(&ctx.key, stdin)?;
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
    let user = load_user_signer(&ctx.key, stdin)?;
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
    let user = load_user_signer(&ctx.key, stdin)?;
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
    let user = load_user_signer(&ctx.key, stdin)?;
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
    run_vendor_login(provider, &dir)?;

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
/// wrap is PRESENTATION, not interception: it mirrors every byte the vendor CLI
/// prints straight to this terminal and forwards this terminal's stdin, and — as
/// a convenience — reprints the first authorize URL it sees on its own line.
fn run_vendor_login(provider: ProviderArg, dir: &Path) -> CredResult {
    let mut command = tokio::process::Command::new(provider.binary());
    command.args(provider.login_args());
    command.env(provider.config_env(), dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("login runtime: {e}"))?;
    rt.block_on(pump_login(command))?;
    Ok(())
}

/// Pump the pty: child output → stdout (mirrored, URL surfaced once), this
/// process's stdin → child. Returns when the child's pty closes (EOF).
async fn pump_login(command: tokio::process::Command) -> CredResult {
    use std::sync::Arc;
    use tokio::io::AsyncReadExt as _;

    let session = Arc::new(capability_host::InteractiveSession::spawn_local(command)?);

    // forward this terminal's stdin to the child until the session ends.
    let writer = session.clone();
    let input = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            let n = match stdin.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if writer.write_all(&buf[..n]).await.is_err() {
                break;
            }
        }
    });

    let mut printed_url = false;
    let mut buf = [0u8; 4096];
    loop {
        let n = session
            .read(&mut buf)
            .await
            .map_err(|e| format!("read login output: {e}"))?;
        if n == 0 {
            break;
        }
        mirror_stdout(&buf[..n])?;
        let first_url = (!printed_url)
            .then(|| extract_auth_url(&buf[..n]))
            .flatten();
        if let Some(url) = first_url {
            printed_url = true;
            println!("\nopen this url: {url}");
        }
    }
    input.abort();
    session.close().await;
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

/// The first `https://…` URL in a chunk of login output (up to the next
/// whitespace), or `None` — the convenience reprint the login wrap surfaces.
fn extract_auth_url(chunk: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(chunk);
    let start = text.find("https://")?;
    let rest = &text[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest[..end].to_string())
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
    let payload = serde_json::to_value(message)?;
    let resp = reqwest::blocking::Client::new()
        .post(format!("{base}/v1/submit"))
        .json(&serde_json::json!({ "target": "gateway", "payload": payload }))
        .send()
        .map_err(|e| format!("POST {base}/v1/submit: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("submit rejected ({status}): {body}").into());
    }
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["height"].as_u64())
        .ok_or_else(|| format!("unexpected submit receipt: {body}").into())
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
    let resp = reqwest::blocking::Client::new()
        .post(format!("{base}/v1/query"))
        .json(&serde_json::json!({ "target": target, "query": query }))
        .send()
        .map_err(|e| format!("POST {base}/v1/query: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("query rejected ({status}): {body}").into());
    }
    Ok(serde_json::from_str(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn login_stream_url_extraction() {
        let chunk = b"Visit the following URL to authorize:\n  https://claude.ai/oauth/authorize?code=abc\nthen paste the code.";
        assert_eq!(
            extract_auth_url(chunk),
            Some("https://claude.ai/oauth/authorize?code=abc".to_string())
        );
        assert_eq!(extract_auth_url(b"no url here"), None);
    }
}
