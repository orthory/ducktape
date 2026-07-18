//! `airlock-cli` — the client-side roles as subcommands, against a gateway
//! reached either LOCALLY (`--host`) or REMOTELY (`--remote <handle>.duck
//! --via <browser-gw>`, routed onto the overlay):
//!   seal    (Credential Provider): verify the quote, then seal + upload the
//!           OAuth refresh token. Released ONLY after the quote proves the
//!           audited image.
//!   inspect (either): read the enclave measurement out of the quote, so you can
//!           pin it as --measurement (TOFU for bootstrap).
//!   run     (Computation Provider): verify the quote, run the handshake, call
//!           /v1/messages with the scoped token. A lightweight self-test;
//!           `airlock-broker` is the real compute-side path for agent sandboxes.

use anyhow::{bail, Context, Result};

use airlock::attest::{self, AttestMode, Measurement};
use airlock::client::Gateway;

/// `--flag value` / `--flag=value` lookup over argv (no clap — house rule).
fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
        if let Some(v) = a.strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
    }
    None
}

fn arg_or(name: &str, default: &str) -> String {
    arg(name).unwrap_or_else(|| default.to_string())
}

/// Build the gateway from `--host` (local) or `--remote/--via` (remote).
fn resolve_gateway() -> Result<Gateway> {
    let Some(handle) = arg("--remote") else {
        return Ok(Gateway::local(arg_or("--host", "http://127.0.0.1:9100")));
    };
    let via = arg("--via").context("--remote requires --via <browser-gw-url>")?;
    Ok(Gateway::remote(handle, via))
}

fn attest_mode() -> Result<AttestMode> {
    arg_or("--attest", "mock").parse()
}

fn measurement() -> Result<Measurement> {
    Measurement::from_hex(&arg("--measurement").context("--measurement is required")?)
}

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("seal") => seal_cmd().await,
        Some("inspect") => inspect_cmd().await,
        Some("run") => run_cmd().await,
        other => {
            eprintln!("usage: airlock-cli <seal|inspect|run> [flags]  (got {:?})", other.unwrap_or(""));
            std::process::exit(2);
        }
    }
}

/// Fetch + verify the quote and return the attested seal_pk. Anything downstream
/// trusts seal_pk ONLY because this verified it.
async fn attested_seal_pk(gw: &Gateway, mode: AttestMode, expected: &Measurement) -> Result<[u8; 32]> {
    let (quote, _vendor) = gw.fetch_quote().await?;
    let report_data = verify_quote(mode, &quote, expected).await?;
    Ok(attest::split_report_data(&report_data).0)
}

async fn seal_cmd() -> Result<()> {
    let mode = attest_mode()?;
    let expected = measurement()?;
    let gw = resolve_gateway()?;

    let seal_pk = attested_seal_pk(&gw, mode, &expected).await?;
    println!(
        "✓ quote verified: measurement matches audited image ({}…), seal key bound",
        &expected.to_hex()[..12]
    );

    let refresh = resolve_refresh_token()?;
    gw.upload_sealed_credential(&seal_pk, &refresh).await?;
    println!("✓ refresh token sealed to enclave key and uploaded (gateway never sees it in clear)");
    Ok(())
}

async fn run_cmd() -> Result<()> {
    let mode = attest_mode()?;
    let expected = measurement()?;
    let gw = resolve_gateway()?;
    let sub = arg_or("--sub", "demo");

    let seal_pk = attested_seal_pk(&gw, mode, &expected).await?;
    let token = gw.open_session(&seal_pk, &sub).await?;
    println!("✓ session established (attested handshake, scoped to sub={sub})");

    let body = serde_json::json!({
        "model": arg_or("--model", "claude-sonnet-5"),
        "max_tokens": arg("--max-tokens").and_then(|s| s.parse::<u32>().ok()).unwrap_or(64),
        "stream": true,
        "messages": [{ "role": "user", "content": arg_or("--prompt", "Say hello in three words.") }],
    });
    let resp = gw
        .route(gw.http().post(gw.url("/v1/messages")))
        .bearer_auth(token)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&body)?)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    println!("gateway /v1/messages -> {status}");
    println!("{text}");
    if !status.is_success() {
        bail!("messages call failed: {status}");
    }
    Ok(())
}

