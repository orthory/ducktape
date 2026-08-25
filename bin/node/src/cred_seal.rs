//! `ducktape user cred inspect|seal` — the credential-provider verbs against a
//! TEE airlock gateway (`bin/airlock-gateway`, running in a confidential VM).
//!
//! These are the ENCLAVE half of credential provision. The self-host half is
//! `cred add`, which captures a login artifact into this node's own store and
//! needs no attestation because its trust anchor is the on-chain seal_pk. Here
//! there is a real enclave, so the credential is released only after its quote
//! proves the audited measurement:
//!
//!   inspect  read the measurement out of the quote, so it can be pinned
//!   seal     verify the quote, then seal + upload the credential under the
//!            attested seal key
//!
//! ## the node is uninvolved in the trust decision
//!
//! Attestation is strictly bilateral: this CLI fetches the quote, verifies it
//! against roots it resolves itself (Intel's pinned inside dcap-qvl, AMD's from
//! the sev builtins), and derives seal_pk from the verified REPORTDATA. Nothing
//! is asked of the node, and nothing about the decision is submitted to it. The
//! node is used for ONE thing, and only in remote mode: reading its own browser
//! gateway base, which is the transport that carries `<handle>.duck` onto the
//! overlay.

use anyhow::Context as _;

use airlock::attest::{self, AttestMode, Measurement};
use airlock::client::Gateway;
use airlock::verify::{SnpProduct, SnpRoots, TdxRoots, TrustRoots, VcekSource};
use airlock::wire::{CredentialKind, CredentialPayload};

use crate::cred_cli::CredResult;

/// Where the gateway is: on this box, or an account's `.duck` handle reached
/// through the node's browser gateway. ONE discriminant, so no verb has to
/// infer the mode from which flags happen to be set.
#[derive(Debug, clap::Args)]
pub(crate) struct GatewayArgs {
    /// a LOCAL gateway's base url
    #[arg(long, value_name = "URL", conflicts_with = "remote")]
    host: Option<String>,
    /// a REMOTE gateway's duck handle (e.g. `airlock.alice.duck`), routed onto
    /// the overlay through this node's own browser gateway
    #[arg(long, value_name = "HANDLE")]
    remote: Option<String>,
}

