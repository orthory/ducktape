// Passkey enrollment — the phone-side WebAuthn ceremony and the pure parsers
// that turn its output into the shape the chain verifies.
//
// The phone (any OS, real browser) runs the ceremony; the desktop only ever
// shows a QR. Flow: register() mints the credential and yields its P-256 public
// key (`new_key`); the node computes the enrollment challenge over that key
// (see the `user-webauthn-challenge` verb — the preimage math lives in the
// node, never here); get() signs that challenge; the assertion becomes a
// `MemberProof::Webauthn` handed to `user-sign-add-member`.
//
// The on-chain verifier (crates/system/identity/src/scheme.rs) is DER-free and
// takes a raw SEC1 public key + raw R‖S signature, so the two parsers here —
// `spkiToRawP256Point` and `derEcdsaToRaw` — are the load-bearing, unit-tested
// bridge between WebAuthn's DER/SPKI world and the chain's raw-bytes world. Get
// them wrong and an enrolled passkey signs something the chain rejects.

// ── Wire types (MemberProof::Webauthn, verbatim serde) ───

/** `MemberProof::Webauthn` on the wire — external-tagged, snake_case, byte
 *  arrays. Fed to `user_sign_add_member(..., possession)`. */
export interface WebauthnProof {
  webauthn: {
    authenticator_data: number[];
    client_data_json: number[];
    signature: number[];
  };
}

export type EnrollResult =
  | { success: true; newKeyHex: string; proof: WebauthnProof }
  | { success: false; error: string };

// ── Byte helpers ─────────────────────────────────────────

const bytes = (buf: ArrayBuffer): Uint8Array => new Uint8Array(buf);

const toHex = (b: Uint8Array): string =>
  Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");

const toNums = (b: Uint8Array): number[] => Array.from(b);

/** base64url (no pad) → bytes — the node emits the challenge this way. */
export const b64urlToBytes = (s: string): Uint8Array => {
  const pad = s.length % 4 === 0 ? "" : "=".repeat(4 - (s.length % 4));
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + pad;
  const bin = atob(b64);
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
};

// ── SPKI → raw P-256 point ───────────────────────────────

/** The fixed DER prefix of a P-256 SubjectPublicKeyInfo, up to and including
 *  the BIT STRING's unused-bits byte — everything before the 65-byte
 *  uncompressed point. A registration response's `getPublicKey()` returns this
 *  exact structure for an ES256 credential. */
const P256_SPKI_PREFIX = "3059301306072a8648ce3d020106082a8648ce3d030107034200";

/** Extract the 65-byte uncompressed SEC1 point (`0x04 ‖ X ‖ Y`) from a P-256
 *  SPKI public key. Validates the algorithm prefix rather than blindly slicing,
 *  so a non-P256 / malformed key is a loud error, not silent wrong bytes. The
 *  chain stores and challenges over exactly these bytes as `new_key`. */
export const spkiToRawP256Point = (spki: Uint8Array): Uint8Array => {
  const prefix = toHex(spki.subarray(0, 26));
  if (prefix !== P256_SPKI_PREFIX) {
    throw new Error("public key is not a P-256 (ES256) SPKI");
  }
  const point = spki.subarray(26);
  if (point.length !== 65 || point[0] !== 0x04) {
    throw new Error("malformed uncompressed P-256 point");
  }
  return point;
};

// ── DER ECDSA signature → raw R‖S ────────────────────────

/** left-pad (or left-trim a single leading zero pad) a DER integer to a fixed
 *  32-byte P-256 scalar. */
const scalar32 = (raw: Uint8Array): Uint8Array => {
  // strip DER's sign-padding zero(s); reject anything that can't fit 32 bytes.
  const trimmed = raw[0] === 0x00 ? raw.subarray(1) : raw;
  if (trimmed.length > 32) throw new Error("ECDSA scalar exceeds 32 bytes");
  const out = new Uint8Array(32);
  out.set(trimmed, 32 - trimmed.length);
  return out;
};

/** Convert an ASN.1/DER ECDSA signature (`SEQUENCE { INTEGER r, INTEGER s }`)
 *  into the chain's raw 64-byte `R‖S`. WebAuthn assertions are DER; the chain
 *  verifier is raw — this is that normalization, done here so the node/chain
 *  stay DER-free.
 *
 *  Imperative walk over the TLV structure — a parser reads clearest as a cursor
 *  advance, not a fold. */
