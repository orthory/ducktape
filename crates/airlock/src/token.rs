//! Scoped session token: `base64url(claims_json).base64url(ed25519_sig)`.
//! Signed by the enclave's session key; the Computation Provider presents it
//! as the bearer on `/v1/messages`. It is NOT the credential — it is scoped and
//! expiring, and the host swaps it for the real access token upstream.

use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub iat: u64,
    pub exp: u64,
    pub max_requests: u32,
    /// base64url of the session's client ephemeral X25519 pk — the enclave
    /// re-derives the handshake keys from it statelessly per request.
    pub eph: String,
    /// Sealed-body session: the enclave REFUSES unsealed `/v1` bodies, so a
    /// stolen bearer alone (visible to path hosts) is useless.
    pub seal: bool,
}

pub fn issue(sess_sk: &SigningKey, claims: &Claims) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims serialize"));
    let sig = sess_sk.sign(payload.as_bytes());
    format!("{payload}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()))
}

/// Verify signature and parse claims. Expiry/budget are the caller's to check
/// (they need the current time / budget state).
pub fn verify(sess_pk: &VerifyingKey, token: &str) -> Result<Claims> {
    let (payload, sig_b64) = token.split_once('.').context("malformed token")?;
    let sig_bytes: [u8; 64] = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .context("token signature base64")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("token signature wrong length"))?;
    sess_pk
        .verify_strict(payload.as_bytes(), &Signature::from_bytes(&sig_bytes))
        .context("token signature invalid")?;
    let claims: Claims = serde_json::from_slice(
        &URL_SAFE_NO_PAD.decode(payload).context("token payload base64")?,
    )
    .context("token claims json")?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    fn claims() -> Claims {
        Claims {
            sub: "demo".into(),
            iat: 100,
            exp: 200,
            max_requests: 5,
            eph: "AAAA".into(),
            seal: true,
        }
    }

    #[test]
    fn issue_verify_round_trips() {
        let sk = SigningKey::generate(&mut OsRng);
        let tok = issue(&sk, &claims());
        assert_eq!(verify(&sk.verifying_key(), &tok).unwrap(), claims());
    }

    #[test]
    fn tampered_payload_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let tok = issue(&sk, &claims());
        let (_, sig) = tok.split_once('.').unwrap();
        let forged = format!(
            "{}.{sig}",
            URL_SAFE_NO_PAD.encode(
                br#"{"sub":"attacker","iat":100,"exp":200,"max_requests":999,"eph":"AAAA","seal":false}"#
            )
        );
        assert!(verify(&sk.verifying_key(), &forged).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let other = SigningKey::generate(&mut OsRng);
        let tok = issue(&sk, &claims());
        assert!(verify(&other.verifying_key(), &tok).is_err());
    }
}
