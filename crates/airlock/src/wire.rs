//! HTTP wire types between host and client. Plain HTTP for the PoC; the
//! transport drops onto duckdns + a gateway RouteRecord in the integration
//! spec.
//!
//! Every type here is STRICT: `deny_unknown_fields`, and no `serde(default)`
//! anywhere. The protocol decodes each frame at its boundary, so a producer
//! that is out of step gets a named decode error instead of a silently
//! defaulted field.
//!
//! Nothing here carries an ACCOUNT. A session request names the credential and
//! the session's ephemeral key, and says nothing about who is acting: identity
//! enters the flow at exactly one place, the gateway hop, where the node stamps
//! a mesh-verified caller the request cannot mint. A field for the caller to
//! declare its own account was the whole of the credential-theft defect.

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
///
/// There is deliberately NO account field. The grant subject is the account the
/// node's gateway proxy vouched for on the hop
/// ([`crate::server::CALLER_ACCOUNT_HEADER`]), which is minted from the
/// mesh-verified peer identity and refused if a caller supplies one. A request
/// cannot name a subject: the computation layer does not get to say who it acts
/// for, and the token it ends up holding carries no identity either — only
/// `sub`, which credential.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub sub: String,
    pub client_eph_pk_b64: String,
    /// Sealed-body session: bodies are AEAD'd broker<->enclave (`bodyseal`)
    /// and the enclave refuses plaintext. Echoed into the token claims.
    pub body_seal: bool,
}

/// The token, AEAD-sealed under the handshake session key. Only the client that
/// derived the same key (from the attested `seal_pk`) can open it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionResponse {
    pub sealed_token_b64: String,
}