/// Which silicon, and the transport bits its roots need. The roots themselves
/// are pinned in-crate; these select the product and where to fetch from.
#[derive(Debug, clap::Args)]
pub(crate) struct AttestArgs {
    /// the enclave's attestation family
    #[arg(long, value_name = "KIND")]
    attest: AttestKind,
    /// TDX only: a PCCS to fetch collateral from (default: Intel PCS)
    #[arg(long, value_name = "URL")]
    pccs_url: Option<String>,
    /// SNP only: the CPU generation the VCEK chains to
    #[arg(long, value_name = "PRODUCT")]
    snp_product: Option<SnpProductArg>,
    /// SNP only: a VCEK DER on disk (default: fetch from AMD's KDS)
    #[arg(long, value_name = "PATH")]
    snp_vcek: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum AttestKind {
    Tdx,
    Snp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum SnpProductArg {
    Milan,
    Genoa,
    Turin,
}

/// Which credential to seal, and where it comes from.
#[derive(Debug, clap::Args)]
pub(crate) struct SealArgs {
    /// the name the enclave stores it under (a session's `sub`)
    #[arg(long, value_name = "NAME", default_value = "compute-provider")]
    name: String,
    /// which vendor the enclave routes this credential to
    #[arg(long, value_name = "VENDOR", default_value = "claude")]
    vendor: VendorArg,
    /// the measurement the quote must match, lowercase hex. `inspect` prints it.
    #[arg(long, value_name = "HEX")]
    measurement: String,
    /// a vendor login artifact to read the credential out of
    #[arg(long, value_name = "PATH", conflicts_with_all = ["access_token", "refresh_token"])]
    credentials: Option<std::path::PathBuf>,
    /// with --credentials: seal the artifact's CURRENT access token (no
    /// rotation, so the owner's own login keeps working) or its refresh token
    #[arg(
        long,
        value_name = "KIND",
        default_value = "bearer",
        requires = "credentials"
    )]
    cred_kind: SealKind,
    /// seal a static access token directly (no rotation)
    #[arg(long, value_name = "TOKEN", conflicts_with = "refresh_token")]
    access_token: Option<String>,
    /// seal an OAuth refresh token directly (rotates in-enclave)
    #[arg(long, value_name = "TOKEN")]
    refresh_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum VendorArg {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum SealKind {
    Bearer,
    Refresh,
}

impl AttestArgs {
    fn mode(&self) -> AttestMode {
        match self.attest {
            AttestKind::Tdx => AttestMode::Tdx,
            AttestKind::Snp => AttestMode::Snp,
        }
    }

    /// Flags → typed trust roots. Resolved BEFORE any network call, so a bad
    /// `--snp-product` / unreadable `--snp-vcek` fails fast rather than after a
    /// round trip.
    fn roots(&self) -> Result<TrustRoots, Box<dyn std::error::Error>> {
        match self.attest {
            AttestKind::Tdx => Ok(TrustRoots::Tdx(TdxRoots {
                pccs_url: self.pccs_url.clone(),
            })),
            AttestKind::Snp => {
                let product = match self
                    .snp_product
                    .ok_or("--attest snp requires --snp-product milan|genoa|turin")?
                {
                    SnpProductArg::Milan => SnpProduct::Milan,
                    SnpProductArg::Genoa => SnpProduct::Genoa,
                    SnpProductArg::Turin => SnpProduct::Turin,
                };
                let vcek = match &self.snp_vcek {
                    Some(path) => VcekSource::Der(
                        std::fs::read(path).with_context(|| format!("read {}", path.display()))?,
                    ),
                    None => VcekSource::Kds,
                };
                Ok(TrustRoots::Snp(Box::new(SnpRoots::amd(product, vcek)?)))
            }
        }
    }
}

/// Build the gateway handle. Remote mode reads the node's own browser-gateway
/// base — the transport that carries a `.duck` authority onto the overlay —
/// rather than making the operator paste it.
fn resolve_gateway(
    args: &GatewayArgs,
    node_base: impl FnOnce() -> Result<String, Box<dyn std::error::Error>>,
) -> Result<Gateway, Box<dyn std::error::Error>> {
    let Some(handle) = args.remote.clone() else {
        let host = args
            .host
            .clone()
            .ok_or("give the gateway: --host <url> (local) or --remote <handle>.duck")?;
        return Ok(Gateway::local(host));
    };
    let via = crate::node_http::get_json(&node_base()?, "/v1/gateway/browser")
        .map_err(|error| format!("read this node's browser gateway base: {error}"))?["base"]
        .as_str()
        .ok_or("this node serves no browser gateway, so it cannot route a .duck authority")?
        .to_string();
    Ok(Gateway::remote(handle, via))
}

/// `cred inspect` — print the enclave measurement the quote carries.
///
/// TOFU for bootstrap: in production the measurement comes from the audited
/// build, not from the enclave being asked to describe itself. Printing it is a
/// convenience for pinning, never a verification.
pub(crate) fn cmd_inspect(
    gateway: GatewayArgs,
    attest_args: AttestArgs,
    node_base: impl FnOnce() -> Result<String, Box<dyn std::error::Error>>,
) -> CredResult {
    let gw = resolve_gateway(&gateway, node_base)?;
    let mode = attest_args.mode();
    let (quote, vendor) = block_on(gw.fetch_quote())?;
    let (mrtd_hex, report_data) = airlock::verify::peek_measurement(mode, &quote)?;
    let (seal_pk, sess_pk) = attest::split_report_data(&report_data);

    eprintln!(
        "attest={mode:?} vendor={vendor} quote={} bytes",
        quote.len()
    );
    eprintln!("REPORTDATA seal_pk = {}", hex::encode(seal_pk));
    eprintln!("REPORTDATA sess_pk = {}", hex::encode(sess_pk));
    eprintln!(
        "--- pin the line below as --measurement (TOFU; in prod pin from the audited build) ---"
    );
    // stdout is the measurement alone, so `$(... cred inspect ...)` is usable.
    println!("{mrtd_hex}");
    Ok(())
}

/// `cred seal` — verify the quote, then seal and upload the credential.
///
/// The credential is released ONLY after the quote proves the pinned
/// measurement: seal_pk is trusted because [`airlock::verify::verify_quote`]
/// verified the chain that binds it, never because the gateway asserted it.
pub(crate) fn cmd_seal(
    gateway: GatewayArgs,
    attest_args: AttestArgs,
    seal: SealArgs,
    node_base: impl FnOnce() -> Result<String, Box<dyn std::error::Error>>,
) -> CredResult {
    // resolve everything local and fallible BEFORE the network: bad roots, a bad
    // measurement or an unreadable artifact must fail before a quote is fetched.
    let roots = attest_args.roots()?;
    let expected = Measurement::from_hex(&seal.measurement)?;
    let credential = resolve_credential(&seal)?;
    let kind = match seal.vendor {
        VendorArg::Claude => CredentialKind::Claude,
        VendorArg::Codex => CredentialKind::Codex,
    };
    let gw = resolve_gateway(&gateway, node_base)?;

    let seal_pk = block_on(async {
        let (quote, _vendor) = gw.fetch_quote().await?;
        let report_data = airlock::verify::verify_quote(&quote, &expected, &roots).await?;
        anyhow::Ok(attest::split_report_data(&report_data).0)
    })?;
    println!(
        "quote verified: measurement matches the audited image ({}…), seal key bound",
        &expected.to_hex()[..12]
    );

    block_on(gw.upload_sealed_credential(&seal_pk, &seal.name, kind, &credential))?;
    let rotation = match &credential {
        CredentialPayload::Bearer { .. } => "static access token (no rotation)",
        CredentialPayload::Refresh { .. } => "refresh token (OAuth, rotates in-enclave)",
    };
    println!(
        "sealed {rotation} and uploaded as {:?} (the gateway never sees it in clear)",
        seal.name
    );
    Ok(())
}

/// Which secret to seal. Direct flags win; otherwise read the vendor artifact.
fn resolve_credential(seal: &SealArgs) -> Result<CredentialPayload, Box<dyn std::error::Error>> {
    if let Some(access_token) = seal.access_token.clone() {
        return Ok(CredentialPayload::Bearer { access_token });
    }
    if let Some(refresh_token) = seal.refresh_token.clone() {
        return Ok(CredentialPayload::Refresh {
            refresh_token,
            access_token: String::new(),
            expires_at: 0,
        });
    }
    let path = seal
        .credentials
        .as_ref()
        .ok_or("give the secret: --credentials <path>, --access-token, or --refresh-token")?;
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&raw).context("credentials json")?;
    let oauth = &json["claudeAiOauth"];
    let field = |key: &str| -> Result<String, String> {
        oauth[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("{}: claudeAiOauth.{key} not found", path.display()))
    };
    match seal.cred_kind {
        SealKind::Bearer => Ok(CredentialPayload::Bearer {
            access_token: field("accessToken")?,
        }),
        SealKind::Refresh => Ok(CredentialPayload::Refresh {
            refresh_token: field("refreshToken")?,
            access_token: oauth["accessToken"].as_str().unwrap_or("").to_string(),
            expires_at: oauth["expiresAt"].as_u64().map(|ms| ms / 1000).unwrap_or(0),
        }),
    }
}

/// The `cred` family is a synchronous CLI; the airlock client is async. One
/// current-thread runtime per verb, built where it is used.
fn block_on<T>(
    future: impl std::future::Future<Output = anyhow::Result<T>>,
) -> Result<T, Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(future)?)
}
