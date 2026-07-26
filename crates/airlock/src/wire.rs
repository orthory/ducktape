//! HTTP wire types between host and client. Plain HTTP for the PoC; the
//! transport drops onto duckdns + a gateway RouteRecord in the integration
//! spec.
//!
//! Every type here is STRICT: `deny_unknown_fields`, and no `serde(default)`
//! anywhere. The protocol decodes each frame at its boundary, so a producer
//! that is out of step gets a named decode error instead of a silently
//! defaulted field. That matters most on [`SessionRequest::account_b64`]: it
//! used to default to `None`, so a client typo became a 403
//! `credential_not_granted` — sending the borrower's operator to add a grant
//! that already existed, which is the exact misdiagnosis the three-state grant
//! taxonomy exists to prevent.

use serde::{Deserialize, Serialize};

/// `GET /attestation` — the enclave quote. The seal key used downstream is read
/// out of the quote's verified REPORTDATA, never trusted from a JSON field.
/// `vendor` ("tdx" | "snp") only selects which verifier the client
/// runs; it is not itself trusted — the quote's own format authenticates it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationResponse {
    pub quote_b64: String,
    pub vendor: String,
}

/// Which vendor a credential belongs to. Selects the upstream base + auth shape
/// the gateway proxies to (`Claude` → Anthropic, `Codex` → OpenAI) unless the
/// operator redirected one with `DUCKTAPE_AIRLOCK_{ANTHROPIC,OPENAI}_BASE`.
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
///
/// Served ONLY by the attested build — see [`crate::server::CredentialUploads`].
/// Sealing is not authentication: the seal public key is published on chain and
/// served at `/attestation`, so anyone can produce a well-formed upload.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialUpload {
    pub name: String,
    pub kind: CredentialKind,
    pub sealed_b64: String,
}

/// Plaintext inside the sealed blob: the upstream credential the enclave holds.
/// `Refresh` carries the subscription's CURRENT access token alongside the
/// rotating refresh token: the enclave serves the access token as-is until
/// `expires_at`, then exchanges the refresh token for a new one. Seeding the
/// live access token means NO refresh fires while it is still valid — so the
/// owner's own local login (which shares the refresh-token chain) is not
/// rotation-invalidated during that window. An empty `access_token` with
/// `expires_at` 0 is the lazy form (refresh on first use), and every producer
/// writes both fields EXPLICITLY — an omitted one is a producer out of step, not
/// a lazy credential. `Bearer` is a STATIC access token used as-is — no refresh,
/// no rotation.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialPayload {
    Refresh {
        refresh_token: String,
        access_token: String,
        expires_at: u64,
    },
    Bearer {
        access_token: String,
    },
}

/// `POST /session` — the Computation Provider asks for a scoped session token.
/// `sub` names the CREDENTIAL the session draws on (the on-chain credential
/// name); the gateway keys budgets, nonces, and upstream selection by it and
/// refuses an unknown name. `client_eph_pk_b64`
/// is this session's ephemeral X25519 public key: the enclave ECDHs it against
/// its static seal key to derive the shared session key (see `handshake`).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub sub: String,
    pub client_eph_pk_b64: String,
    /// Sealed-body session: bodies are AEAD'd broker<->enclave (`bodyseal`)
    /// and the enclave refuses plaintext. Echoed into the token claims.
    pub body_seal: bool,
    /// The account (base64) the caller claims to act on behalf of — the grant
    /// subject. Load-bearing only on a co-hosted lending gateway, and even there
    /// it is a CROSS-CHECK, never the authorization input: it must equal the
    /// account the node's proxy vouched for in `x-duck-caller-account`, or the
    /// session is refused (`account_mismatch`). `null` on gateways with no grant
    /// lookup (the owner-local and TEE paths), where it is simply unread — but
    /// the field is always PRESENT on the wire, so an omission is a decode error
    /// rather than a silent `None`.
    ///
    /// `deny_unknown_fields` alone does not get that: serde lets an `Option`
    /// field go missing even with no `#[serde(default)]`, and absent would decode
    /// to `None` — which on a gated gateway comes out as a 403 about a grant.
    /// Naming a deserializer suppresses that fallback, so an omission is a
    /// `missing field` error like every other.
    #[serde(deserialize_with = "Option::deserialize")]
    pub account_b64: Option<String>,
}

/// The token, AEAD-sealed under the handshake session key. Only the client that
/// derived the same key (from the attested `seal_pk`) can open it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionResponse {
    pub sealed_token_b64: String,
}
