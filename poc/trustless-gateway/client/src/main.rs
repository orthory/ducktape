//! `tcg-client` — both client roles as subcommands, against a gateway reached
//! either LOCALLY (Credential Provider == Computation Provider) or REMOTELY
//! (via a duckdns handle over the overlay):
//!   inspect (either):    read the enclave measurement out of the quote (TOFU).
//!   seal  (Credential Provider): verify the quote, then seal + upload the OAuth
//!         refresh token. Released ONLY after the quote proves the audited image.
//!   run   (Computation Provider): verify the quote, run the session-key
//!         handshake, get a scoped token, call /v1/messages. Never holds the
//!         credential.
//!   token (Computation Provider): same handshake, just print the token (for
//!         ANTHROPIC_AUTH_TOKEN).
//!
//! The `Gateway` abstraction is topology-agnostic: local hits a loopback host;
//! remote hits the local node's browser-gateway with an `x-duck-authority`
//! header, which routes the same paths to the remote node's published service.

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::{Args, Parser, Subcommand};

use tcg_core::attest::{self, AttestMode, Measurement};
use tcg_core::handshake;
use tcg_core::seal;
use tcg_core::wire::{
    AttestationResponse, CredentialPayload, CredentialUpload, SessionRequest, SessionResponse,
};

#[derive(Parser)]
#[command(name = "tcg-client", about = "Trustless credential gateway client")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read the enclave measurement (MRTD) out of the quote, so you can pin it
    /// as --measurement. Prints the MRTD hex to stdout, details to stderr.
    Inspect(InspectArgs),
    Seal(SealArgs),
    Run(RunArgs),
    /// Mint a scoped session token and print it (for ANTHROPIC_AUTH_TOKEN).
    Token(TokenArgs),
}

/// How to reach the gateway. `--host` is local (same-machine loopback); `--remote
/// <handle>.duck --via <browser-gw-url>` is remote over the overlay.
#[derive(Args, Clone)]
struct GatewayArgs {
    #[arg(long, default_value = "http://127.0.0.1:9100")]
    host: String,
    /// duckdns handle of a remote gateway (Credential Provider != Computation
    /// Provider). Requires --via.
    #[arg(long)]
    remote: Option<String>,
    /// The local node's browser-gateway base URL that routes duck:// authorities
    /// onto the overlay. Required with --remote.
    #[arg(long)]
    via: Option<String>,
}

#[derive(Args)]
struct InspectArgs {
    #[command(flatten)]
    gw: GatewayArgs,
    /// mock | tdx | snp
    #[arg(long, default_value = "mock")]
    attest: String,
}

#[derive(Args)]
struct SealArgs {
    #[command(flatten)]
    gw: GatewayArgs,
    /// mock | tdx | snp
    #[arg(long, default_value = "mock")]
    attest: String,
    /// Expected audited-image measurement (48-byte hex).
    #[arg(long)]
    measurement: String,
    /// Literal refresh token (demo). Overrides --credentials.
    #[arg(long)]
    refresh_token: Option<String>,
    /// Read claudeAiOauth.refreshToken from a credentials.json.
    #[arg(long)]
    credentials: Option<String>,
}

#[derive(Args)]
struct RunArgs {
    #[command(flatten)]
    gw: GatewayArgs,
    /// mock | tdx | snp
    #[arg(long, default_value = "mock")]
    attest: String,
    /// Expected audited-image measurement (48-byte hex).
    #[arg(long)]
    measurement: String,
    #[arg(long, default_value = "demo")]
    sub: String,
    #[arg(long, default_value = "Say hello in three words.")]
    prompt: String,
    #[arg(long, default_value = "claude-sonnet-5")]
    model: String,
    #[arg(long, default_value_t = 64)]
    max_tokens: u32,
}

#[derive(Args)]
struct TokenArgs {
    #[command(flatten)]
    gw: GatewayArgs,
    /// mock | tdx | snp
    #[arg(long, default_value = "mock")]
    attest: String,
    /// Expected audited-image measurement (48-byte hex).
    #[arg(long)]
    measurement: String,
    #[arg(long, default_value = "demo")]
    sub: String,
}

/// Topology-agnostic handle to the gateway.
struct Gateway {
    base: String,
    /// `Some(authority)` on the remote path — sent as `x-duck-authority` so the
    /// local node's browser-gateway routes the request onto the overlay.
    authority: Option<String>,
    http: reqwest::Client,
}

