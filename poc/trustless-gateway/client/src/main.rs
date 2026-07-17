//! `tcg-client` — both client roles as subcommands:
//!   seal  (Credential Provider): verify the enclave quote, then seal + upload
//!         the OAuth refresh token. Released ONLY after the quote proves the
//!         audited image.
//!   run   (Computation Provider): get a scoped session token, call the host's
//!         /v1/messages with it. Never holds the credential.

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::{Args, Parser, Subcommand};

use tcg_core::attest::{self, AttestMode, Measurement};
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
    Seal(SealArgs),
    Run(RunArgs),
}

#[derive(Args)]
struct SealArgs {
    #[arg(long, default_value = "http://127.0.0.1:9100")]
    host: String,
    /// mock | tdx
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
    #[arg(long, default_value = "http://127.0.0.1:9100")]
    host: String,
    #[arg(long, default_value = "demo")]
    sub: String,
    #[arg(long, default_value = "Say hello in three words.")]
    prompt: String,
    #[arg(long, default_value = "claude-sonnet-5")]
    model: String,
    #[arg(long, default_value_t = 64)]
    max_tokens: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Seal(a) => seal_cmd(a).await,
        Cmd::Run(a) => run_cmd(a).await,
    }
}

async fn seal_cmd(args: SealArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;
    let expected = Measurement::from_hex(&args.measurement)?;
    let http = reqwest::Client::new();

    let att: AttestationResponse = http
        .get(format!("{}/attestation", args.host))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let quote = BASE64.decode(att.quote_b64).context("quote base64")?;

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

    let status = http
        .post(format!("{}/credential", args.host))
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
    let http = reqwest::Client::new();

    let sess: SessionResponse = http
        .post(format!("{}/session", args.host))
        .json(&SessionRequest { sub: args.sub.clone() })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!("✓ session token issued (scoped to sub={})", args.sub);

    let body = serde_json::json!({
        "model": args.model,
        "max_tokens": args.max_tokens,
        "stream": true,
        "messages": [{ "role": "user", "content": args.prompt }],
    });
    let resp = http
        .post(format!("{}/v1/messages", args.host))
        .bearer_auth(sess.token)
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
    }
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
    let coll = get_collateral(&pccs, quote, std::time::Duration::from_secs(15))
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
