//! HTTP wire types between host and client. Plain HTTP for the PoC; the
//! transport drops onto duckdns + a gateway RouteRecord in the integration
//! spec.

use serde::{Deserialize, Serialize};

/// `GET /attestation` — the enclave quote. The seal + session pubkeys are read
/// out of the quote's verified REPORTDATA, never trusted from a JSON field.
#[derive(Serialize, Deserialize)]
pub struct AttestationResponse {
    pub quote_b64: String,
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
/// In the PoC the caller just names itself; on the mesh this is the overlay
/// AccountId.
#[derive(Serialize, Deserialize)]
pub struct SessionRequest {
    pub sub: String,
}

#[derive(Serialize, Deserialize)]
pub struct SessionResponse {
    pub token: String,
}
