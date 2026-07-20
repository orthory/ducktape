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
use airlock::wire::CredentialPayload;

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
    arg("--attest").context("--attest is required (tdx|snp)")?.parse()
}

/// Flags -> typed trust roots (tdx/snp only). The roots themselves are pinned
/// (Intel inside dcap-qvl, AMD ARK/ASK from the sev builtins); flags select the
/// product and transport (PCCS URL, VCEK file).
fn resolve_roots(mode: AttestMode) -> Result<airlock::verify::TrustRoots> {
    use airlock::verify::{SnpProduct, SnpRoots, TdxRoots, TrustRoots, VcekSource};
    match mode {
        AttestMode::Tdx => Ok(TrustRoots::Tdx(TdxRoots { pccs_url: arg("--pccs-url") })),
        AttestMode::Snp => {
            let product: SnpProduct = arg("--snp-product")
                .context("--attest snp requires --snp-product milan|genoa|turin")?
                .parse()?;
            let vcek = match arg("--snp-vcek") {
                Some(path) => VcekSource::Der(
                    std::fs::read(&path).with_context(|| format!("read {path}"))?,
                ),
                None => VcekSource::Kds,
            };
            Ok(TrustRoots::Snp(Box::new(SnpRoots::amd(product, vcek)?)))
        }
    }
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
    // Roots come from flags alone — resolve BEFORE any network so a bad
    // --snp-product/--snp-vcek fails fast.
    let roots = resolve_roots(mode)?;
    let (quote, _vendor) = gw.fetch_quote().await?;
    let report_data = airlock::verify::verify_quote(&quote, expected, &roots).await?;
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

    let credential = resolve_credential()?;
    let kind = match &credential {
        CredentialPayload::Bearer { .. } => "static access token (no rotation)",
        CredentialPayload::Refresh { .. } => "refresh token (OAuth, rotates)",
    };
    gw.upload_sealed_credential(&seal_pk, &credential).await?;
    println!("✓ {kind} sealed to enclave key and uploaded (gateway never sees it in clear)");
    Ok(())
}

async fn run_cmd() -> Result<()> {
    let mode = attest_mode()?;
    let expected = measurement()?;
    let gw = resolve_gateway()?;
    let sub = arg_or("--sub", "demo");

    let seal_pk = attested_seal_pk(&gw, mode, &expected).await?;
    let (token, keys) = gw.open_session_sealed(&seal_pk, &sub).await?;
    println!("✓ sealed session established (attested handshake, scoped to sub={sub})");

    let body = serde_json::json!({
        "model": arg_or("--model", "claude-sonnet-5"),
        "max_tokens": arg("--max-tokens").and_then(|s| s.parse::<u32>().ok()).unwrap_or(64),
        "stream": true,
        "messages": [{ "role": "user", "content": arg_or("--prompt", "Say hello in three words.") }],
    });
    let sealed_body = airlock::bodyseal::seal_request(&keys, &serde_json::to_vec(&body)?);
    let resp = gw
        .route(gw.http().post(gw.url("/v1/messages")))
        .bearer_auth(token)
        .header("anthropic-version", "2023-06-01")
        // a subscription (OAuth) access token is only accepted with the oauth beta
        // capability header — the same one Claude Code sends. Harmless with an API key.
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("content-type", "application/json")
        .header(airlock::bodyseal::SEAL_HEADER, airlock::bodyseal::SEAL_V1)
        .body(sealed_body.clone())
        .send()
        .await?;
    let status = resp.status();
    let sealed_outer = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/octet-stream"));
    println!("gateway /v1/messages -> {status}");
    if sealed_outer {
        // A self-test is fine buffered: unseal the whole stream and print it.
        let wire = resp.bytes().await?;
        let mut opener = airlock::bodyseal::StreamOpener::new(&keys, &airlock::bodyseal::request_binding(&sealed_body));
        let items = opener.feed(&wire)?;
        if !opener.finished() {
            bail!("sealed response was truncated (no Final marker)");
        }
        for item in items {
            if let airlock::bodyseal::OpenedItem::Data(data) = item {
                print!("{}", String::from_utf8_lossy(&data));
            }
        }
        println!();
    } else {
        if status.is_success() {
            bail!("sealed session received a plaintext success body (forged by a path host?)");
        }
        println!("{}", resp.text().await?);
    }
    if !status.is_success() {
        bail!("messages call failed: {status}");
    }
    Ok(())
}

async fn inspect_cmd() -> Result<()> {
    let mode = attest_mode()?;
    let gw = resolve_gateway()?;
    let (quote, vendor) = gw.fetch_quote().await?;

    let (mrtd_hex, report_data) = airlock::verify::peek_measurement(mode, &quote)?;
    let (seal_pk, sess_pk) = attest::split_report_data(&report_data);

    eprintln!("attest={mode:?} vendor={vendor} quote={} bytes", quote.len());
    eprintln!("REPORTDATA seal_pk = {}", hex::encode(seal_pk));
    eprintln!("REPORTDATA sess_pk = {}", hex::encode(sess_pk));
    eprintln!("--- pin the line below as --measurement (TOFU; in prod pin from the audited build): ---");
    println!("{mrtd_hex}");
    Ok(())
}

/// Resolve which credential to seal. Direct: `--access-token` (static Bearer, no
/// rotation) or `--refresh-token` (OAuth, rotates). From a Claude credentials
/// file: `--credentials <path>` reads `claudeAiOauth.refreshToken` by default, or
/// its current `accessToken` with `--cred-kind bearer` — the latter seals a live
/// subscription WITHOUT rotating the refresh chain the owner is still using.
fn resolve_credential() -> Result<CredentialPayload> {
    if let Some(access_token) = arg("--access-token") {
        return Ok(CredentialPayload::Bearer { access_token });
    }
    if let Some(refresh_token) = arg("--refresh-token") {
        return Ok(CredentialPayload::Refresh { refresh_token });
    }
    let Some(path) = arg("--credentials") else {
        bail!("provide --access-token, --refresh-token, or --credentials");
    };
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    let j: serde_json::Value = serde_json::from_str(&raw).context("credentials json")?;
    let oauth = &j["claudeAiOauth"];
    match arg_or("--cred-kind", "refresh").as_str() {
        "bearer" => Ok(CredentialPayload::Bearer {
            access_token: oauth["accessToken"]
                .as_str()
                .context("claudeAiOauth.accessToken not found")?
                .to_string(),
        }),
        "refresh" => Ok(CredentialPayload::Refresh {
            refresh_token: oauth["refreshToken"]
                .as_str()
                .context("claudeAiOauth.refreshToken not found")?
                .to_string(),
        }),
        other => bail!("--cred-kind must be 'bearer' or 'refresh', got {other:?}"),
    }
}