export const derEcdsaToRaw = (der: Uint8Array): Uint8Array => {
  if (der[0] !== 0x30) throw new Error("DER signature is not a SEQUENCE");
  // sequence length byte(s): short form only (an ECDSA-P256 sig is < 128 bytes).
  let cursor = der[1] & 0x80 ? 2 + (der[1] & 0x7f) : 2;
  const readInt = (): Uint8Array => {
    if (der[cursor] !== 0x02) throw new Error("DER signature INTEGER expected");
    const len = der[cursor + 1];
    const start = cursor + 2;
    const value = der.subarray(start, start + len);
    cursor = start + len;
    return value;
  };
  const r = scalar32(readInt());
  const s = scalar32(readInt());
  const raw = new Uint8Array(64);
  raw.set(r, 0);
  raw.set(s, 32);
  return raw;
};

// ── Proof assembly ───────────────────────────────────────

/** Assemble a `MemberProof::Webauthn` from a `get()` assertion's three parts.
 *  `signature` is DER (as the browser returns it) and is normalized to raw
 *  R‖S here. */
export const assembleWebauthnProof = (assertion: {
  authenticatorData: ArrayBuffer;
  clientDataJSON: ArrayBuffer;
  signature: ArrayBuffer;
}): WebauthnProof => ({
  webauthn: {
    authenticator_data: toNums(bytes(assertion.authenticatorData)),
    client_data_json: toNums(bytes(assertion.clientDataJSON)),
    signature: toNums(derEcdsaToRaw(bytes(assertion.signature))),
  },
});

// ── Ceremony orchestration (browser-only; the phone runs this) ──
//
// Thin wrappers over `navigator.credentials`; they cannot be unit-tested (no
// authenticator in CI), so all the fallible parsing lives in the pure helpers
// above. `getChallenge` is injected so the caller wires it to the node's
// `user-webauthn-challenge` — this module never reconstructs the preimage.

/** register() → the new passkey's raw P-256 public key (hex), plus the opaque
 *  credential id the follow-up get() must scope to. */
export const registerPasskey = (opts: {
  rpId: string;
  rpName: string;
  userName: string;
}): Promise<{ newKeyHex: string; credentialId: ArrayBuffer }> =>
  Promise.resolve()
    .then(() =>
      navigator.credentials.create({
        publicKey: {
          rp: { id: opts.rpId, name: opts.rpName },
          user: {
            id: crypto.getRandomValues(new Uint8Array(16)),
            name: opts.userName,
            displayName: opts.userName,
          },
          // register challenge is not the binding one — the binding challenge
          // is signed by the follow-up get() over the node's value.
          challenge: crypto.getRandomValues(new Uint8Array(32)),
          pubKeyCredParams: [{ type: "public-key", alg: -7 }],
          authenticatorSelection: { userVerification: "preferred" },
          attestation: "none",
        },
      }),
    )
    .then((cred) => {
      const c = cred as PublicKeyCredential;
      const response = c.response as AuthenticatorAttestationResponse;
      const spki = response.getPublicKey();
      if (!spki) throw new Error("authenticator did not return a public key");
      return {
        newKeyHex: toHex(spkiToRawP256Point(bytes(spki))),
        credentialId: c.rawId,
      };
    });

/** get() over the node's enrollment challenge → the possession `MemberProof`. */
export const assertPossession = (opts: {
  rpId: string;
  credentialId: ArrayBuffer;
  challenge: Uint8Array;
}): Promise<WebauthnProof> =>
  Promise.resolve()
    .then(() =>
      navigator.credentials.get({
        publicKey: {
          rpId: opts.rpId,
          // copy into a fresh ArrayBuffer-backed view: TS 5.7's Uint8Array is
          // generic over ArrayBufferLike, but BufferSource wants an
          // ArrayBuffer-backed one — the copy satisfies it without an unsafe cast.
          challenge: new Uint8Array(opts.challenge),
          allowCredentials: [{ type: "public-key", id: opts.credentialId }],
          userVerification: "preferred",
        },
      }),
    )
    .then((cred) => {
      const response = (cred as PublicKeyCredential)
        .response as AuthenticatorAssertionResponse;
      return assembleWebauthnProof(response);
    });
