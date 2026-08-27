# Identity rework phase 5 — WebAuthn (passkeys, QR login, wallet link) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A passkey or an Ethereum wallet becomes a member key of an account from the CLI and the app, and a new device joins an account by scanning a QR — all through the live RP page `https://auth.ducktape.byeongsu.dev/` (contract: `ops/auth-page/README.md`).

**Architecture:** One new pure library, `crates/authpage`, owns the page contract from the client side: the fragment URL, the one-shot loopback listener that receives the page's form POST, the result JSON, and the three ceremony builders (passkey-origin frame, wallet-origin frame, QR-login `AddKey`). It does no node I/O — `node` queries (`KeyGen`, `Get`) stay in the CLI/app, which sequence the ceremonies. `keyscheme` gains the one primitive the wallet flow needs (`recover_personal_sign`, the eth two-touch's first touch) and a testkit helper that yields an assertion's three parts.

**Tech Stack:** Rust (std `TcpListener` for the loopback callback; `base64`, `serde_json`, `k256` via keyscheme, `rand`), clap, Ice.

**Spec:** `docs/superpowers/specs/2026-08-27-identity-rework-design.md` §WebAuthn, §Clients, §Phases (5). Page contract: `ops/auth-page/README.md`.

## Global Constraints

- The page's request/result shapes are the README's, byte for byte: fragment `#op=create|get|eth&challenge=<b64url>&user=<b64url>&name=<urlencoded>&cb=<url>`; result = form POST, field `result=<JSON>`; `cb` is loopback only; the listener answers `200 text/html`.
- Eth is TWO-TOUCH: touch 1 signs `ducktape:reveal-key:v1` ‖ 16 random bytes to reveal the pubkey (recovered client-side), touch 2 signs the real preimage (`personal_message(ns, preimage)` bytes passed as `challenge`).
- Passkey registration is TWO CEREMONIES: `create` (pubkey) then `get` over the `AddKey` frame preimage — a `webauthn.create` attestation is not a possession proof.
- No sleeps in tests; the listener test synchronizes on the accepted connection.
- Gates: `cargo test -p authpage -p keyscheme`, `-p node-bin --bin ducktape`, `-p ducktape-app`; clippy `--tests --no-deps` on each; `node ops/auth-page/test.mjs` (wired into `make test`).

---

### Task 1: `keyscheme` — `recover_personal_sign` + assertion parts in the testkit

**Files:** `crates/kernel/keyscheme/src/eth.rs`, `src/lib.rs` (re-export), `src/testkit.rs`

- `pub fn recover_personal_sign(message: &[u8], proof: &[u8]) -> Option<Vec<u8>>` — the recovery half of `verify_personal_sign` (normalize high-S, flip parity, `recover_from_prehash(eip191_digest(message))`), answering the 33-byte compressed SEC1 key; `verify_personal_sign` becomes `recover(..) == expected`.
- testkit: `pub fn passkey_assertion_parts(sk, rp_id, ns, preimage) -> (Vec<u8>, Vec<u8>, Vec<u8>)` (authenticatorData, clientDataJSON, raw R‖S); `assertion` composes it.
- Test: `recover_personal_sign_answers_the_signing_key` (eth_key/eth_proof round trip; a flipped byte recovers a different key).

### Task 2: `crates/authpage`

**Files:** Create `crates/authpage/Cargo.toml`, `crates/authpage/src/lib.rs`; modify workspace `Cargo.toml` (member + dependency).

