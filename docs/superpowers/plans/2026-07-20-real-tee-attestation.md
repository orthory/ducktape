# Real TEE Attestation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete mock attestation entirely and ship real Intel TDX (dcap-qvl) and AMD SEV-SNP (VCEK chain) quote verification as the only path, wired into `capability-host`, `airlock-broker`, and `airlock-cli`.

**Architecture:** One shared verifier module `airlock::verify` (feature `verify`) holds both vendor verifiers behind a `TrustRoots` value whose production constructors hard-pin the Intel/AMD roots. The gateway server gains a `Quoter` injection seam so in-process tests on non-TEE hardware mint an SNP-format quote signed by a test chain (`airlock::testkit`, feature `testkit`) and verify it through the REAL verify path with test roots — a caller injecting fake roots only fools itself. Mock is deleted last, compiler-guided.

**Tech Stack:** `sev = 8.0` (`snp` + `crypto_nossl`: pure-RustCrypto AMD chain verify, builtin ARK/ASK), `dcap-qvl = 0.5` (latest — USER DIRECTIVE mid-execution: latest versions for security deps; `rustcrypto` backend, Intel root pinned in-crate, `CollateralClient::with_default_http(INTEL_PCS_URL)` replaces 0.3's `get_collateral_from_pcs`), `p384 0.13`/`rsa 0.9`/`x509-cert 0.2` (pinned to sev 8's own graph so cert types stay identical — bump when sev bumps), reqwest (KDS fetch).

**Spec:** `docs/superpowers/specs/2026-07-20-real-tee-attestation-design.md`

## Global Constraints