impl Gateway {
    fn resolve(a: &GatewayArgs) -> Result<Self> {
        let (base, authority) = if let Some(handle) = &a.remote {
            let via = a
                .via
                .clone()
                .context("--remote requires --via <browser-gw-url>")?;
            (via, Some(handle.clone()))
        } else {
            (a.host.clone(), None)
        };
        Ok(Self { base, authority, http: reqwest::Client::new() })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    /// Add the overlay-routing header on the remote path; a no-op locally.
    fn route(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.authority {
            Some(a) => rb.header("x-duck-authority", a),
            None => rb,
        }
    }

    async fn attestation(&self) -> Result<AttestationResponse> {
        Ok(self
            .route(self.http.get(self.url("/attestation")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Inspect(a) => inspect_cmd(a).await,
        Cmd::Seal(a) => seal_cmd(a).await,
        Cmd::Run(a) => run_cmd(a).await,
        Cmd::Token(a) => token_cmd(a).await,
    }
}

async fn inspect_cmd(args: InspectArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;
    let gw = Gateway::resolve(&args.gw)?;
    let att = gw.attestation().await?;
    let quote = BASE64.decode(&att.quote_b64).context("quote base64")?;

    let (mrtd_hex, report_data, extra) = match mode {
        AttestMode::Mock => {
            let (m, rd) = attest::mock_peek(&quote)?;
            (m.to_hex(), rd, Vec::new())
        }
        AttestMode::Tdx => tdx_inspect(&quote)?,
        AttestMode::Snp => snp_inspect(&quote)?,
    };
    let (seal_pk, sess_pk) = attest::split_report_data(&report_data);

    eprintln!("attest={mode:?} vendor={} quote={} bytes", att.vendor, quote.len());
    for line in &extra {
        eprintln!("{line}");
    }
    eprintln!("REPORTDATA seal_pk = {}", hex::encode(seal_pk));
    eprintln!("REPORTDATA sess_pk = {}", hex::encode(sess_pk));
    eprintln!("--- pin the line below as --measurement (TOFU; in prod pin from the audited build): ---");
    println!("{mrtd_hex}");
    Ok(())
}

async fn seal_cmd(args: SealArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;
    let expected = Measurement::from_hex(&args.measurement)?;
    let gw = Gateway::resolve(&args.gw)?;

    let att = gw.attestation().await?;
    let quote = BASE64.decode(&att.quote_b64).context("quote base64")?;

    // Verify BEFORE releasing anything.
    let report_data = verify_quote(mode, &quote, &expected).await?;
    let (seal_pk, _sess_pk) = attest::split_report_data(&report_data);
    println!(
        "✓ quote verified: measurement matches audited image ({}…), seal key bound",
        &expected.to_hex()[..12]
    );

    let refresh = resolve_refresh_token(&args)?;
    let payload = serde_json::to_vec(&CredentialPayload { refresh_token: refresh })?;
    let sealed = seal::seal(&seal_pk, &payload);

    let status = gw
        .route(gw.http.post(gw.url("/credential")))
        .json(&CredentialUpload { sealed_b64: BASE64.encode(sealed) })
        .send()
        .await?
        .status();
    if !status.is_success() {
        bail!("credential upload failed: {status}");
    }
    println!("✓ refresh token sealed to enclave key and uploaded (host never sees it in clear)");
    Ok(())
}

async fn run_cmd(args: RunArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;
    let expected = Measurement::from_hex(&args.measurement)?;
    let gw = Gateway::resolve(&args.gw)?;

    let token = handshake_token(&gw, mode, &expected, &args.sub).await?;
    println!("✓ session established (attested handshake, scoped to sub={})", args.sub);

    let body = serde_json::json!({
        "model": args.model,
        "max_tokens": args.max_tokens,
        "stream": true,
        "messages": [{ "role": "user", "content": args.prompt }],
    });
    let resp = gw
        .route(gw.http.post(gw.url("/v1/messages")))
        .bearer_auth(token)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&body)?)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    println!("host /v1/messages -> {status}");
    println!("{text}");
    if !status.is_success() {
        bail!("messages call failed: {status}");
    }
    Ok(())
}

async fn token_cmd(args: TokenArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;
    let expected = Measurement::from_hex(&args.measurement)?;
    let gw = Gateway::resolve(&args.gw)?;
    let token = handshake_token(&gw, mode, &expected, &args.sub).await?;
    println!("{token}");
    Ok(())
}

/// The Computation Provider's handshake: verify the quote (so `seal_pk` is
/// attested), ECDH against it to derive the session key, ask for a token, and
/// open the token that comes back sealed under that key. A gateway that did not
/// possess the attested seal secret cannot produce a token this opens.
async fn handshake_token(
    gw: &Gateway,
    mode: AttestMode,
    expected: &Measurement,
    sub: &str,
) -> Result<String> {
    let att = gw.attestation().await?;
    let quote = BASE64.decode(&att.quote_b64).context("quote base64")?;
    let report_data = verify_quote(mode, &quote, expected).await?;
    let (seal_pk, _sess_pk) = attest::split_report_data(&report_data);

    let (client_eph_pk, session_key) = handshake::client_handshake(&seal_pk);
    let resp: SessionResponse = gw
        .route(gw.http.post(gw.url("/session")))
        .json(&SessionRequest {
            sub: sub.to_string(),
            client_eph_pk_b64: BASE64.encode(client_eph_pk),
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let sealed = BASE64.decode(&resp.sealed_token_b64).context("sealed token base64")?;
    let token = handshake::open_token(&session_key, &sealed)
        .context("open session token (handshake key mismatch — quote not from the real enclave?)")?;
    String::from_utf8(token).context("session token was not utf-8")
}

fn resolve_refresh_token(args: &SealArgs) -> Result<String> {
    if let Some(t) = &args.refresh_token {
        return Ok(t.clone());
    }
    if let Some(path) = &args.credentials {
        let raw = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
        let j: serde_json::Value = serde_json::from_str(&raw).context("credentials json")?;
        return j["claudeAiOauth"]["refreshToken"]
            .as_str()
            .map(|s| s.to_string())
            .context("claudeAiOauth.refreshToken not found");
    }
    bail!("provide --refresh-token or --credentials");
}

async fn verify_quote(
    mode: AttestMode,
    quote: &[u8],
    expected: &Measurement,
) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    match mode {
        AttestMode::Mock => attest::mock_verify(quote, expected),
        AttestMode::Tdx => tdx_verify(quote, expected).await,
        AttestMode::Snp => snp_verify(quote, expected),
    }
}

// ===== Intel TDX (feature `tdx`, dcap-qvl) ==================================

/// Offline: parse the TDX quote and read MRTD/RTMRs/report_data. No collateral
/// / network — structural parse only.
#[cfg(feature = "tdx")]
fn tdx_inspect(quote: &[u8]) -> Result<(String, [u8; attest::REPORT_DATA_LEN], Vec<String>)> {
    use dcap_qvl::quote::Quote;
    let q = Quote::parse(quote).map_err(|e| anyhow::anyhow!("parse quote: {e:?}"))?;
    let td = q.report.as_td10().context("quote is not a TDX TD10 report")?;
    let mut rd = [0u8; attest::REPORT_DATA_LEN];
    rd.copy_from_slice(&td.report_data[..attest::REPORT_DATA_LEN]);
    let extra = vec![
        format!("MRTD  {}", hex::encode(td.mr_td)),
        format!("RTMR0 {}", hex::encode(td.rt_mr0)),
        format!("RTMR1 {}", hex::encode(td.rt_mr1)),
        format!("RTMR2 {}", hex::encode(td.rt_mr2)),
        format!("RTMR3 {}", hex::encode(td.rt_mr3)),
    ];
    Ok((hex::encode(td.mr_td), rd, extra))
}

#[cfg(not(feature = "tdx"))]
fn tdx_inspect(_quote: &[u8]) -> Result<(String, [u8; attest::REPORT_DATA_LEN], Vec<String>)> {
    bail!("tcg-client was built without the `tdx` feature (rebuild with --features tdx)")
}

#[cfg(feature = "tdx")]
async fn tdx_verify(
    quote: &[u8],
    expected: &Measurement,
) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    // NOTE: best-effort against dcap-qvl's API; compile + run on the TDX box
    // (needs network to Intel PCS/PCCS for collateral). PCCS_URL overrides the
    // collateral endpoint.
    use dcap_qvl::collateral::get_collateral;
    use dcap_qvl::verify::verify;

    let pccs = std::env::var("PCCS_URL")
        .unwrap_or_else(|_| "https://api.trustedservices.intel.com/tdx/certification/v4/".into());
    let coll = get_collateral(&pccs, quote)
        .await
        .map_err(|e| anyhow::anyhow!("fetch collateral: {e:?}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = verify(quote, &coll, now).map_err(|e| anyhow::anyhow!("dcap verify: {e:?}"))?;
    let td = report.report.as_td10().context("quote is not a TDX TD10 report")?;
    if td.mr_td != expected.0 {
        bail!(
            "MRTD mismatch: {} != expected {}",
            hex::encode(td.mr_td),
            expected.to_hex()
        );
    }
    let mut rd = [0u8; attest::REPORT_DATA_LEN];
    rd.copy_from_slice(&td.report_data[..attest::REPORT_DATA_LEN]);
    Ok(rd)
}

#[cfg(not(feature = "tdx"))]
async fn tdx_verify(
    _quote: &[u8],
    _expected: &Measurement,
) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    bail!("tcg-client was built without the `tdx` feature (rebuild with --features tdx)")
}

// ===== AMD SEV-SNP (feature `snp`) =========================================
//
// The SEV-SNP ATTESTATION_REPORT is a fixed layout (AMD SEV-SNP ABI, Table 22):
//   report_data  at offset 0x050, 64 bytes
//   measurement  at offset 0x090, 48 bytes
// so a structural parse needs no crate. The AMD VCEK/KDS signature chain — the
// part that proves the report is genuine — is NOT verified in this build (this
// box has no SEV-SNP silicon to test against). `snp_inspect` (offline, releases
// nothing) parses structurally; `snp_verify` FAILS CLOSED unless the operator
// explicitly opts into structural-only trust, so a demo can never mistake an
// unverified report for a verified one. Finishing the KDS verify is the SNP
// follow-up (mirrors how the TDX arm started).

#[cfg(feature = "snp")]
const SNP_REPORT_DATA_OFF: usize = 0x050;
#[cfg(feature = "snp")]
const SNP_MEASUREMENT_OFF: usize = 0x090;

#[cfg(feature = "snp")]
fn snp_parse(quote: &[u8]) -> Result<([u8; attest::MRTD_LEN], [u8; attest::REPORT_DATA_LEN])> {
    let end = SNP_MEASUREMENT_OFF + attest::MRTD_LEN;
    if quote.len() < end {
        bail!("SEV-SNP report too short: {} < {end} bytes", quote.len());
    }
    let mut meas = [0u8; attest::MRTD_LEN];
    meas.copy_from_slice(&quote[SNP_MEASUREMENT_OFF..end]);
    let mut rd = [0u8; attest::REPORT_DATA_LEN];
    rd.copy_from_slice(&quote[SNP_REPORT_DATA_OFF..SNP_REPORT_DATA_OFF + attest::REPORT_DATA_LEN]);
    Ok((meas, rd))
}

#[cfg(feature = "snp")]
fn snp_inspect(quote: &[u8]) -> Result<(String, [u8; attest::REPORT_DATA_LEN], Vec<String>)> {
    let (meas, rd) = snp_parse(quote)?;
    let extra = vec!["MEASUREMENT (SEV-SNP launch digest)".to_string()];
    Ok((hex::encode(meas), rd, extra))
}

#[cfg(not(feature = "snp"))]
fn snp_inspect(_quote: &[u8]) -> Result<(String, [u8; attest::REPORT_DATA_LEN], Vec<String>)> {
    bail!("tcg-client was built without the `snp` feature (rebuild with --features snp)")
}

#[cfg(feature = "snp")]
fn snp_verify(quote: &[u8], expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    let (meas, rd) = snp_parse(quote)?;
    if meas != expected.0 {
        bail!(
            "SEV-SNP measurement mismatch: {} != expected {}",
            hex::encode(meas),
            expected.to_hex()
        );
    }
    // The signature chain is what makes the report trustworthy. Until the AMD
    // KDS/VCEK verify is wired, fail closed unless the operator opts in.
    if std::env::var("TCG_SNP_INSECURE_STRUCTURAL").as_deref() != Ok("1") {
        bail!(
            "SEV-SNP report structurally matches but its AMD signature chain is NOT verified in \
             this build. Set TCG_SNP_INSECURE_STRUCTURAL=1 to accept structural-only trust (demo \
             on SNP hardware), or finish the KDS/VCEK verify before trusting it."
        );
    }
    eprintln!("[snp] WARNING: AMD signature chain NOT verified — structural trust only");
    Ok(rd)
}

#[cfg(not(feature = "snp"))]
fn snp_verify(_quote: &[u8], _expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    bail!("tcg-client was built without the `snp` feature (rebuild with --features snp)")
}
