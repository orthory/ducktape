# Identity rework phase 6 — signed git push Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `git push --signed` (gpg.format=ssh, an ed25519 SSH key that is a member of an account) lands as a `PushRefs` whose principal is the SIGNER's account — verified by every validator — and `ducktape account key add --ssh <id_ed25519.pub>` admits such a key.

**Architecture:** `keyscheme::sshsig` parses and verifies OpenSSH `SSHSIG` blobs (ed25519, `sha512`/`sha256`) against raw ed25519 (`ed25519-dalek`); the `Ed25519` scheme arm accepts an SSHSIG blob (magic-distinguished from the 64-byte commonware envelope) so an SSH key can sign our own frames/consents via `ssh-keygen -Y sign -n ducktape`. `noded/git_http` advertises `push-cert=<chain>/<repo>`, parses git's `push-cert` block out of the receive-pack request and puts `PushCert { cert, sshsig }` on the op. Forge, in consensus, verifies the SSHSIG under namespace `git`, parses the certificate, requires its update list to equal the op's and its nonce to name the repo, and takes principal = `OfKey(signer)`; an unsigned push keeps today's node-origin principal. The forge guest is regenerated.

**Tech Stack:** Rust (`ed25519-dalek` 2, `sha2`, `base64`), the git pack protocol v0 push-cert extension, OpenSSH `PROTOCOL.sshsig`, wasm guest-builder.

**Spec:** `docs/superpowers/specs/2026-08-27-identity-rework-design.md` §"Git push — `git push --signed` (phase 6)", §keyscheme proof envelopes (`SSHSIG` magic).

## Global Constraints

- SSHSIG per OpenSSH `PROTOCOL.sshsig`: blob = `"SSHSIG"‖u32 1‖string pubkey‖string namespace‖string reserved‖string hash_alg‖string signature`; signed data = `"SSHSIG"‖string namespace‖string reserved‖string hash_alg‖string H(message)`; `pubkey`/`signature` are ssh-wire (`string "ssh-ed25519"‖string bytes`). Armor = `-----BEGIN SSH SIGNATURE-----` / base64 (76-col) / `-----END SSH SIGNATURE-----`.
- git push certificate (send-pack.c `generate_push_cert`): pkt-line `push-cert\0<caps>` then one pkt-line per certificate line INCLUDING the signature's armor lines, then `push-cert-end`, then flush, then the pack. Certificate text = `certificate version 0.1\npusher <ident>\npushee <url>\nnonce <nonce>\n[push-option …\n]\n<old> <new> <refname>\n…`; the signature is over exactly that text (up to the first armor line). With a cert, the plain command lines are NOT sent.
- Nonce = `<chain_id>/<repo>`; the bridge checks the chain half (it knows its chain), consensus checks the repo half (`Env` carries no chain id) — documented, not hidden.
- Namespaces: `git` for push certs (what git uses), `ducktape` for our own frames/consents signed by an SSH key.
- Every `PushRefs` constructor gains `cert`; JSON callers keep working (`#[serde(default)]`).
- Gates: `cargo test -p keyscheme -p forge`, `-p noded --lib git_http`, `-p node-bin --bin ducktape account_cli`; clippy `--tests --no-deps` each; `make wasm-modules` (forge + identity regen — both carry keyscheme) then `make wasm-modules-check`.

---

### Task 1: `keyscheme::sshsig` + the Ed25519 arm's second envelope

**Files:** Create `crates/kernel/keyscheme/src/sshsig.rs`; modify `crates/kernel/keyscheme/src/lib.rs`, `src/testkit.rs`, `crates/kernel/keyscheme/Cargo.toml`, workspace `Cargo.toml` (`ed25519-dalek = "2"`).

**Produces:**
- `pub const SSHSIG_MAGIC: &[u8] = b"SSHSIG"`, `pub const DUCKTAPE_SSH_NS: &str = "ducktape"`, `pub const GIT_SSH_NS: &str = "git"`.
- `pub struct SshSig { pub pubkey: [u8; 32], pub namespace: String, pub hash: SshHash, pub signature: [u8; 64] }`, `pub enum SshHash { Sha256, Sha512 }`.
- `pub fn parse(blob: &[u8]) -> Result<SshSig, String>` (ed25519 only, version 1).
- `pub fn dearmor(text: &str) -> Result<Vec<u8>, String>`; `pub fn armor(blob: &[u8]) -> String`.
- `pub fn verify_ed25519(pubkey: &[u8], namespace: &str, message: &[u8], blob: &[u8]) -> bool`.
- `pub fn authorized_key(line: &str) -> Result<Vec<u8>, String>` — `ssh-ed25519 <b64> [comment]` → 32 raw bytes.
- Ed25519 arm: a proof starting with `SSHSIG` verifies as `sshsig::verify_ed25519(pubkey, "ducktape", &union_unique(ns, preimage), proof)`; else the 64-byte commonware path.
- testkit: `ssh_key(seed) -> ed25519_dalek::SigningKey`, `ssh_pubkey(&sk) -> Vec<u8>`, `sshsig(&sk, namespace, message) -> Vec<u8>`, `ssh_proof(&sk, ns, preimage) -> Vec<u8>`.
- Tests: the REAL `ssh-keygen -Y sign -n git` fixture (pubkey `263850b7…da21`, the cert text, the armored signature) verifies; wrong namespace / tampered blob / other key fail; `KeyScheme::Ed25519.verify` accepts `ssh_proof` and still accepts `ed25519_proof`; `authorized_key` parses and refuses rsa.