- Workspace FORBIDS clap; args are hand-rolled `--flag value` lookups (see `bin/airlock-cli/src/main.rs::arg`).
- Edition 2024; axum 0.8 route syntax `/v1/{*rest}`.
- `CARGO_INCREMENTAL=0` on every cargo invocation (this box's rustc segfault workaround).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`.
- Never log key material or URI paths/query strings (KDS URLs carry chip_id — keep them out of error strings; report only HTTP status).
- Standalone bins (`airlock-cli`, `airlock-broker`, `airlock-gateway`) use `println!`/`eprintln!` (coordinator precedent); anything reachable from the node uses `tracing`.
- No `--attest` / `DUCKTAPE_AIRLOCK_ATTEST` value may default; after Task 7 the accepted values are `tdx | snp` (server config also accepts `auto`).
- Commit trailer: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Worktree: `/home/eddy/dev/ducktape/.worktree/real-tee-attest`, branch `feat-airlock-real-attest`, PR against `dev`.
- Crate API references live in the scratchpad: `sev-8.0.0/` and `dcap-qvl-0.3.12/` under `/tmp/claude-1000/-home-eddy-dev-ducktape/eeec504e-b8d1-429b-8895-935b2bf3362d/scratchpad/` — consult before guessing any sev/dcap API.

---

### Task 1: `airlock::verify` (SNP real chain verify) + `airlock::testkit`

**Files:**
- Modify: `crates/system/airlock/Cargo.toml`
- Modify: `crates/system/airlock/src/lib.rs`
- Create: `crates/system/airlock/src/verify.rs`
- Create: `crates/system/airlock/src/testkit.rs`
- Create: `crates/system/airlock/tests/verify_snp.rs`

**Interfaces (produced, relied on by every later task):**
- `airlock::verify::{TrustRoots, TdxRoots, SnpRoots, SnpProduct, VcekSource}` — all `Clone`, **plain data**. USER FEEDBACK (mid-execution): the airlock lib NEVER reads env — each binary parses its config ONCE at its boundary into these typed values (capability-host's `AirlockConfig::from_env` stays the single env reader; the bins parse flags) and hands them in. No `TrustRoots::from_env` in the lib.
- `airlock::verify::verify_quote(quote: &[u8], expected: &Measurement, roots: &TrustRoots) -> anyhow::Result<[u8; REPORT_DATA_LEN]>` (async).
- `airlock::testkit::SnpTestEnclave` with `new(&Measurement) -> Result<Self>`, `quote(&self, &[u8; 64]) -> Result<Vec<u8>>`, `roots(&self) -> TrustRoots`.

- [ ] **Step 1: Cargo features + deps**

In `crates/system/airlock/Cargo.toml` add:

```toml
[features]
# ... existing client/server ...
# Real quote verification: TDX via dcap-qvl (Intel root pinned in-crate), SNP
# via the AMD VCEK chain (sev crate, crypto_nossl). reqwest for PCS/KDS fetch.
verify = ["dep:dcap-qvl", "dep:sev", "dep:reqwest"]
# Test-only fake silicon: mints an SNP-format quote signed by a test chain that
# verifies ONLY under its own minted roots — never under AMD's.
testkit = ["verify", "dep:p384", "dep:rsa", "dep:x509-cert", "dep:rand"]

[dependencies]
# ... existing ...
dcap-qvl = { version = "0.3", optional = true }
sev = { version = "8", default-features = false, features = ["snp", "crypto_nossl"], optional = true }
p384 = { version = "0.13", optional = true }
rsa = { version = "0.9", features = ["sha2"], optional = true }
x509-cert = { version = "0.2", features = ["builder"], optional = true }
rand = { version = "0.8", optional = true }
```

In `src/lib.rs`, next to the existing `client`/`server` feature gates:

```rust
#[cfg(feature = "verify")]
pub mod verify;
#[cfg(feature = "testkit")]
pub mod testkit;
```

- [ ] **Step 2: Write the failing test** — `crates/system/airlock/tests/verify_snp.rs`:

```rust
//! Real SEV-SNP verification exercised end-to-end on non-TEE hardware: the
//! testkit mints a chain + report, and `airlock::verify` runs the REAL parser,
//! certificate-chain verify, and report-signature verify against it.
#![cfg(feature = "testkit")]

use airlock::attest::{self, Measurement, MRTD_LEN};
use airlock::testkit::SnpTestEnclave;
use airlock::verify::verify_quote;

fn meas(b: u8) -> Measurement {
    Measurement([b; MRTD_LEN])
}

#[tokio::test]
async fn minted_snp_quote_verifies_and_binds_report_data() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let rd = attest::make_report_data(&[9u8; 32], &[8u8; 32]);
    let quote = enclave.quote(&rd).unwrap();
    let out = verify_quote(&quote, &meas(0x11), &enclave.roots()).await.unwrap();
    assert_eq!(out, rd);
}

#[tokio::test]
async fn wrong_measurement_is_rejected() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let quote = enclave.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    assert!(verify_quote(&quote, &meas(0x22), &enclave.roots()).await.is_err());
}

#[tokio::test]
async fn tampered_report_data_breaks_the_signature() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let mut quote = enclave.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    quote[0x50] ^= 1; // REPORT_DATA offset in the SNP report
    assert!(verify_quote(&quote, &meas(0x11), &enclave.roots()).await.is_err());
}

#[tokio::test]
async fn a_quote_from_a_different_chain_is_rejected() {
    // Two independently minted enclaves: A's roots must refuse B's quote.
    let a = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let b = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let quote = b.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    assert!(verify_quote(&quote, &meas(0x11), &a.roots()).await.is_err());
}

#[tokio::test]
async fn garbage_quote_is_rejected() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    assert!(verify_quote(&[0u8; 32], &meas(0x11), &enclave.roots()).await.is_err());
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd /home/eddy/dev/ducktape/.worktree/real-tee-attest && CARGO_INCREMENTAL=0 cargo test -p airlock --features testkit --test verify_snp 2>&1 | tail -5`
Expected: compile FAIL — `verify`/`testkit` modules do not exist yet.

- [ ] **Step 4: Implement `src/verify.rs`**

```rust
//! Real TEE quote verification — the client-side half of attestation.
//! TDX via dcap-qvl (Intel root CA pinned inside the crate); SEV-SNP via the
//! AMD VCEK chain (sev crate, crypto_nossl backend; ARK/ASK pinned from the
//! crate builtins). There is no mock: a quote that does not chain to Intel or
//! AMD silicon roots does not verify. Injecting non-default `TrustRoots` only
//! weakens the injector — production entry points construct pinned roots.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, bail, Context, Result};
use sev::certs::snp::{ca, Certificate, Chain, Verifiable};
use sev::firmware::guest::AttestationReport;
use sev::parser::Decoder;

use crate::attest::{AttestMode, Measurement, REPORT_DATA_LEN};

/// AMD EPYC generation whose pinned root keys the verifier trusts. Pinned by
/// the operator beside the measurement — a measurement only means something on
/// the platform generation it was audited for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnpProduct {
    Milan,
    Genoa,
    Turin,
}

impl SnpProduct {
    fn kds_segment(self) -> &'static str {
        match self {
            Self::Milan => "Milan",
            Self::Genoa => "Genoa",
            Self::Turin => "Turin",
        }
    }

    fn builtin_ca(self) -> Result<ca::Chain> {
        use sev::certs::snp::builtin::{genoa, milan, turin};
        let (ark, ask) = match self {
            Self::Milan => (milan::ark(), milan::ask()),
            Self::Genoa => (genoa::ark(), genoa::ask()),
            Self::Turin => (turin::ark(), turin::ask()),
        };
        Ok(ca::Chain { ark: ark.context("builtin ARK")?, ask: ask.context("builtin ASK")? })
    }
}

impl std::str::FromStr for SnpProduct {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "milan" => Ok(Self::Milan),
            "genoa" => Ok(Self::Genoa),
            "turin" => Ok(Self::Turin),
            other => bail!("unknown SNP product {other:?} (want milan|genoa|turin)"),
        }
    }
}

/// Where the per-chip VCEK leaf certificate comes from. Transport, not trust:
/// whatever this yields must still chain to the pinned ARK/ASK.
#[derive(Clone)]
pub enum VcekSource {
    /// Fetch from the AMD Key Distribution Service by chip id + reported TCB.
    Kds,
    /// A DER certificate supplied out of band (air-gapped operation, tests).
    Der(Vec<u8>),
}

#[derive(Clone)]
pub struct SnpRoots {
    pub product: SnpProduct,
    pub ca: ca::Chain,
    pub vcek: VcekSource,
}

impl SnpRoots {
    /// Production roots: AMD's builtin ARK/ASK for `product`.
    pub fn amd(product: SnpProduct, vcek: VcekSource) -> Result<Self> {
        Ok(Self { product, ca: product.builtin_ca()?, vcek })
    }
}

#[derive(Clone)]
pub struct TdxRoots {
    /// Collateral endpoint; `None` = Intel PCS. The root CA the chain must
    /// anchor to is pinned inside dcap-qvl regardless.
    pub pccs_url: Option<String>,
}

#[derive(Clone)]
pub enum TrustRoots {
    Tdx(TdxRoots),
    Snp(SnpRoots),
}

impl TrustRoots {
    /// Production construction from the broker's `DUCKTAPE_AIRLOCK_*` env. The
    /// roots themselves are hard-pinned; env selects vendor/product and
    /// transport only (PCCS URL, VCEK file path).
    pub fn from_env(mode: AttestMode) -> Result<Self> {
        fn env_nonempty(k: &str) -> Option<String> {
            std::env::var(k).ok().filter(|v| !v.is_empty())
        }
        match mode {
            AttestMode::Tdx => {
                Ok(Self::Tdx(TdxRoots { pccs_url: env_nonempty("DUCKTAPE_AIRLOCK_PCCS_URL") }))
            }
            AttestMode::Snp => {
                let product: SnpProduct = env_nonempty("DUCKTAPE_AIRLOCK_SNP_PRODUCT")
                    .context("attest=snp requires DUCKTAPE_AIRLOCK_SNP_PRODUCT (milan|genoa|turin)")?
                    .parse()?;
                let vcek = match env_nonempty("DUCKTAPE_AIRLOCK_SNP_VCEK") {
                    Some(path) => VcekSource::Der(
                        std::fs::read(&path).with_context(|| format!("read VCEK file {path}"))?,
                    ),
                    None => VcekSource::Kds,
                };
                Ok(Self::Snp(SnpRoots::amd(product, vcek)?))
            }
        }
    }
}

/// Verify `quote` against pinned trust roots and the expected measurement and
/// return the bound REPORTDATA. The ONLY way a caller learns a seal_pk.
pub async fn verify_quote(
    quote: &[u8],
    expected: &Measurement,
    roots: &TrustRoots,
) -> Result<[u8; REPORT_DATA_LEN]> {
    match roots {
        TrustRoots::Tdx(t) => verify_tdx(quote, expected, t).await,
        TrustRoots::Snp(s) => verify_snp(quote, expected, s).await,
    }
}

// ===== AMD SEV-SNP ==========================================================

async fn verify_snp(
    quote: &[u8],
    expected: &Measurement,
    roots: &SnpRoots,
) -> Result<[u8; REPORT_DATA_LEN]> {
    let report = AttestationReport::decode(&mut &quote[..], ())
        .context("parse SEV-SNP attestation report")?;

    let signing_key = report.key_info.signing_key();
    if signing_key != 0 {
        bail!("SEV-SNP report signed by a non-VCEK key (signing_key={signing_key}); only VCEK is supported");
    }

    let vcek_der = match &roots.vcek {
        VcekSource::Der(der) => der.clone(),
        VcekSource::Kds => fetch_vcek_kds(&report, roots.product).await?,
    };
    let vcek = Certificate::from_der(&vcek_der)
        .map_err(|e| anyhow!("parse VCEK certificate: {e}"))?;

    let chain = Chain { ca: roots.ca.clone(), vek: vcek };
    (&chain, &report)
        .verify()
        .map_err(|e| anyhow!("SEV-SNP chain/report signature: {e}"))?;

    if report.measurement != expected.0 {
        bail!(
            "SEV-SNP measurement mismatch: {} != expected {} (not the audited image)",
            hex::encode(report.measurement),
            expected.to_hex()
        );
    }
    Ok(report.report_data)
}

/// Fetch the VCEK from AMD KDS by product / chip id / reported TCB SPLs,
/// caching per URL (KDS rate-limits aggressively; a broker re-verifies the
/// same gateway across restarts). Never put the URL in an error — it carries
/// the chip id.
async fn fetch_vcek_kds(report: &AttestationReport, product: SnpProduct) -> Result<Vec<u8>> {
    let tcb = &report.reported_tcb;
    // Turin chips carry an 8-byte hwid in a 64-byte field.
    let hwid = match product {
        SnpProduct::Turin => hex::encode(&report.chip_id[..8]),
        _ => hex::encode(report.chip_id),
    };
    let mut url = format!(
        "https://kdsintf.amd.com/vcek/v1/{}/{}?blSPL={:02}&teeSPL={:02}&snpSPL={:02}&ucodeSPL={:02}",
        product.kds_segment(),
        hwid,
        tcb.bootloader,
        tcb.tee,
        tcb.snp,
        tcb.microcode
    );
    if let Some(fmc) = tcb.fmc {
        url.push_str(&format!("&fmcSPL={fmc:02}"));
    }

    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(&url).cloned() {
        return Ok(hit);
    }

    let resp = reqwest::get(&url).await.context("fetch VCEK from AMD KDS")?;
    if !resp.status().is_success() {
        bail!("AMD KDS refused the VCEK request: HTTP {}", resp.status());
    }
    let der = resp.bytes().await.context("read VCEK body")?.to_vec();
    cache.lock().unwrap().insert(url, der.clone());
    Ok(der)
}
```

(TDX fns land in Task 2 — for now `TdxRoots` verify arm can `bail!("tdx verify lands in task 2")` so this task compiles; Task 2 replaces it.)

- [ ] **Step 5: Implement `src/testkit.rs`**

```rust
//! Fake silicon for tests: mints an SNP-format quote signed by a freshly
//! generated test chain. Verifies ONLY under `self.roots()` — never under the
//! AMD builtins — so nothing here can forge a quote a production verifier
//! accepts. RSA-2048 stands in for AMD's 4096-bit CA keys: the verifier checks
//! the algorithm (RSA-PSS/SHA-384), not the size, and 2048 keeps keygen fast.

use anyhow::{anyhow, Context, Result};
use p384::ecdsa::signature::DigestSigner;
use sev::certs::snp::{ca, Certificate};
use sev::firmware::guest::AttestationReport;
use sev::parser::{Decoder, Encoder};
use sha2::{Digest, Sha384};

use crate::attest::{Measurement, REPORT_DATA_LEN};
use crate::verify::{SnpProduct, SnpRoots, TrustRoots, VcekSource};

/// SNP report byte layout facts the minting relies on (AMD SEV-SNP ABI):
/// the signature covers bytes [0, 0x2a0); REPORT_DATA sits at 0x50.
const SIGNED_LEN: usize = 0x2a0;

pub struct SnpTestEnclave {
    ca: ca::Chain,
    vcek_der: Vec<u8>,
    signer: p384::ecdsa::SigningKey,
    measurement: Measurement,
}

impl SnpTestEnclave {
    pub fn new(measurement: &Measurement) -> Result<Self> {
        let mut rng = rand::rngs::OsRng;
        let ark_key = rsa::pss::SigningKey::<Sha384>::new(
            rsa::RsaPrivateKey::new(&mut rng, 2048).context("ARK keygen")?,
        );
        let ask_key = rsa::pss::SigningKey::<Sha384>::new(
            rsa::RsaPrivateKey::new(&mut rng, 2048).context("ASK keygen")?,
        );
        let vcek_key = p384::ecdsa::SigningKey::random(&mut rng);

        let ark_der = mint_cert("CN=test ARK", "CN=test ARK", rsa_spki(&ark_key)?, &ark_key)?;
        let ask_der = mint_cert("CN=test ARK", "CN=test ASK", rsa_spki(&ask_key)?, &ark_key)?;
        let vcek_der = mint_cert("CN=test ASK", "CN=test VCEK", p384_spki(&vcek_key)?, &ask_key)?;

        Ok(Self {
            ca: ca::Chain {
                ark: Certificate::from_der(&ark_der).map_err(|e| anyhow!("ARK parse: {e}"))?,
                ask: Certificate::from_der(&ask_der).map_err(|e| anyhow!("ASK parse: {e}"))?,
            },
            vcek_der,
            signer: vcek_key,
            measurement: *measurement,
        })
    }

    /// Mint a v2 SNP report carrying `report_data`, signed by the test VCEK.
    pub fn quote(&self, report_data: &[u8; REPORT_DATA_LEN]) -> Result<Vec<u8>> {
        let mut report = AttestationReport::default();
        report.version = 2;
        report.sig_algo = 1; // ECDSA P-384 with SHA-384
        report.chip_id = [1u8; 64]; // non-Turin-like -> legacy TCB layout
        report.measurement = self.measurement.0;
        report.report_data = *report_data;

        let mut bytes = Vec::new();
        report.encode(&mut bytes, ()).context("encode unsigned report")?;

        let digest = Sha384::new_with_prefix(&bytes[..SIGNED_LEN]);
        let sig: p384::ecdsa::Signature = self.signer.sign_digest(digest);
        let (r, s) = (sig.r().to_bytes(), sig.s().to_bytes());
        report.signature = sev::certs::snp::ecdsa::Signature::new(le72(&r), le72(&s));

        let mut out = Vec::new();
        report.encode(&mut out, ()).context("encode signed report")?;
        // Sanity: the signed prefix must be unchanged by the re-encode.
        debug_assert_eq!(&out[..SIGNED_LEN], &bytes[..SIGNED_LEN]);
        // Round-trip check: the minted bytes must parse back.
        AttestationReport::decode(&mut &out[..], ()).context("minted report must re-parse")?;
        Ok(out)
    }

    /// Trust roots under which — and only under which — `quote()` verifies.
    pub fn roots(&self) -> TrustRoots {
        TrustRoots::Snp(SnpRoots {
            product: SnpProduct::Genoa,
            ca: self.ca.clone(),
            vcek: VcekSource::Der(self.vcek_der.clone()),
        })
    }
}

/// 48-byte big-endian scalar -> the report's 72-byte little-endian field.
fn le72(be: &[u8]) -> [u8; 72] {
    let mut out = [0u8; 72];
    for (i, b) in be.iter().rev().enumerate() {
        out[i] = *b;
    }
    out
}
```

plus two small helpers `mint_cert` / `rsa_spki` / `p384_spki` using `x509_cert::builder::{Builder, CertificateBuilder, Profile}`:

```rust
fn rsa_spki(key: &rsa::pss::SigningKey<Sha384>) -> Result<x509_cert::spki::SubjectPublicKeyInfoOwned> {
    use rsa::signature::Keypair;
    use x509_cert::spki::EncodePublicKey;
    let der = key.verifying_key().to_public_key_der().context("RSA SPKI")?;
    x509_cert::spki::SubjectPublicKeyInfoOwned::try_from(der.as_bytes()).map_err(|e| anyhow!("SPKI: {e}"))
}

fn p384_spki(key: &p384::ecdsa::SigningKey) -> Result<x509_cert::spki::SubjectPublicKeyInfoOwned> {
    use x509_cert::spki::EncodePublicKey;
    let der = key.verifying_key().to_public_key_der().context("P-384 SPKI")?;
    x509_cert::spki::SubjectPublicKeyInfoOwned::try_from(der.as_bytes()).map_err(|e| anyhow!("SPKI: {e}"))
}

fn mint_cert(
    issuer: &str,
    subject: &str,
    spki: x509_cert::spki::SubjectPublicKeyInfoOwned,
    signer: &rsa::pss::SigningKey<Sha384>,
) -> Result<Vec<u8>> {
    use std::str::FromStr;
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    use x509_cert::der::Encode;
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::time::Validity;

    let profile = Profile::Manual { issuer: Some(Name::from_str(issuer).context("issuer DN")?) };
    let builder = CertificateBuilder::new(
        profile,
        SerialNumber::from(1u32),
        Validity::from_now(std::time::Duration::from_secs(3600 * 24 * 365)).context("validity")?,
        Name::from_str(subject).context("subject DN")?,
        spki,
        signer,
    )
    .context("cert builder")?;
    let cert = builder
        .build_with_rng::<rsa::pss::Signature>(&mut rand::rngs::OsRng)
        .map_err(|e| anyhow!("sign cert: {e}"))?;
    cert.to_der().context("cert DER")
}
```

API notes (verified against vendored sources, may need mechanical adjustment):
`sev-8.0.0/src/certs/snp/cert_nossl.rs` — cert-to-cert verify demands signee
`signature_algorithm == RSA-SSA-PSS` and verifies PSS/SHA-384 over the tbs DER;
no AMD OID extensions are checked, so `Profile::Manual` suffices. If
`build_with_rng` has a different name/shape in x509-cert 0.2, or
`rsa::pss::SigningKey` needs `BlindedSigningKey` for the builder's `Signer`
bound, adjust to whichever of `build`/`build_with_rng` +
`SigningKey`/`BlindedSigningKey` compiles — the assertion of this task is the
test suite, not the exact builder call.

- [ ] **Step 6: Run the tests until green**

Run: `CARGO_INCREMENTAL=0 cargo test -p airlock --features testkit --test verify_snp 2>&1 | tail -8`
Expected: 5 passed. Also run existing suites untouched: `CARGO_INCREMENTAL=0 cargo test -p airlock --features server,client 2>&1 | tail -3` → all pass (mock still present).

- [ ] **Step 7: Lint + commit**

Run: `CARGO_INCREMENTAL=0 cargo clippy -p airlock --features testkit --tests --no-deps 2>&1 | tail -3` → no warnings.

```bash
git add crates/system/airlock
git commit -m "feat(airlock): real SEV-SNP chain verification + minted-chain testkit"
```

---

### Task 2: TDX real verify + vendored fixtures + `TrustRoots::from_env` test

**Files:**
- Modify: `crates/system/airlock/src/verify.rs`
- Create: `crates/system/airlock/tests/fixtures/tdx_quote` (vendored binary)
- Create: `crates/system/airlock/tests/fixtures/tdx_quote_collateral.json` (vendored)
- Create: `crates/system/airlock/tests/fixtures/README.md`
- Create: `crates/system/airlock/tests/verify_tdx.rs`

**Interfaces:**
- Consumes: `TrustRoots` (Task 1).
- Produces: `airlock::verify::verify_tdx_at(quote: &[u8], collateral: &dcap_qvl::QuoteCollateralV3, now_secs: u64, expected: &Measurement) -> Result<[u8; REPORT_DATA_LEN]>` — the pure, fixture-testable core; `verify_quote` TDX arm becomes real.

- [ ] **Step 1: Vendor the fixtures** (from the already-downloaded crate source — MIT-licensed Phala sample data):

```bash
mkdir -p crates/system/airlock/tests/fixtures
SCRATCH=/tmp/claude-1000/-home-eddy-dev-ducktape/eeec504e-b8d1-429b-8895-935b2bf3362d/scratchpad
cp $SCRATCH/dcap-qvl-0.3.12/sample/tdx_quote crates/system/airlock/tests/fixtures/
cp $SCRATCH/dcap-qvl-0.3.12/sample/tdx_quote_collateral.json crates/system/airlock/tests/fixtures/
```

`fixtures/README.md`: one paragraph — real Intel-signed TDX quote + PCS collateral vendored from the `dcap-qvl` 0.3.12 crate (MIT, Phala Network); verification is pinned to a timestamp inside the collateral's validity window (issue/nextUpdate intersection: 1750329147..1752920163).

- [ ] **Step 2: Failing test** — `tests/verify_tdx.rs`:

```rust
//! Real Intel-signed TDX quote verified offline: the full dcap-qvl chain
//! (PCK chain -> Intel root, TCB info, QE identity) runs against vendored
//! collateral at a pinned timestamp inside its validity window.
#![cfg(feature = "verify")]

use airlock::attest::Measurement;
use airlock::verify::verify_tdx_at;

const QUOTE: &[u8] = include_bytes!("fixtures/tdx_quote");
const COLLATERAL: &[u8] = include_bytes!("fixtures/tdx_quote_collateral.json");
/// Midpoint of the collateral's tcb_info/qe_identity validity intersection.
const NOW: u64 = 1751624655;

fn collateral() -> dcap_qvl::QuoteCollateralV3 {
    serde_json::from_slice(COLLATERAL).unwrap()
}

fn quote_mrtd() -> Measurement {
    // Pin the fixture's own MRTD by parsing the (not yet verified) quote.
    use dcap_qvl::quote::Quote;
    let q = Quote::parse(QUOTE).unwrap();
    Measurement(q.report.as_td10().unwrap().mr_td)
}

#[test]
fn real_tdx_quote_verifies_against_pinned_intel_roots() {
    let rd = verify_tdx_at(QUOTE, &collateral(), NOW, &quote_mrtd()).unwrap();
    assert_eq!(rd.len(), 64);
}

#[test]
fn wrong_mrtd_is_rejected() {
    let wrong = Measurement([0x22; airlock::attest::MRTD_LEN]);
    assert!(verify_tdx_at(QUOTE, &collateral(), NOW, &wrong).is_err());
}

#[test]
fn tampered_quote_is_rejected() {
    let mut quote = QUOTE.to_vec();
    let mid = quote.len() / 2;
    quote[mid] ^= 1;
    assert!(verify_tdx_at(&quote, &collateral(), NOW, &quote_mrtd()).is_err());
}

#[test]
fn expired_collateral_is_rejected() {
    // A `now` far past nextUpdate must fail closed.
    assert!(verify_tdx_at(QUOTE, &collateral(), NOW + 10 * 365 * 24 * 3600, &quote_mrtd()).is_err());
}
```

Run: `CARGO_INCREMENTAL=0 cargo test -p airlock --features verify --test verify_tdx 2>&1 | tail -5`
Expected: FAIL — `verify_tdx_at` not defined (and `dcap_qvl` not visible to tests: add `dcap-qvl` + `serde_json` to `[dev-dependencies]` if the compile demands it; serde_json is already there).

- [ ] **Step 3: Implement the TDX arm in `verify.rs`** (replacing the Task-1 stub):

```rust
// ===== Intel TDX ============================================================

async fn verify_tdx(
    quote: &[u8],
    expected: &Measurement,
    roots: &TdxRoots,
) -> Result<[u8; REPORT_DATA_LEN]> {
    let collateral = match &roots.pccs_url {
        Some(url) => dcap_qvl::collateral::get_collateral(url, quote).await,
        None => dcap_qvl::collateral::get_collateral_from_pcs(quote).await,
    }
    .map_err(|e| anyhow!("fetch TDX collateral: {e:?}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    verify_tdx_at(quote, &collateral, now, expected)
}

/// TCB statuses accepted as "the platform is trustworthy enough to release a
/// credential to". Anything else (OutOfDate, ConfigurationNeeded, Revoked, …)
/// fails closed with the status named.
const TDX_OK_STATUSES: &[&str] = &["UpToDate", "SWHardeningNeeded"];

/// Pure verification against provided collateral at a given time — the
/// fixture-testable core. Full dcap chain: PCK -> Intel root (pinned inside
/// dcap-qvl), TCB info, QE identity, quote signature.
pub fn verify_tdx_at(
    quote: &[u8],
    collateral: &dcap_qvl::QuoteCollateralV3,
    now_secs: u64,
    expected: &Measurement,
) -> Result<[u8; REPORT_DATA_LEN]> {
    let verified = dcap_qvl::verify::rustcrypto::verify(quote, collateral, now_secs)
        .map_err(|e| anyhow!("dcap verify: {e:?}"))?;
    if !TDX_OK_STATUSES.contains(&verified.status.as_str()) {
        bail!(
            "TDX TCB status {:?} not accepted (advisories: {:?})",
            verified.status,
            verified.advisory_ids
        );
    }
    let td = verified.report.as_td10().context("quote is not a TDX TD10 report")?;
    if td.mr_td != expected.0 {
        bail!(
            "MRTD mismatch: {} != expected {} (not the audited image)",
            hex::encode(td.mr_td),
            expected.to_hex()
        );
    }
    let mut rd = [0u8; REPORT_DATA_LEN];
    rd.copy_from_slice(&td.report_data[..REPORT_DATA_LEN]);
    Ok(rd)
}
```

Note: `Measurement` needs its tuple field public for the test's `Measurement(...)` construction — it already is (`pub struct Measurement(pub [u8; MRTD_LEN])`). `dcap-qvl` must be added to airlock `[dev-dependencies]` (non-optional) so the fixture test can name `QuoteCollateralV3`/`Quote` when only `--features verify` is active — same version spec as the optional real dependency.

- [ ] **Step 3b (best-effort): real AMD-signed SNP fixture.** Try to vendor a real SNP attestation report + matching VCEK from the virtee projects (`gh api` search `repos/virtee/snpguest/contents` and `repos/virtee/sev/contents/tests` for `*.bin`/report+cert test data). If found: add `tests/fixtures/snp_report.bin` + `snp_vcek.der` (+ provenance in fixtures README) and one test in `verify_snp.rs` verifying it with `SnpRoots::amd(<its product>, VcekSource::Der(...))` — green if the report's TCB certs chain, or asserting the precise named failure if the fixture is measurement-only. If no clean fixture pair exists upstream, SKIP and note in the fixtures README that real-silicon SNP coverage is task #26; the minted-chain tests already exercise parser + chain + signature.

- [ ] **Step 4: Green + `from_env` unit test**

Run: `CARGO_INCREMENTAL=0 cargo test -p airlock --features verify --test verify_tdx 2>&1 | tail -5` → 4 passed. (If `real_tdx_quote_verifies…` fails on a CRL time bound, nudge `NOW` toward 1750329147+1 day and update the fixtures README.)

`verify.rs` already carries a `#[cfg(test)] mod tests` covering `SnpProduct::from_str` (milan/genoa/turin/garbage). Env-shape tests belong to capability-host's `AirlockConfig` (Task 4), the single env boundary — the lib has no env surface to test.

Run: `CARGO_INCREMENTAL=0 cargo test -p airlock --features testkit 2>&1 | tail -3` → all green.

- [ ] **Step 5: Lint + commit**

```bash
CARGO_INCREMENTAL=0 cargo clippy -p airlock --features testkit --tests --no-deps 2>&1 | tail -3
git add crates/system/airlock
git commit -m "feat(airlock): real TDX verification with vendored Intel-signed fixture"
```

---

### Task 3: server `Quoter` seam + airlock e2e rewrite (custody path off mock)

**Files:**
- Modify: `crates/system/airlock/src/server.rs`
- Modify: `crates/system/airlock/src/testkit.rs`
- Modify: `crates/system/airlock/tests/e2e.rs`
- Modify: `crates/system/airlock/Cargo.toml` (dev-deps)

**Interfaces:**
- Produces: `airlock::server::Quoter = Box<dyn Fn(&[u8; REPORT_DATA_LEN]) -> anyhow::Result<Vec<u8>> + Send + Sync>`; `airlock::server::build_with_quoter(cfg: GatewayConfig, vendor: &str, quoter: Quoter) -> Result<(Router, String)>`; `airlock::testkit::SnpTestEnclave::quoter(&self) -> Quoter` (behind `server` feature).
- Consumes: `SnpTestEnclave` (Task 1), `verify_quote` (Tasks 1–2).

- [ ] **Step 1: Extract the seam in `server.rs`**

Split `build` so quote generation is a parameter; `build` keeps its exact
signature and behavior (mock branch INCLUDED for now — it dies in Task 7):

```rust
/// Quote generation, injected. Production uses configfs-tsm; tests inject a
/// testkit quoter. A process that injects a quoter already controls the
/// process — clients only trust what verifies against pinned roots.
pub type Quoter = Box<dyn Fn(&[u8; attest::REPORT_DATA_LEN]) -> Result<Vec<u8>> + Send + Sync>;

pub fn build(cfg: GatewayConfig) -> Result<(Router, String)> {
    // `auto` picks the vendor from the hardware; explicit mock|tdx|snp as named.
    // (mock branch unchanged here; deleted in the mock-removal task)
    ...existing mode resolution producing (mode, quoter-or-quote)...
    build_with_quoter(cfg, mode.as_str(), quoter)
}

/// Build the gateway with an injected quote generator. Mints the enclave keys,
/// calls `quoter` once on the freshly bound REPORTDATA.
pub fn build_with_quoter(cfg: GatewayConfig, vendor: &str, quoter: Quoter) -> Result<(Router, String)> {
    let seal_kp = SealKeypair::generate();
    let sess_sk = SigningKey::generate(&mut OsRng);
    let sess_pk = sess_sk.verifying_key();
    let seal_pk = seal_kp.public_bytes();
    let report_data = attest::make_report_data(&seal_pk, &sess_pk.to_bytes());
    let quote = quoter(&report_data)?;
    let vendor = vendor.to_string();
    // ...the existing AppState construction + Router, verbatim...
    Ok((app, vendor))
}
```

Concretely: `build`'s mode-resolution arms become closures — mock arm:
`Box::new(move |rd| Ok(attest::mock_quote(rd, &m)))`; tdx/snp/auto arm:
`Box::new(move |rd| tsm_gen_quote(mode_opt, rd).map(|(_, q)| q))` — note
`tsm_gen_quote` returns the detected mode, which `build` needs for `vendor`
when `attest == "auto"`; keep auto's probe by calling
`tsm_gen_quote(None, …)` FIRST in `build` to learn the mode, then pass a
quoter that reuses the quote it already generated
(`let q = quote.clone(); Box::new(move |_| Ok(q.clone()))` is WRONG — the
REPORTDATA isn't known yet). Correct shape: for auto, probe the provider file
only (`fs::read_to_string(...)/provider`) to learn the mode without generating,
then use the normal tsm quoter. Extract a small
`fn tsm_probe_provider() -> Result<AttestMode>` from `tsm_gen_quote` for this.

- [ ] **Step 2: `SnpTestEnclave::quoter`** in `testkit.rs`:

```rust
#[cfg(feature = "server")]
impl SnpTestEnclave {
    /// A server-injectable quoter minting quotes bound to the live REPORTDATA.
    /// The enclave must be shared (`Arc`) because the server keeps the closure.
    pub fn quoter(self: &std::sync::Arc<Self>) -> crate::server::Quoter {
        let enclave = self.clone();
        Box::new(move |rd| enclave.quote(rd))
    }
}
```

- [ ] **Step 3: Rewrite `tests/e2e.rs`** — replace the mock plumbing, keep every scenario:

- `boot_gateway` boots via the seam:

```rust
async fn boot_gateway(upstream: &str, enclave: &Arc<SnpTestEnclave>) -> String {
    let (app, vendor) = server::build_with_quoter(
        GatewayConfig {
            attest: "snp".into(),
            measurement: None,
            anthropic_base: upstream.into(),
            oauth_token_url: format!("{upstream}/oauth/token"),
            oauth_client_id: "test-client".into(),
            session_ttl_secs: 3600,
            max_requests: 100,
        },
        "snp",
        enclave.quoter(),
    )
    .unwrap();
    assert_eq!(vendor, "snp");
    spawn(app).await
}
```

- `attested_seal_pk` goes through the REAL verifier:

```rust
async fn attested_seal_pk(gw: &Gateway, enclave: &Arc<SnpTestEnclave>) -> [u8; 32] {
    let (quote, vendor) = gw.fetch_quote().await.unwrap();
    assert_eq!(vendor, "snp");
    let rd = airlock::verify::verify_quote(&quote, &measurement(), &enclave.roots()).await.unwrap();
    attest::split_report_data(&rd).0
}
```

with `fn measurement() -> Measurement { Measurement([0x11; attest::MRTD_LEN]) }` and each test constructing `let enclave = Arc::new(SnpTestEnclave::new(&measurement()).unwrap());` up front. All four tests (`full_custody_path…`, `proxy_rejects…`, `a_forged_gateway…`, `static_bearer…`) keep their assertions; `a_forged_gateway_cannot_mint_a_token_the_client_opens` keeps handshaking against `[0x42; 32]`.

- Update the e2e header comment and the required features:
  `#![cfg(all(feature = "server", feature = "client", feature = "testkit"))]`.
- Cargo `[dev-dependencies]` trick: a crate's integration tests can't enable its own features; the test simply no-ops (cfg'd out) unless invoked with `--features server,client,testkit`. That is the new documented invocation.

- [ ] **Step 4: Green**

Run: `CARGO_INCREMENTAL=0 cargo test -p airlock --features server,client,testkit 2>&1 | tail -5`
Expected: e2e 4 passed + verify_snp 5 + verify_tdx 4 + unit tests. (Mock unit tests in `attest.rs` still pass — untouched until Task 7.)

- [ ] **Step 5: Lint + commit**

```bash
CARGO_INCREMENTAL=0 cargo clippy -p airlock --features server,client,testkit --tests --no-deps 2>&1 | tail -3
git add crates/system/airlock
git commit -m "feat(airlock): quoter injection seam; custody e2e runs the real SNP verify path"
```

---

### Task 4: capability-host — real verify in `verify_gateway` + tests off mock

**Files:**
- Modify: `crates/system/capability-host/Cargo.toml`
- Modify: `crates/system/capability-host/src/broker.rs`

**Interfaces:**
- Consumes: `airlock::verify::{verify_quote, TrustRoots}`, `airlock::testkit::SnpTestEnclave`, `airlock::server::build_with_quoter`.
- Produces: broker env contract — `DUCKTAPE_AIRLOCK_ATTEST=tdx|snp` (mock still parses until Task 7), `DUCKTAPE_AIRLOCK_SNP_PRODUCT`, `DUCKTAPE_AIRLOCK_SNP_VCEK`, `DUCKTAPE_AIRLOCK_PCCS_URL`.

- [ ] **Step 1: Cargo wiring**

```toml
airlock = { path = "../airlock", features = ["client", "verify"] }
# dev-dependencies:
airlock = { path = "../airlock", features = ["client", "server", "verify", "testkit"] }
```

- [ ] **Step 2: Replace the refusal arm** in `verify_gateway` (`broker.rs` ~line 1090):

```rust
/// Fetch + verify the gateway quote and return the attested seal key, via the
/// real vendor verifier (`airlock::verify`) against pinned Intel/AMD roots.
async fn verify_gateway(
    gateway: &Gateway,
    mode: AttestMode,
    expected: &Measurement,
) -> Result<[u8; 32], String> {
    let (quote, _vendor) = gateway
        .fetch_quote()
        .await
        .map_err(|e| format!("airlock fetch quote: {e}"))?;
    let report_data = match mode {
        AttestMode::Mock => {
            attest::mock_verify(&quote, expected).map_err(|e| format!("airlock verify: {e}"))?
        }
        AttestMode::Tdx | AttestMode::Snp => {
            let roots = trust_roots(cfg, mode)?;
            airlock::verify::verify_quote(&quote, expected, &roots)
                .await
                .map_err(|e| format!("airlock verify: {e}"))?
        }
    };
    Ok(attest::split_report_data(&report_data).0)
}
```

(`verify_gateway` gains a `cfg: &AirlockConfig` parameter; its one call site in the
`AnthropicAuth` setup already holds the parsed config.)

```rust

/// Production: pinned roots assembled from the ALREADY-PARSED typed config.
/// Tests: an injected override (compiled OUT of non-test builds) so an
/// in-process test enclave is verified through the real verify path.
///
/// Config-boundary rule (user feedback): `AirlockConfig::from_env` is the ONE
/// place that reads `DUCKTAPE_AIRLOCK_*` env — it grows typed fields
/// `snp_product: Option<SnpProduct>` (parsed at config time),
/// `snp_vcek: Option<VcekSource>` (file READ at config time -> `VcekSource::Der`),
/// `pccs_url: Option<String>` — so misconfig fails at config time and
/// everything downstream is plain typed data.
fn trust_roots(cfg: &AirlockConfig, mode: AttestMode) -> Result<airlock::verify::TrustRoots, String> {
    use airlock::verify::{SnpRoots, TdxRoots, TrustRoots, VcekSource};
    #[cfg(test)]
    if let Some(roots) = test_trust_roots().lock().unwrap().clone() {
        return Ok(roots);
    }
    match mode {
        AttestMode::Tdx => Ok(TrustRoots::Tdx(TdxRoots { pccs_url: cfg.pccs_url.clone() })),
        AttestMode::Snp => {
            let product = cfg.snp_product.ok_or_else(|| {
                "airlock attest=snp requires DUCKTAPE_AIRLOCK_SNP_PRODUCT (milan|genoa|turin)"
                    .to_string()
            })?;
            let vcek = cfg.snp_vcek.clone().unwrap_or(VcekSource::Kds);
            SnpRoots::amd(product, vcek)
                .map(TrustRoots::Snp)
                .map_err(|e| format!("airlock SNP roots: {e}"))
        }
        AttestMode::Mock => Err("mock has no trust roots".into()),
    }
}

#[cfg(test)]
fn test_trust_roots() -> &'static std::sync::Mutex<Option<airlock::verify::TrustRoots>> {
    static ROOTS: std::sync::OnceLock<std::sync::Mutex<Option<airlock::verify::TrustRoots>>> =
        std::sync::OnceLock::new();
    ROOTS.get_or_init(|| std::sync::Mutex::new(None))
}
```

(The Mock arm survives until Task 7; the enum makes its removal a compile error there.)

- [ ] **Step 3: Rewrite the in-file airlock tests** (`broker.rs` ~line 2200+). Follow the existing tests' structure exactly, changing only the attestation layer:
  - Boot the in-process gateway with `server::build_with_quoter(cfg-with-attest-"snp", "snp", enclave.quoter())` instead of `attest: "mock"`.
  - Seal-side setup that used `attest::mock_verify(&quote, …)` to learn `seal_pk` now uses `attest::split_report_data(&airlock::verify::verify_quote(&quote, &expected, &enclave.roots()).await.unwrap()).0`.
  - Env: `DUCKTAPE_AIRLOCK_ATTEST=snp` (instead of `mock`), and set `test_trust_roots()` to `Some(enclave.roots())` at the start of each test that reaches `verify_gateway`, clearing it (`*lock = None`) at the end. These tests already serialize env mutation — reuse whatever lock/serial pattern the existing airlock tests in this file use; the roots override rides the same serialization.
  - The negative test that pins a DIFFERENT measurement ("22"×48) keeps working: the enclave mints for "11"×48; the real verifier rejects the mismatch.

- [ ] **Step 4: Green**

Run: `CARGO_INCREMENTAL=0 cargo test -p capability-host airlock 2>&1 | tail -5`
Expected: all airlock-scoped tests pass. Then the crate's full suite: `CARGO_INCREMENTAL=0 cargo test -p capability-host 2>&1 | tail -3`.

- [ ] **Step 5: Lint + commit**

```bash
CARGO_INCREMENTAL=0 cargo clippy -p capability-host --tests --no-deps 2>&1 | tail -3
git add crates/system/capability-host
git commit -m "feat(capability-host): wire real TDX/SNP verification into the airlock broker"
```

---

### Task 5: `airlock-broker` + `airlock-cli` on the shared verifier

**Files:**
- Modify: `bin/airlock-broker/src/main.rs`
- Modify: `bin/airlock-broker/Cargo.toml`
- Modify: `bin/airlock-cli/src/main.rs`
- Modify: `bin/airlock-cli/Cargo.toml`

**Interfaces:**
- Consumes: `airlock::verify::{verify_quote, TrustRoots, TdxRoots, SnpRoots, SnpProduct, VcekSource}`.
- Produces: CLI contract — `--attest` REQUIRED (`mock` still accepted until Task 7); new flags `--snp-product milan|genoa|turin` (required with `--attest snp`), `--snp-vcek <der-file>` (optional), `--pccs-url <url>` (optional, tdx).

- [ ] **Step 1: Cargo** — both bins: `airlock = { path = "…", features = ["client", "verify"] }`. Delete airlock-cli's `[features] tdx / snp` section and its `dcap-qvl` dependency.

- [ ] **Step 2: Shared flag → roots resolution** (duplicate the small fn in each bin — they are separate crates with separate arg helpers):

```rust
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
            Ok(TrustRoots::Snp(SnpRoots::amd(product, vcek)?))
        }
        AttestMode::Mock => unreachable!("mock handled by the caller until its removal"),
    }
}
```

- [ ] **Step 3: airlock-cli** — make it thin:
  - `fn attest_mode()` loses its default: `arg("--attest").context("--attest is required (tdx|snp|mock)")?.parse()`.
  - `verify_quote(mode, quote, expected)` becomes: Mock → `attest::mock_verify` (until Task 7); Tdx|Snp → `airlock::verify::verify_quote(quote, expected, &resolve_roots(mode)?).await`.
  - DELETE the entire `tdx`/`snp` sections at the bottom (`tdx_inspect`, `tdx_verify`, `snp_*` fns, both feature-gated and fallback variants).
  - `inspect_cmd`: Mock → `mock_peek` (until Task 7); Tdx → parse-without-verify via `dcap_qvl::quote::Quote::decode` is no longer available (dep dropped) — instead run the REAL verify (`resolve_roots` + `verify_quote`) and print the measurement mismatch error's embedded quote-MRTD… that inverts the TOFU flow. Keep it honest and simple: add to `airlock::verify` a `pub fn peek_measurement(mode: AttestMode, quote: &[u8]) -> Result<(String, [u8; REPORT_DATA_LEN])>` that structurally parses (dcap `Quote::parse` / snp `AttestationReport::decode`) WITHOUT signature verification, documented as TOFU-inspect-only, and use it for `inspect_cmd` (mock arm stays local until Task 7). The `--- pin the line below ---` stderr framing stays.

- [ ] **Step 4: airlock-broker** — `attest: arg_or("--attest", "mock")` → required (same `.context` as cli); the `Tdx | Snp` refusal arm in `attested_seal_pk` becomes the `airlock::verify::verify_quote(...)` call with `resolve_roots`.

- [ ] **Step 5: Build + smoke**

Run: `CARGO_INCREMENTAL=0 cargo build -p airlock-cli -p airlock-broker 2>&1 | tail -3` → success.
Smoke the flag contract: `./target/debug/airlock-cli run --host http://127.0.0.1:1 2>&1 | head -2` → errors with "--attest is required" BEFORE any connection attempt. And `./target/debug/airlock-cli run --attest snp --measurement $(python3 -c "print('11'*48)") --host http://127.0.0.1:1 2>&1 | head -2` → errors with "--snp-product" required.

- [ ] **Step 6: Lint + commit**

```bash
CARGO_INCREMENTAL=0 cargo clippy -p airlock-cli --tests --no-deps 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo clippy -p airlock-broker --tests --no-deps 2>&1 | tail -2
git add bin/airlock-cli bin/airlock-broker
git commit -m "feat(airlock-bins): real vendor verify via airlock::verify; --attest required"
```

---

### Task 6: node overlay e2e onto the real verify path

**Files:**
- Modify: `bin/node/Cargo.toml` (dev-dependency `airlock` gains `verify`, `testkit`)
- Modify: `bin/node/tests/airlock_gateway_e2e.rs`

**Interfaces:**
- Consumes: `SnpTestEnclave` (+ `.quoter()`, `.roots()`), `server::build_with_quoter`, `verify::verify_quote`.

- [ ] **Step 1:** dev-dep: `airlock = { path = "…", features = ["client", "server", "verify", "testkit"] }`.

- [ ] **Step 2:** Rewrite the attestation layer of the test (both the single-node self-serve test and the 2-node WG test share `boot gateway` + two `attest::mock_verify` call sites at ~lines 260 and 356):
  - Boot: `server::build_with_quoter(cfg-with-attest-"snp", "snp", enclave.quoter())`, `assert_eq!(vendor, "snp")`; the enclave is `Arc<SnpTestEnclave>` minted for the test's "11"×48 measurement and threaded to the client side.
  - Both `attest::split_report_data(&attest::mock_verify(&quote, &expected).unwrap()).0` sites become `attest::split_report_data(&airlock::verify::verify_quote(&quote, &expected, &enclave.roots()).await.unwrap()).0`.
  - Update the header comment (lines ~11, 239): the quote is now a minted SNP-format report verified by the REAL chain verifier under test roots; only silicon-backed generation remains hardware-TODO.

- [ ] **Step 3: Green** (the single-node lane; the 2-node lane is known-unreliable on this box):

Run: `CARGO_INCREMENTAL=0 cargo test -p node-bin --test airlock_gateway_e2e airlock_single_node_self_serves_its_own_route -- --nocapture 2>&1 | tail -5`
Expected: PASS (~20 s). Do NOT chase `airlock_over_gateway_two_wireguard_nodes` if its WG bring-up times out — that is the pre-existing dev-box limitation; confirm it still COMPILES (it builds as part of the test binary).

- [ ] **Step 4: Commit**

```bash
git add bin/node
git commit -m "test(node): airlock overlay e2e attests through the real SNP verify path"
```

---

### Task 7: delete mock — compiler-guided sweep

**Files:**
- Modify: `crates/system/airlock/src/attest.rs`
- Modify: `crates/system/airlock/src/server.rs`
- Modify: `crates/system/capability-host/src/broker.rs`
- Modify: `bin/airlock-cli/src/main.rs`
- Modify: `bin/airlock-broker/src/main.rs`

- [ ] **Step 1:** In `attest.rs`: delete the `Mock` variant, `MOCK_MAGIC`, `MOCK_ISSUER_SEED`, `mock_issuer`, `mock_quote`, `mock_peek`, `mock_verify`, the three mock unit tests (`mock_quote_verifies_and_binds_keys`, `wrong_measurement_rejected`, `tampered_report_data_rejected` — `report_data_split_is_inverse` stays), and the `ed25519_dalek` imports if now unused (`SigningKey` is still used elsewhere in the crate — check before removing the dependency itself; the seal/handshake/token modules use ed25519 — only trim the *imports in attest.rs*). Rewrite the module doc comment: attestation binds keys into REPORTDATA; generation is configfs-tsm in the server; verification is `airlock::verify`; the dev-box story is `testkit`.
- [ ] **Step 2:** `FromStr for AttestMode` drops "mock" (error message: `want 'tdx' or 'snp'`). `as_str` loses the arm.
- [ ] **Step 3:** Build the workspace and fix every error the enum change surfaces — expected sites: `server.rs::build` mock arm (delete; `GatewayConfig.attest` doc becomes "tdx | snp | auto"; `measurement: Option<String>` field — now unused by any arm — delete the field and fix the two e2e constructors and any node test constructing it); `broker.rs::verify_gateway` Mock arm (delete; `trust_roots` handles both remaining modes); its `DUCKTAPE_AIRLOCK_ATTEST` error copy (drop the mock sentence); `airlock-cli` Mock arms in `verify_quote`/`inspect_cmd` + `resolve_roots`'s `unreachable!` arm; `airlock-broker` Mock arm.

Run: `CARGO_INCREMENTAL=0 cargo build --workspace 2>&1 | grep -E "^error" | head` — iterate until empty. `rg -n "mock_verify|mock_quote|mock_peek|MOCK_|AttestMode::Mock|\"mock\"" crates/system/airlock crates/system/capability-host bin/airlock-cli bin/airlock-broker bin/airlock-gateway bin/node --type rust` → zero hits (except unrelated words like "mock upstream" in test comments, which are fine — the mock UPSTREAM is an HTTP stub, not attestation).

- [ ] **Step 4: Full test sweep**

```bash
CARGO_INCREMENTAL=0 cargo test -p airlock --features server,client,verify,testkit 2>&1 | tail -3
CARGO_INCREMENTAL=0 cargo test -p capability-host 2>&1 | tail -3
CARGO_INCREMENTAL=0 cargo test -p node-bin --test airlock_gateway_e2e airlock_single_node_self_serves_its_own_route 2>&1 | tail -3
```

All green.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(airlock)!: delete mock attestation — real TDX/SNP verification is the only path"
```

---

### Task 8: docs, gates, PR

**Files:**
- Modify: `crates/system/airlock/README.md`
- Modify: `docs/superpowers/specs/2026-07-18-execution-auth-separation-design.md`

- [ ] **Step 1: README** — rewrite the attestation-touching sections: `--attest mock` examples become `snp`/`tdx` with `--snp-product`; the "Per-vendor attestation" section describes real-only verify (dcap-qvl / AMD VCEK chain, both pinned, SNP fail-closed incl. VLEK refusal, TCB status policy) and the `testkit` story for non-TEE boxes; the remote-overlay recipe's `DUCKTAPE_AIRLOCK_ATTEST=mock` line becomes `snp` + `DUCKTAPE_AIRLOCK_SNP_PRODUCT` with a note that the credential node must now run on real silicon; the test invocation becomes `cargo test -p airlock --features server,client,verify,testkit`. Note `AIRLOCK_SNP_INSECURE_STRUCTURAL` no longer exists.
- [ ] **Step 2: exec/auth spec §graft** — update "Remaining": vendor verify is now wired (this plan's spec linked); remaining = SSE-over-overlay + hardware validation (task #26).
- [ ] **Step 3: Full gates**

```bash
CARGO_INCREMENTAL=0 cargo clippy -p airlock --features server,client,verify,testkit --tests --no-deps 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo clippy -p capability-host --tests --no-deps 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo clippy -p airlock-cli --tests --no-deps 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo clippy -p airlock-broker --tests --no-deps 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo build --workspace 2>&1 | tail -2
CARGO_INCREMENTAL=0 cargo check -p files --no-default-features 2>&1 | tail -2
```

- [ ] **Step 4: PR against dev** — title `feat(airlock): delete mock attestation; real TDX/SNP verification everywhere`; body: what mock was, the TrustRoots/testkit security story, TCB status policy, VLEK refusal, KDS cache, what cannot run on this box (silicon-backed generation → task #26), and the breaking env/flag contract (`--attest` required, `mock` gone, `DUCKTAPE_AIRLOCK_SNP_PRODUCT`). `gh pr create --base dev`.
- [ ] **Step 5:** Independent adversarial review from a clean context before any merge decision (repo rule), focusing on: fail-open paths in verify, the cfg(test) roots override not leaking into release builds, KDS/PCCS error handling, and the deleted `measurement` field's blast radius.
