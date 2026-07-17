//! Attestation: bind the enclave's seal + session public keys into a quote's
//! REPORTDATA, and let the client verify the quote before releasing a
//! credential.
//!
//! Mock mode lives here (runs anywhere — the dev box is a Ryzen 5950X with no
//! SEV/TDX). Real quote generation is vendor-generic via `configfs-tsm` (in
//! `tcg-host`): Intel TDX (`tdx_guest`) and AMD SEV-SNP (`sev_guest`) share the
//! sysfs report path. Verification is vendor-SPECIFIC and lives in `tcg-client`
//! behind feature flags (`dcap-qvl` for TDX; the AMD VCEK/KDS chain for SNP),
//! since it needs async + network + heavy deps that the mock path must not pull.

use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey};

pub const REPORT_DATA_LEN: usize = 64;
pub const MRTD_LEN: usize = 48; // TDX MRTD is SHA-384

/// The expected launch measurement (MRTD) of the audited host image. In the
/// PoC it is a CLI flag; production pins it on Ducktape consensus.
#[derive(Clone, Copy)]
pub struct Measurement(pub [u8; MRTD_LEN]);

impl Measurement {
    pub fn from_hex(s: &str) -> Result<Self> {
        let v = hex::decode(s.trim()).context("measurement is not valid hex")?;
        if v.len() != MRTD_LEN {
            bail!(
                "measurement must be {MRTD_LEN} bytes ({} hex chars), got {}",
                MRTD_LEN * 2,
                v.len()
            );
        }
        let mut m = [0u8; MRTD_LEN];
        m.copy_from_slice(&v);
        Ok(Measurement(m))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// The attestation vendor. `Mock` runs anywhere; `Tdx`/`Snp` need the matching
/// confidential-VM silicon. `auto` (a CLI-only value) resolves to `Tdx`/`Snp`
/// by probing `configfs-tsm`'s `provider`, so it is not a variant here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttestMode {
    Mock,
    Tdx,
    Snp,
}

impl AttestMode {
    /// The `configfs-tsm` `provider` string for this vendor, or `None` for mock.
    pub fn tsm_provider(self) -> Option<&'static str> {
        match self {
            Self::Mock => None,
            Self::Tdx => Some("tdx_guest"),
            Self::Snp => Some("sev_guest"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Tdx => "tdx",
            Self::Snp => "snp",
        }
    }
}

impl std::str::FromStr for AttestMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mock" => Ok(Self::Mock),
            "tdx" => Ok(Self::Tdx),
            "snp" => Ok(Self::Snp),
            other => bail!("unknown attest mode {other:?} (want 'mock', 'tdx', or 'snp')"),
        }
    }
}

/// REPORTDATA is exactly the two pubkeys concatenated (32 + 32 = 64), so the
/// quote *is* the key binding — no hashing, no trusting a side-channel field.
pub fn make_report_data(seal_pk: &[u8; 32], sess_pk: &[u8; 32]) -> [u8; REPORT_DATA_LEN] {
    let mut rd = [0u8; REPORT_DATA_LEN];
    rd[..32].copy_from_slice(seal_pk);
    rd[32..].copy_from_slice(sess_pk);
    rd
}

pub fn split_report_data(rd: &[u8; REPORT_DATA_LEN]) -> ([u8; 32], [u8; 32]) {
    let mut seal_pk = [0u8; 32];
    let mut sess_pk = [0u8; 32];
    seal_pk.copy_from_slice(&rd[..32]);
    sess_pk.copy_from_slice(&rd[32..]);
    (seal_pk, sess_pk)
}

// ---------------------------------------------------------------------------
// Mock attestation: a fake quote signed by a well-known issuer key, verifiable
// on any box. Layout: MAGIC(4) || mrtd(48) || report_data(64) || sig(64).
// ---------------------------------------------------------------------------

const MOCK_MAGIC: &[u8; 4] = b"MOCK";
const MOCK_ISSUER_SEED: [u8; 32] = [7u8; 32];

fn mock_issuer() -> SigningKey {
    SigningKey::from_bytes(&MOCK_ISSUER_SEED)
}