### Task 2: forge — `PushRefs.cert`, verify in consensus

**Files:** Create `crates/modules/apps/forge/src/pushcert.rs`; modify `src/interface.rs` (`PushCert`, `cert` field), `src/state.rs` (`apply` PushRefs arm → `push_principal`), `src/lib.rs` (exports), every `PushRefs { .. }` literal (`cert: None`): `src/module.rs`, `src/client.rs`, `tests/{push,multi_repo,sync_round_trip}.rs`, `crates/kernel/host/tests/wasm_forge_parity.rs`, `crates/modules/apps/runs/src/sink.rs`, `bin/node/src/{relay,blob_fetch}.rs`; `crates/modules/apps/forge/Cargo.toml` (`keyscheme`).

**Produces:**
- `pub struct PushCert { pub cert: Vec<u8>, pub sshsig: Vec<u8> }`; `ForgeMsg::PushRefs { repo, updates, pack_digest, #[serde(default)] cert: Option<PushCert> }`.
- `pushcert::parse(cert: &[u8]) -> Result<Certificate, String>` with `Certificate { nonce: String, updates: Vec<RefUpdate> }` (branch short names; zero oid → `None`; non-`refs/heads/` refused; 40-hex refused otherwise).
- `pushcert::nonce(chain_id, repo) -> String` = `"{chain_id}/{repo}"`; `pushcert::nonce_names_repo(nonce, repo) -> bool` (suffix `/{repo}`).
- `pushcert::signer(cert: &PushCert, repo: &str, updates: &[RefUpdate]) -> Result<Vec<u8>, String>`: SSHSIG verify under `git`, parse, nonce names repo, update SETS equal → the 32-byte signer.
- state: `push_principal(ctx, name, cert, updates)`: `Some(cert)` → `signer` → `identity_account(signer).map_or(signer, account_principal)`; `None` → `principal_of_origin`.
- Tests (module.rs): a signed push (testkit `sshsig` under `git`) claims the repo for the SIGNER key (not the frame origin); a later unsigned push to protected `main` from the node origin is refused; certificate updates ≠ op updates refused; nonce for another repo refused; bad signature refused. `pushcert` unit tests on the real fixture text.

### Task 3: noded `git_http` — advertise + parse the push-cert

**Files:** `crates/noded/src/git_http.rs`.

- `GitService::Receive` caps gain `push-cert=<nonce>` (nonce from `handle.status.current().chain_id` + repo; a node with no chain id yet advertises no push-cert).
- `parse_push_commands(commands: &[Vec<u8>], expected_nonce: &str) -> Result<PushCommands, String>` (pure): `PushCommands { cmds: Vec<(old, new, refname)>, cert: Option<PushCert> }`. First line `push-cert` → collect lines until `push-cert-end`; split at the armor's BEGIN line; `cert` = the text before, `sshsig` = `dearmor(rest)`; verify `nonce == expected_nonce` (else `Err("push-cert nonce …")`); cmds = the cert's update lines. Else today's `<old> <new> <refname>` parsing.
- Early refusal (a clean `ng`) when `keyscheme::sshsig::verify_ed25519` fails at the bridge — consensus re-verifies regardless.
- Tests: a pkt-line stream built from the fixture parses to the cert + one command; a wrong nonce refuses; the unsigned stream still parses.

### Task 4: CLI `account key add --ssh <pub>`

**Files:** `bin/node/src/account_cli.rs`, `ops/completions/ducktape.{bash,zsh}`.

- `KeyCmd::Add` gains `--ssh <PATH>` (conflicts with pubkey/passkey/eth); `NewKey::Ssh(PathBuf)`.
- `cmd_key_add_ssh`: read the `.pub` → `authorized_key` → member consent (`consented_add_key(Ed25519)`) → `preimage = node::frame_preimage(Ed25519, pubkey, seq, msg)` → `ssh-keygen -Y sign -n ducktape -f <pub>` over `union_unique(FRAME_NS, preimage)` on stdin → `dearmor` → frame = preimage ‖ blob → `submit_frame`. Prints the git config lines to finish the setup.
- Pure: `ssh_frame(preimage, armored) -> Result<Vec<u8>, String>`; test: a frame built from testkit `sshsig` decodes at `node::decode_frame` with the SSH key as origin.

### Task 5: wasm regen, gates, PR

- `cargo run -q -p guest-builder -- crates/modules/apps/forge` and `… system/identity`; copy to `crates/kernel/host/tests/fixtures/`; `make wasm-modules-check`.
- Gates per Global Constraints; commit; PR against `dev`; merge when green; sweep.