async fn inspect_cmd() -> Result<()> {
    let mode = attest_mode()?;
    let gw = resolve_gateway()?;
    let (quote, vendor) = gw.fetch_quote().await?;

    let (mrtd_hex, report_data, extra) = match mode {
        AttestMode::Mock => {
            let (m, rd) = attest::mock_peek(&quote)?;
            (m.to_hex(), rd, Vec::new())
        }
        AttestMode::Tdx => tdx_inspect(&quote)?,
        AttestMode::Snp => snp_inspect(&quote)?,
    };
    let (seal_pk, sess_pk) = attest::split_report_data(&report_data);

    eprintln!("attest={mode:?} vendor={vendor} quote={} bytes", quote.len());
    for line in &extra {
        eprintln!("{line}");
    }
    eprintln!("REPORTDATA seal_pk = {}", hex::encode(seal_pk));
    eprintln!("REPORTDATA sess_pk = {}", hex::encode(sess_pk));
    eprintln!("--- pin the line below as --measurement (TOFU; in prod pin from the audited build): ---");
    println!("{mrtd_hex}");
    Ok(())
}

fn resolve_refresh_token() -> Result<String> {
    if let Some(t) = arg("--refresh-token") {
        return Ok(t);
    }
    let Some(path) = arg("--credentials") else {
        bail!("provide --refresh-token or --credentials");
    };
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    let j: serde_json::Value = serde_json::from_str(&raw).context("credentials json")?;
    j["claudeAiOauth"]["refreshToken"]
        .as_str()
        .map(|s| s.to_string())
        .context("claudeAiOauth.refreshToken not found")
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
    bail!("airlock-cli was built without the `tdx` feature (rebuild with --features tdx)")
}

#[cfg(feature = "tdx")]
async fn tdx_verify(quote: &[u8], expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
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
        bail!("MRTD mismatch: {} != expected {}", hex::encode(td.mr_td), expected.to_hex());
    }
    let mut rd = [0u8; attest::REPORT_DATA_LEN];
    rd.copy_from_slice(&td.report_data[..attest::REPORT_DATA_LEN]);
    Ok(rd)
}

#[cfg(not(feature = "tdx"))]
async fn tdx_verify(_quote: &[u8], _expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    bail!("airlock-cli was built without the `tdx` feature (rebuild with --features tdx)")
}

// ===== AMD SEV-SNP (feature `snp`) =========================================
//
// The SEV-SNP ATTESTATION_REPORT is a fixed layout (AMD SEV-SNP ABI, Table 22):
//   report_data  at offset 0x050, 64 bytes
//   measurement  at offset 0x090, 48 bytes
// The AMD VCEK/KDS signature chain is NOT verified in this build (no SEV-SNP
// silicon to test against). `snp_inspect` parses structurally; `snp_verify`
// FAILS CLOSED unless the operator opts into structural-only trust.

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
    Ok((hex::encode(meas), rd, Vec::new()))
}

#[cfg(not(feature = "snp"))]
fn snp_inspect(_quote: &[u8]) -> Result<(String, [u8; attest::REPORT_DATA_LEN], Vec<String>)> {
    bail!("airlock-cli was built without the `snp` feature (rebuild with --features snp)")
}

#[cfg(feature = "snp")]
fn snp_verify(quote: &[u8], expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    let (meas, rd) = snp_parse(quote)?;
    if meas != expected.0 {
        bail!("SEV-SNP measurement mismatch: {} != expected {}", hex::encode(meas), expected.to_hex());
    }
    if std::env::var("AIRLOCK_SNP_INSECURE_STRUCTURAL").as_deref() != Ok("1") {
        bail!(
            "SEV-SNP report structurally matches but its AMD signature chain is NOT verified in \
             this build. Set AIRLOCK_SNP_INSECURE_STRUCTURAL=1 to accept structural-only trust \
             (demo on SNP hardware), or finish the KDS/VCEK verify before trusting it."
        );
    }
    eprintln!("[snp] WARNING: AMD signature chain NOT verified — structural trust only");
    Ok(rd)
}

#[cfg(not(feature = "snp"))]
fn snp_verify(_quote: &[u8], _expected: &Measurement) -> Result<[u8; attest::REPORT_DATA_LEN]> {
    bail!("airlock-cli was built without the `snp` feature (rebuild with --features snp)")
}