**Interfaces (produces):**
- `pub const AUTH_PAGE: &str = "https://auth.ducktape.byeongsu.dev/"`.
- `pub enum Request { Create { challenge: [u8; 32], user: u64, name: String }, Get { challenge: [u8; 32] }, Eth { message: Vec<u8> } }`; `pub fn request_url(page: &str, request: &Request, callback: &str) -> String`.
- `pub enum Outcome { Create { credential_id: Vec<u8>, public_key: Vec<u8> }, Get { authenticator_data: Vec<u8>, client_data_json: Vec<u8>, signature: Vec<u8>, user_handle: Option<u64> }, Eth { address: String, signature: Vec<u8>, message: Vec<u8> } }`; `pub fn parse_result(json: &str) -> Result<Outcome, String>` (an `error` result is `Err("<name>: <message>")`).
- `pub struct Listener` — `Listener::bind() -> io::Result<Listener>`, `callback_url(&self) -> String` (`http://127.0.0.1:<port>/`), `wait(self) -> Result<Outcome, String>` (accept ONE connection, read the request, take `result=` out of the form body, answer `200 text/html`, return); `pub fn open_browser(url: &str) -> bool` (xdg-open / open / cmd start; false = print the URL instead).
- Ceremony builders (pure): `pub fn reveal_message() -> Vec<u8>`; `pub fn passkey_frame_request(pubkey, seq, msg) -> (Request, Vec<u8> /*preimage*/)` (challenge = `webauthn_challenge(FRAME_NS, preimage)`); `pub fn passkey_frame(preimage, outcome: &Outcome) -> Result<Vec<u8>, String>` (preimage ‖ `webauthn_proof`); `pub fn wallet_frame_request(pubkey, seq, msg) -> (Request, Vec<u8>)` (`Eth { message: personal_message(FRAME_NS, preimage) }`); `pub fn wallet_frame(preimage, outcome) -> Result<Vec<u8>, String>`; `pub fn login_request(chain_id, device_key, generation) -> Request` (`Get { challenge: webauthn_challenge(IDENTITY_ADD_KEY_NS, add_key_preimage(chain_id, Ed25519, device_key, generation)) }`); `pub fn login_consent(outcome) -> Result<(u64, Vec<u8>), String>` (account number from `userHandle`, the envelope proof); `pub fn create_challenge() -> [u8; 32]` (random, pass-through).
- Tests: URL matches the README fragment (all four params, `user` = 8-byte LE b64url, `name` url-encoded); `parse_result` on the three README samples + the error shape; listener round trip over a raw TCP POST; passkey frame decodes with origin = the passkey (`node::decode_frame`) using `passkey_assertion_parts`; wallet frame decodes with origin = the wallet (`eth_key` signing `eip191_digest(message)`); `login_consent` proof verifies under `IDENTITY_ADD_KEY_NS`.

### Task 3: CLI

**Files:** `bin/node/src/account_cli.rs`, `ops/completions/ducktape.{bash,zsh}`

- `AccountArgs` gains global `--auth-page <URL>` (default `AUTH_PAGE`) and `--no-browser` (print the URL, never spawn a browser).
- `account key add` gains `--passkey` and `--eth` (each `conflicts_with = "pubkey"`): passkey = ceremony 1 `create` (user = account number, name) → `KeyGen(pubkey)` → this device's consent → `AddKey{scheme: Secp256r1, ..}` frame with the PASSKEY as origin via ceremony 2 `get`; eth = touch 1 reveal → recover pubkey → `KeyGen` → consent → `AddKey{scheme: Secp256k1}` frame with the WALLET as origin via touch 2. Both submit through `node_http::submit_frame`.
- `account create --name X --eth`: touch 1 reveal, touch 2 signs a `Create{name, scheme: Secp256k1}` frame with the wallet as origin.
- `account login [--label]`: `KeyGen(this key)` → `login_request` → `get` → `login_consent` → `Get{number}` → `AddKey{scheme: Ed25519, label, authorizer: {key: <the account's Secp256r1 key that verifies>, proof}}` signed by this key.
- Tests (pure): the verb tree parses the new flags; `login` builds an `AddKey` whose authorizer verifies (fake outcome from `passkey_assertion_parts`).

### Task 4: App

**Files:** `app/src/backend/node.rs`, `app/src/ui/extern/backend.ice`, `app/src/ui/handlers/roster.ice`, `app/src/ui/screens/settings.ice`, `app/src/ui/view.ice`, `app/src/ui/tests/app.ice`, `app/src/tests/shell.rs`

- Backend: `register_passkey(rpc, password, chain_id, label)`, `link_wallet(rpc, password, chain_id, label)`, `login_with_passkey(rpc, password, chain_id, label)` — each runs its ceremonies on `spawn_blocking`, opens the browser, returns `bool ! AppError`; `AccountKeyRow` already lists them by scheme.
- UI: the ACCOUNT KEYS card gains "Register a passkey" and "Link a wallet" beside "Add a device"; the no-account block gains "Log in with a passkey". All gated on `password`/`account_busy` and land in `account_changed`/`account_op_failed`.
- Test: the three buttons are wired to the three handlers (source lint), and each handler is `run every … -> account_changed _ | account_op_failed _`.

### Task 5: ops + gates + PR

- `Makefile` `test:` runs `node ops/auth-page/test.mjs`; `ops/auth-page/README.md` gains the eth two-touch paragraph and the client entry points.
- Gates per Global Constraints; commit; PR against `dev`; merge when green; `ops/worktree-clean.sh --yes`.
