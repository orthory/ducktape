# `auth.ducktape.byeongsu.dev` — the WebAuthn relying-party page

One static file, no backend (`index.html`). Gate: `node ops/auth-page/test.mjs`
— plain `node`, no dependencies, no install; it lifts the page's pure helper
block and checks the fragment parser, DER→raw and SPKI→compressed-SEC1. The app/CLI opens the system
browser to it with the request in the URL fragment, the page runs the
ceremony, and the result goes to a one-shot loopback listener in the app/CLI
(the `gh auth login` shape). Design: `docs/superpowers/specs/2026-08-27-identity-rework-design.md`
§WebAuthn; the verifier it must satisfy: `crates/kernel/keyscheme`.

The RP ID is the page's own host, never a parameter. The same file served by
the node at `http://localhost:<port>/.duck/auth` gives RP ID `localhost` —
fine for a platform authenticator on the same machine, no cross-device QR.

## Request — URL fragment

```
#op=create|get|eth&challenge=<b64url>&user=<b64url>&name=<urlencoded>&cb=<url>
```

All binary fields are base64url, no padding.

| param | ops | meaning |
|---|---|---|
| `challenge` | all | `create`/`get`: the 32 challenge bytes (`SHA-256(ns ‖ preimage)`, hashed by the client; passed straight through). `eth`: the exact `personal_sign` message bytes (`union_unique(ns, preimage)`, NOT hashed; the wallet prepends the EIP-191 prefix itself). |
| `user` | create | `user.id`: the account number as 8 bytes u64 LE. |
| `name` | create | `user.name` = `user.displayName`: the account's display name. |
| `cb` | all, optional | the listener URL. **Loopback only** (`http://127.0.0.1`, `[::1]`, `localhost`) — any other host is refused before the ceremony. Without it the page prints the result JSON (manual testing). |

Fixed options: `pubKeyCredParams` ES256 (-7) only; `residentKey: required`
(discoverable → usernameless QR login); `userVerification: preferred`; no
`authenticatorAttachment` (hybrid/cross-device stays allowed);
`attestation: none`. `get` always sends `allowCredentials: []`.

## Result — delivered to `cb`

A top-level **form POST**, `application/x-www-form-urlencoded`, one field
`result=<JSON>`. No CORS, no preflight, no Chrome local-network-access
prompt: navigations to loopback are exempt from all three, a `fetch` is not.
The listener's response body is what the user sees ("done, return to the
app"), then it closes.

```jsonc
// create — the client consumes publicKey + credentialId; the rest is for debugging
{"op":"create","credentialId":"…","publicKey":"<33-byte compressed SEC1>","alg":-7,
 "attestationObject":"…","clientDataJSON":"…"}

// get — feeds keyscheme's Secp256r1 envelope: authenticatorData ‖ clientDataJSON ‖ signature
{"op":"get","credentialId":"…","authenticatorData":"…","clientDataJSON":"…",
 "signature":"<64 raw R‖S>","userHandle":"<8-byte account number or null>"}

// eth — the client recovers the pubkey (k256 recover, one line); the page has no secp256k1
{"op":"eth","address":"0x…","signature":"0x<65 bytes r‖s‖v>","message":"<the bytes signed>"}

// any failure, so the listener can stop waiting
{"op":"…","error":"<DOMException name>","message":"…"}
```

## Ceremonies — how the clients sequence the ops

The client half is `crates/authpage` (the fragment URL, the loopback
listener, the result JSON, and the frame/consent builders); the CLI verbs
are `ducktape account key add --passkey|--eth`, `create --eth` and
`login` (`--no-browser` prints the URL for a headless box, `--auth-page`
points at another deployment); the app's Settings › Account card has the
same three buttons.

- **A passkey is registered in two ceremonies.** `create` yields the public
  key but a `webauthn.create` attestation is not a possession proof
  `keyscheme` accepts, so the client then asks for a `get` over the `AddKey`
  frame preimage — the passkey signs its own admission as the frame's
  origin.
- **A wallet is two touches.** A wallet never shows its public key, so touch
  1 signs `ducktape:reveal-key:v1` ‖ 16 random bytes and the client
  RECOVERS the key from the signature (`keyscheme::recover_personal_sign`);
  touch 2 signs the real preimage. Nothing on chain verifies the reveal
  signature — it authorizes nothing.
- **A login is one `get` with `allowCredentials: []`**: the discoverable
  passkey answers with its `userHandle` (the account number written at
  registration), and its assertion over the NEW device's `AddKey` preimage
  is the member consent that frame carries; the device signs the frame.

## Deploy

Cloudflare Workers static assets; the `custom_domain` route makes wrangler
create the DNS record and certificate in the zone.

```
npx wrangler@4 login                                       # once per machine (OAuth; headless: --browser=false, then curl the callback URL within 120 s)
npx wrangler@4 deploy --config ops/auth-page/wrangler.toml
node ops/auth-page/test.mjs                                # the pure helpers (fragment, DER→raw, SPKI→SEC1)
```
