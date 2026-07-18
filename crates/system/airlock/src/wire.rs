//! HTTP wire types between host and client. Plain HTTP for the PoC; the
//! transport drops onto duckdns + a gateway RouteRecord in the integration
//! spec.

use serde::{Deserialize, Serialize};

/// `GET /attestation` — the enclave quote. The seal key used downstream is read
/// out of the quote's verified REPORTDATA, never trusted from a JSON field.
/// `vendor` ("mock" | "tdx" | "snp") only selects which verifier the client
/// runs; it is not itself trusted — the quote's own format authenticates it.
#[derive(Serialize, Deserialize)]
pub struct AttestationResponse {
    pub quote_b64: String,
    pub vendor: String,
}

/// `POST /credential` — the sealed refresh token, encrypted to the enclave's
/// seal key. Only the enclave (inside the TD) can open it.
#[derive(Serialize, Deserialize)]
pub struct CredentialUpload {
    pub sealed_b64: String,
}

/// Plaintext inside the sealed blob.
#[derive(Serialize, Deserialize)]
pub struct CredentialPayload {
    pub refresh_token: String,
}

/// `POST /session` — the Computation Provider asks for a scoped session token.
/// `sub` names the caller (on the mesh, the overlay AccountId). `client_eph_pk_b64`
/// is this session's ephemeral X25519 public key: the enclave ECDHs it against
/// its static seal key to derive the shared session key (see `handshake`).
#[derive(Serialize, Deserialize)]
pub struct SessionRequest {
    pub sub: String,
    pub client_eph_pk_b64: String,
}

/// The token, AEAD-sealed under the handshake session key. Only the client that
/// derived the same key (from the attested `seal_pk`) can open it.
#[derive(Serialize, Deserialize)]
pub struct SessionResponse {
    pub sealed_token_b64: String,
}
