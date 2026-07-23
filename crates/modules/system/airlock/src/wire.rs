//! HTTP wire types between host and client. Plain HTTP for the PoC; the
//! transport drops onto duckdns + a gateway RouteRecord in the integration
//! spec.

use serde::{Deserialize, Serialize};

/// `GET /attestation` — the enclave quote. The seal key used downstream is read
/// out of the quote's verified REPORTDATA, never trusted from a JSON field.
/// `vendor` ("tdx" | "snp") only selects which verifier the client
/// runs; it is not itself trusted — the quote's own format authenticates it.
#[derive(Serialize, Deserialize)]
pub struct AttestationResponse {
    pub quote_b64: String,
    pub vendor: String,
}

/// Which vendor a credential belongs to. Selects the upstream base + auth shape
/// the gateway proxies to (`Claude` → Anthropic, `Codex` → OpenAI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    Claude,
    Codex,
}

/// `POST /credential` — the sealed credential, encrypted to the enclave's seal
/// key. Only the enclave (inside the TD) can open it. `name`/`kind` are cleartext
/// routing metadata (never secret): the gateway keys a NAMED store by `name` and
/// picks the upstream by `kind`.
#[derive(Serialize, Deserialize)]
pub struct CredentialUpload {
    pub name: String,
    pub kind: CredentialKind,
    pub sealed_b64: String,
}

/// Plaintext inside the sealed blob: the upstream credential the enclave holds.
/// `Refresh` is exchanged via OAuth for an access token and ROTATES on each
/// refresh (subscription path). `Bearer` is a STATIC access token used as-is —
/// no refresh, no rotation — so sealing a live subscription's current access
/// token does not invalidate the token chain the owner is still using.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CredentialPayload {
    Refresh { refresh_token: String },
    Bearer { access_token: String },
}

/// `POST /session` — the Computation Provider asks for a scoped session token.
/// `sub` names the CREDENTIAL the session draws on (the on-chain credential
/// name); the gateway keys budgets, nonces, and upstream selection by it and
/// refuses an unknown name. `client_eph_pk_b64`
/// is this session's ephemeral X25519 public key: the enclave ECDHs it against
/// its static seal key to derive the shared session key (see `handshake`).
#[derive(Serialize, Deserialize)]
pub struct SessionRequest {
    pub sub: String,
    pub client_eph_pk_b64: String,
    /// Sealed-body session: bodies are AEAD'd broker<->enclave (`bodyseal`)
    /// and the enclave refuses plaintext. Echoed into the token claims.
    pub body_seal: bool,
    /// The account (base64) the caller CLAIMS to act on behalf of — the grant
    /// subject. Only load-bearing on a co-hosted lending gateway, where the node
    /// wires a grant lookup that checks this account against the on-chain
    /// credential record; a session claiming an ungranted account is refused
    /// (`credential_not_granted`). `None` on gateways with no grant lookup (the
    /// owner-local and TEE paths), where it is simply unread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_b64: Option<String>,
}

/// The token, AEAD-sealed under the handshake session key. Only the client that
/// derived the same key (from the attested `seal_pk`) can open it.
#[derive(Serialize, Deserialize)]
pub struct SessionResponse {
    pub sealed_token_b64: String,
}
