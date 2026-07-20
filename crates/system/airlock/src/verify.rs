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

use crate::attest::{Measurement, REPORT_DATA_LEN};

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

/// Which pinned trust anchor a quote must chain to. Plain data: this crate
/// never reads env or flags — each binary parses its own config ONCE at its
/// boundary into these typed values (capability-host's `AirlockConfig`, the
/// bins' flag parsing) and hands them in.
#[derive(Clone)]
pub enum TrustRoots {
    Tdx(TdxRoots),
    /// Boxed: a CA chain is ~1 KiB and would dominate the enum size.
    Snp(Box<SnpRoots>),
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

// ===== Intel TDX ============================================================

async fn verify_tdx(
    _quote: &[u8],
    _expected: &Measurement,
    _roots: &TdxRoots,
) -> Result<[u8; REPORT_DATA_LEN]> {
    bail!("tdx verify lands in task 2")
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
        bail!(
            "SEV-SNP report signed by a non-VCEK key (signing_key={signing_key}); \
             only VCEK is supported"
        );
    }

    let vcek_der = match &roots.vcek {
        VcekSource::Der(der) => der.clone(),
        VcekSource::Kds => fetch_vcek_kds(&report, roots.product).await?,
    };
    let vcek =
        Certificate::from_der(&vcek_der).map_err(|e| anyhow!("parse VCEK certificate: {e}"))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snp_product_parses_case_insensitively() {
        assert_eq!("milan".parse::<SnpProduct>().unwrap(), SnpProduct::Milan);
        assert_eq!("Genoa".parse::<SnpProduct>().unwrap(), SnpProduct::Genoa);
        assert_eq!("TURIN".parse::<SnpProduct>().unwrap(), SnpProduct::Turin);
        assert!("rome".parse::<SnpProduct>().is_err());
    }

}
