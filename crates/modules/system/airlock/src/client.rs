//! The async HTTP client side of the gateway protocol, shared by `airlock-cli`
//! (the CLI roles) and `airlock-broker` (the Computation Provider's local
//! api-snatch). Topology-agnostic: a `Gateway` is either LOCAL (same-machine
//! loopback) or REMOTE (a duckdns handle routed by the local node's
//! browser-gateway, carried in `x-duck-authority`).
//!
//! Vendor-specific quote VERIFICATION stays in the binaries (it needs
//! `dcap-qvl` for TDX etc.); this module owns only the transport + the ECDH
//! handshake, which take an already-verified `seal_pk`.

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::handshake;
use crate::seal;
use crate::wire::{
    AttestationResponse, CredentialKind, CredentialPayload, CredentialUpload, SessionRequest,
    SessionResponse,
};

/// Topology-agnostic handle to a gateway.
pub struct Gateway {
    base: String,
    /// `Some(authority)` on the remote path — sent as `x-duck-authority` so the
    /// local node's browser-gateway routes the request onto the overlay.
    authority: Option<String>,
    http: reqwest::Client,
}

impl Gateway {
    /// Local: Credential Provider == Computation Provider (same-machine loopback).
    pub fn local(host: String) -> Self {
        Self { base: host, authority: None, http: reqwest::Client::new() }
    }

    /// Remote: reach `handle` (a duckdns name) through `via` (the local node's
    /// browser-gateway base URL), which routes it onto the overlay.
    pub fn remote(handle: String, via: String) -> Self {
        Self { base: via, authority: Some(handle), http: reqwest::Client::new() }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    /// Add the overlay-routing header on the remote path; a no-op locally.
    pub fn route(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.authority {
            Some(a) => rb.header("x-duck-authority", a),
            None => rb,
        }
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Fetch the enclave quote and the advertised vendor. The caller verifies
    /// the quote per vendor before trusting anything read out of it.
    pub async fn fetch_quote(&self) -> Result<(Vec<u8>, String)> {
        let att: AttestationResponse = self
            .route(self.http.get(self.url("/attestation")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let quote = BASE64.decode(&att.quote_b64).context("quote base64")?;
        Ok((quote, att.vendor))
    }

    /// Run the session-key handshake against an ALREADY-VERIFIED `seal_pk` and
    /// return the scoped session token. ECDHs the attested key, so the token can
    /// only be opened by this client — a relaying node cannot read it.
    pub async fn open_session(&self, seal_pk: &[u8; 32], sub: &str) -> Result<String> {
        let (token, _keys) = self.open_session_with(seal_pk, sub, false).await?;
        Ok(token)
    }

    /// Sealed-body session: bodies must be AEAD'd under the returned keys
    /// (`bodyseal`); the enclave refuses plaintext on this token.
    pub async fn open_session_sealed(
        &self,
        seal_pk: &[u8; 32],
        sub: &str,
    ) -> Result<(String, handshake::SessionKeys)> {
        self.open_session_with(seal_pk, sub, true).await
    }

    async fn open_session_with(
        &self,
        seal_pk: &[u8; 32],
        sub: &str,
        body_seal: bool,
    ) -> Result<(String, handshake::SessionKeys)> {
        let (client_eph_pk, keys) = handshake::client_handshake(seal_pk);
        let resp: SessionResponse = self
            .route(self.http.post(self.url("/session")))
            .json(&SessionRequest {
                sub: sub.to_string(),
                client_eph_pk_b64: BASE64.encode(client_eph_pk),
                body_seal,
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let sealed = BASE64.decode(&resp.sealed_token_b64).context("sealed token base64")?;
        let token = handshake::open_token(&keys.session, &sealed).context(
            "open session token (handshake key mismatch — quote not from the real enclave?)",
        )?;
        Ok((String::from_utf8(token).context("session token was not utf-8")?, keys))
    }

    /// Seal `payload` (a refresh token or a static bearer) to the ALREADY-VERIFIED
    /// `seal_pk` and upload it under the credential `name`/`kind`. The gateway
    /// never sees the secret in the clear; the name/kind are cleartext routing.
    pub async fn upload_sealed_credential(
        &self,
        seal_pk: &[u8; 32],
        name: &str,
        kind: CredentialKind,
        payload: &CredentialPayload,
    ) -> Result<()> {
        let payload = serde_json::to_vec(payload)?;
        let sealed = seal::seal(seal_pk, &payload);
        let status = self
            .route(self.http.post(self.url("/credential")))
            .json(&CredentialUpload {
                name: name.to_string(),
                kind,
                sealed_b64: BASE64.encode(sealed),
            })
            .send()
            .await?
            .status();
        if !status.is_success() {
            anyhow::bail!("credential upload failed: {status}");
        }
        Ok(())
    }
}