pub fn mock_quote(report_data: &[u8; REPORT_DATA_LEN], m: &Measurement) -> Vec<u8> {
    let mut signed = Vec::with_capacity(MRTD_LEN + REPORT_DATA_LEN);
    signed.extend_from_slice(&m.0);
    signed.extend_from_slice(report_data);
    let sig = mock_issuer().sign(&signed);

    let mut q = Vec::with_capacity(4 + MRTD_LEN + REPORT_DATA_LEN + 64);
    q.extend_from_slice(MOCK_MAGIC);
    q.extend_from_slice(&m.0);
    q.extend_from_slice(report_data);
    q.extend_from_slice(&sig.to_bytes());
    q
}

/// Parse + authenticate a mock quote WITHOUT comparing the measurement. Used by
/// `inspect` to read the embedded MRTD out of a quote. Still checks the issuer
/// signature so a garbage blob is rejected.
pub fn mock_peek(quote: &[u8]) -> Result<(Measurement, [u8; REPORT_DATA_LEN])> {
    let need = 4 + MRTD_LEN + REPORT_DATA_LEN + 64;
    if quote.len() != need {
        bail!("mock quote wrong length: {} != {need}", quote.len());
    }
    if &quote[..4] != MOCK_MAGIC {
        bail!("not a mock quote (bad magic)");
    }
    let mrtd = &quote[4..4 + MRTD_LEN];
    let rd = &quote[4 + MRTD_LEN..4 + MRTD_LEN + REPORT_DATA_LEN];
    let sig_bytes: [u8; 64] = quote[4 + MRTD_LEN + REPORT_DATA_LEN..].try_into().unwrap();

    let mut signed = Vec::with_capacity(MRTD_LEN + REPORT_DATA_LEN);
    signed.extend_from_slice(mrtd);
    signed.extend_from_slice(rd);
    mock_issuer()
        .verifying_key()
        .verify_strict(&signed, &Signature::from_bytes(&sig_bytes))
        .context("mock quote signature invalid")?;

    let mut m = [0u8; MRTD_LEN];
    m.copy_from_slice(mrtd);
    let mut out = [0u8; REPORT_DATA_LEN];
    out.copy_from_slice(rd);
    Ok((Measurement(m), out))
}

/// Verify a mock quote: issuer signature valid AND measurement matches the
/// expected audited image. Returns the bound REPORTDATA.
pub fn mock_verify(quote: &[u8], expected: &Measurement) -> Result<[u8; REPORT_DATA_LEN]> {
    let (m, rd) = mock_peek(quote)?;
    if m.0 != expected.0 {
        bail!(
            "measurement mismatch: quote {} != expected {} (not the audited image)",
            m.to_hex(),
            expected.to_hex()
        );
    }
    Ok(rd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meas(b: u8) -> Measurement {
        Measurement([b; MRTD_LEN])
    }

    #[test]
    fn report_data_split_is_inverse() {
        let seal = [1u8; 32];
        let sess = [2u8; 32];
        let rd = make_report_data(&seal, &sess);
        assert_eq!(split_report_data(&rd), (seal, sess));
    }

    #[test]
    fn mock_quote_verifies_and_binds_keys() {
        let seal = [9u8; 32];
        let sess = [8u8; 32];
        let rd = make_report_data(&seal, &sess);
        let q = mock_quote(&rd, &meas(0x11));
        let verified = mock_verify(&q, &meas(0x11)).unwrap();
        assert_eq!(split_report_data(&verified), (seal, sess));
    }

    #[test]
    fn wrong_measurement_rejected() {
        let rd = make_report_data(&[9u8; 32], &[8u8; 32]);
        let q = mock_quote(&rd, &meas(0x11));
        assert!(mock_verify(&q, &meas(0x22)).is_err());
    }

    #[test]
    fn tampered_report_data_rejected() {
        let rd = make_report_data(&[9u8; 32], &[8u8; 32]);
        let mut q = mock_quote(&rd, &meas(0x11));
        q[4 + MRTD_LEN] ^= 0x01; // flip a REPORTDATA bit
        assert!(mock_verify(&q, &meas(0x11)).is_err());
    }
}
