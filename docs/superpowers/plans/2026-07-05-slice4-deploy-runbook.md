# Slice 4 (final) — Real Coordinator Deployment Recipe + Cross-Machine Acceptance Runbook + Integration-Gap Handoff

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task ends in a commit; do not batch commits. This is a **docs + config** slice: there is no new library logic and (deliberately) **no node wiring**. The one code artifact is a subprocess smoke-test that proves the deploy invocation actually serves.

**Goal.** Land the three Slice-4 acceptance artifacts from the design of record, honestly scoped:

1. A **real, working** coordinator deployment recipe for `p2p.ducktape.industries` (`bin/coordinator` as an untrusted, non-root, least-privilege VPS/container service) — this part works **today**.
2. A **cross-machine zero-exposure runbook** for two NAT'd validators + a coordinator, using the real binaries, in which **every step is tagged** `[WORKS TODAY]` or `[NEEDS NODE WIRING]` so nobody mistakes the proven mechanism for a live end-to-end tunnel.
3. An **integration-gap assessment** — the precise, file-anchored handoff enumerating what remains to wire `nat-traversal` (`NatClient`) and `wireguard-effect` (`apply_tunnel_plan` / `DefguardWireGuardEffect`) into the live `ducktape-node`, the composition decision against the existing commonware `authenticated::discovery` transport, and why the real cross-machine run needs the user's infra.

**The honesty invariant this whole slice rests on.** Slices 0a/0b/2/3 built the **full reachability mechanism** as CI-proven library crates (`crates/system/nat-traversal`, `crates/system/wireguard-effect`), and Slice 1 wired **v3 signed invites** into `bin/node/src/config.rs`. **BUT** `nat-traversal` and `wireguard-effect` are **not** dependencies of `node-bin` and are **not referenced anywhere in `bin/node`** — verified below. So the live `ducktape-node` does **not** yet discover its reflexive address, hole-punch, bring up a WireGuard interface, or relay. **The mechanism is proven; it is UNWIRED into the live node.** This slice must never present the full zero-exposure tunnel as working today. It ships the coordinator deployment (real), a runbook that draws the works/not-wired line exactly, and the engineering handoff to close the gap.

## Design anchors

`docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`:

- §"Acceptance": three items must hold before the epic merges — (1) CI simulated-NAT suite [**DONE in Slice 3**], (2) cross-machine zero-exposure demo [**needs node wiring + real infra**], (3) real `p2p.ducktape.industries` deployed with a documented recipe and a test network's v3 invite pointing at it [**this slice ships the recipe; the "test network v3 invite pointing at it" cannot round-trip until the node consumes `Coordinated` hints — see the gap doc**].
- §"Epic decomposition": "Slice 4. Real `p2p.ducktape` deployment recipe + cross-machine acceptance runbook."
- §"Components" 1 (untrusted entry helper), 3 (STUN client + self-endpoint discovery), 4 (UDP hole-punch), 5 (WireGuard effect wiring), 6 (relay fallback — two relay concepts kept separate).
- §"Trust and threat model": the coordinator is untrusted — sees ciphertext + coarse topology only; **never** holds a key, **never** decrypts, **never** joins consensus. The deploy recipe must make that structural (no secrets on the box).

Companion operator doc modeled on: `docs/sentry-deployment.md` (Phase 1 sentry recipes) — same voice, same "why this is safe" framing, same docs-root placement (it is **not** in the vocs sidebar; neither are these).

## Verified ground truth (confirmed by reading the tree at HEAD `8a272ac`)

- **The gate builds.** `cargo build -p coordinator-bin` → exit 0, produces `target/debug/coordinator` (ELF x86-64).
- **Coordinator CLI.** `bin/coordinator/src/main.rs`: `coordinator --listen <SocketAddr>`, default `0.0.0.0:3478`; binds a single **UDP** socket (`tokio::net::UdpSocket::bind`), prints `coordinator listening on {addr}` to **stderr**, then `nat_traversal::run_coordinator(sock).await`. No TCP listener. No key material. No config file. No disk. Stateless.
- **Coordinator deps.** `bin/coordinator/Cargo.toml`: `nat-traversal` + `tokio` (`net`, `rt-multi-thread`, `macros`); dev-deps add `nat-traversal`'s `simnat` feature + `tokio`. It is a workspace member (`Cargo.toml` line 47: `"bin/coordinator"`).
- **`run_coordinator` relay bind.** `crates/system/nat-traversal/src/client.rs::run_coordinator_with_idle`: relay-splice sockets are bound on `bind_ip = sock.local_addr().ip()`. **Load-bearing caveat:** if the coordinator is launched with `--listen 0.0.0.0:3478`, `bind_ip` is `0.0.0.0`, so a `RelayGrant` hands the client `0.0.0.0:<ephemeral>` — **not remotely dialable**. STUN reflexive, rendezvous `Lookup`, and `PunchSync` are all unaffected (they echo/return the *observed source*, independent of the coordinator's bind IP); **only the ciphertext-relay fallback** needs the coordinator bound to its **routable public IP**. This distinction drives both the deploy recipe and the gap doc.
- **`NatClient` surface** (`client.rs`) the node will eventually drive: `bind` / `bind_multi(key, Vec<SocketAddr>)`, `discover_reflexive` / `discover_reflexive_failover(per_try) -> (idx, reflexive)`, `register`, `readvertise(nonce)`, `lookup(peer)`, `recv_punch_sync`, `send_punch_to` / `recv_punch_from(expected)`, `request_relay(peer) -> (session, relay_addr)`, `relay_send` / `relay_recv`. `NodeKey([u8; 32])` is a raw ed25519 public key.
- **WireGuard effect surface** (`crates/system/wireguard-effect/src/{lib,wiring,defguard_effect}.rs`): `trait WireGuardEffect { create_interface / apply(&InterfaceConfiguration) / remove_interface }`; `apply_tunnel_plan(effect, ifname, private_key_base64, listen_endpoint, plan: &TunnelInstallPlan, peer_endpoint_override: Option<SocketAddr>)`; real `DefguardWireGuardEffect` (unix, `defguard_wireguard_rs` userspace `WGApi`); test `FakeWireGuardEffect`. The crate's own module doc states: **"nothing in the workspace calls `WGApi::configure_interface` today."** `peer_endpoint_override` is the exact seam where a punched reflexive (or a relay socket) is injected.
- **v3 invite wiring (Slice 1)** in `bin/node/src/config.rs`: `INVITE_PREFIX_V3 = "ducktape-invite-v3:"`; `enum Reach { Direct(String), Fronted(String), Coordinated(CoordRef) }`; `CoordRef { coord_addr: String, coord_key: ed25519::PublicKey }`; `struct ReachHint { expected_key, reach }`; canonical form `coordinated:<ek_hex>@<coord_addr>#<coord_key_hex>`; v2 stays parse-only → all-`Direct`, no signature. **Verified placeholder:** `NetworkDescriptor::reach_entries()` (config.rs ~337–354) resolves `Reach::Coordinated(c) => &c.coord_addr` and pushes `(expected_key, coord_addr)` straight into the list that becomes the commonware **TCP** `bootstrappers` (`bin/node/src/main.rs` ~2540, fed to `discovery::Config::local(...)` at ~2641). I.e. today a `Coordinated` hint is dialed as an ordinary **TCP mesh peer** at the coordinator's address — which is a **UDP STUN/rendezvous/relay** service, not a mesh peer. So the config type exists but the transport wiring is a stub that cannot work as a reachability path.
- **The node does NOT depend on the reachability crates.** `grep -rn 'nat-traversal|wireguard-effect|nat_traversal|wireguard_effect' bin/node bin/noded` → **NONE FOUND**; `bin/node/Cargo.toml` dependency list confirms neither crate is present. The mesh is built solely from commonware `authenticated::discovery` (`Network::new(..., discovery::Config::local(...))`, `main.rs` ~2649). No `NatClient`, no STUN, no hole-punch, no reflexive publish, no `WireGuardEffect`.

## Global Constraints

- **Merge gate = coordinator-bin build + doc existence. Do NOT pull node-bin clippy.** `bin/node`/`noded` clippy is pre-existingly red from toolchain drift in unrelated dep crates (same constraint every prior slice carried). The Slice-4 gate is exactly:
  ```bash
  cargo build -p coordinator-bin && \
  test -f docs/deploy/coordinator.md && \
  test -f docs/deploy/cross-machine-zero-exposure-runbook.md && \
  test -f docs/deploy/private-cutover-integration-gap.md && \
  test -f ops/coordinator/ducktape-coordinator.service && \
  test -f ops/coordinator/Dockerfile
  ```
- **No node wiring in this slice.** Do **not** add `nat-traversal` / `wireguard-effect` to `bin/node`'s deps, do **not** touch the mesh boot, do **not** re-route `Coordinated` hints. That is the follow-on ("Slice 5 — node reachability wiring") the gap doc hands off. Touching it here would break the "docs + config, gate stays cheap" contract and drag in the red node-bin clippy.
- **Honesty tags are mandatory in the runbook.** Every actionable step in `cross-machine-zero-exposure-runbook.md` carries exactly one of `[WORKS TODAY]` or `[NEEDS NODE WIRING]`. A step that is partly-real (e.g. "the coordinator answers, but the node won't use the answer") is `[NEEDS NODE WIRING]` with a one-line "the mechanism exists in `crates/…`; the node does not call it yet."
- **The one new test must be real and hermetic.** Task 2's subprocess test drives the **actual compiled `coordinator` binary** via `env!("CARGO_BIN_EXE_coordinator")` with `--listen 127.0.0.1:0`, reads the real bound port from the binary's stderr line, and proves it answers a live `BindRequest`. No fixed ports (no CI collisions), no sleeps-as-synchronization. It runs under `cargo test -p coordinator-bin` (not the merge gate, but green in this task).
- **Untrusted-coordinator invariant, made structural on the box.** The recipe puts **no key, no secret, no state** on the coordinator host. `DynamicUser=yes`, empty capability set, read-only filesystem. If a future reviewer can find a place a secret *would* live, the recipe is wrong.
- **Placement matches the sentry doc.** Operator docs live at `docs/deploy/*.md` (docs root family, like `docs/sentry-deployment.md`); ops artifacts live at `ops/coordinator/`. Do **not** add these to `docs/vocs.config.ts`'s sidebar — `sentry-deployment.md` isn't there either; keep the blast radius to new files + one cross-link in the design spec.

## Deliverables map (every file this slice creates or edits)

| Path | Kind | Task |
|---|---|---|
| `docs/deploy/coordinator.md` | new — coordinator deploy recipe (real) | 1 |
| `ops/coordinator/ducktape-coordinator.service` | new — hardened systemd unit | 1 |
| `ops/coordinator/coordinator.env.example` | new — the one operator-edited line (listen addr; **not** a secret) | 1 |
| `ops/coordinator/Dockerfile` | new — multi-stage, distroless, non-root | 1 |
| `ops/coordinator/README.md` | new — 20-line pointer tying the three ops files together | 1 |
| `bin/coordinator/tests/deploy_smoke.rs` | new — subprocess proof the real `coordinator --listen` serves | 2 |
| `bin/coordinator/Cargo.toml` | edit — add `process`/`io-util` tokio dev-features for the subprocess test | 2 |
| `docs/deploy/cross-machine-zero-exposure-runbook.md` | new — tagged two-machine runbook | 3 |
| `docs/deploy/private-cutover-integration-gap.md` | new — the node-wiring handoff | 4 |
| `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md` | edit — Acceptance section links the runbook + gap doc, honest status | 5 |

---

### Task 1: Coordinator deploy recipe — `docs/deploy/coordinator.md` + `ops/coordinator/` (the REAL, works-today part)

Ship the untrusted-coordinator deployment as (a) an operator recipe doc and (b) three ready-to-use ops artifacts (systemd unit, its env example, a distroless Dockerfile) plus a short README. This is the one Slice-4 acceptance item that is fully real today: `bin/coordinator` runs as-is.

**Files:**
- Create: `docs/deploy/coordinator.md`
- Create: `ops/coordinator/ducktape-coordinator.service`
- Create: `ops/coordinator/coordinator.env.example`
- Create: `ops/coordinator/Dockerfile`
- Create: `ops/coordinator/README.md`

- [ ] **Step 1: Write the hardened systemd unit — `ops/coordinator/ducktape-coordinator.service`**

Use this exact content. Rationale is inline as comments so an operator/reviewer can audit the least-privilege posture without leaving the file.

```ini
# ducktape-coordinator — the UNTRUSTED private-cutover entry helper.
#
# Runs bin/coordinator (STUN reflexive + rendezvous + ciphertext relay). It is
# NOT a validator, holds NO key material, serves NO state, and NEVER decrypts —
# see docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md.
# The hardening below makes those invariants structural: a dynamic throwaway
# user, an empty capability set, a read-only filesystem, and no secrets on disk.
[Unit]
Description=Ducktape private-cutover coordinator (untrusted STUN/rendezvous/relay)
Documentation=https://github.com/orthory/ducktape/blob/dev/docs/deploy/coordinator.md
After=network-online.target
Wants=network-online.target

[Service]
# The ONE operator-set value: which address to bind. Ship 0.0.0.0:3478 by
# default; set it to the box's ROUTABLE PUBLIC IP (e.g. 203.0.113.10:3478) if
# you need the ciphertext-relay fallback — relay-grant addresses are bound on
# the coordinator's socket IP, so a 0.0.0.0 bind hands peers an undialable
# 0.0.0.0:<port> relay endpoint. STUN + rendezvous + hole-punch work fine on
# 0.0.0.0. This file holds NO secret; the coordinator has none.
EnvironmentFile=/etc/ducktape/coordinator.env
ExecStart=/usr/local/bin/ducktape-coordinator --listen ${COORDINATOR_LISTEN}
Restart=always
RestartSec=2

# ---- least privilege ----------------------------------------------------
# Ephemeral, on-the-fly user/group: nothing to compromise, no home, no shell.
DynamicUser=yes
# 3478 > 1024, so binding it needs NO privileged-port capability. Drop them all.
CapabilityBoundingSet=
AmbientCapabilities=
NoNewPrivileges=yes
# Stateless: no state/config/cache/log dirs, no writable paths at all.
ProtectSystem=strict
ProtectHome=yes
ReadOnlyPaths=/
PrivateTmp=yes
PrivateDevices=yes
ProtectProc=invisible
ProcSubset=pid
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
RestrictNamespaces=yes
# UDP only — no unix sockets, no raw/packet families.
RestrictAddressFamilies=AF_INET AF_INET6
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources
UMask=0077

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 2: Write the env example — `ops/coordinator/coordinator.env.example`**

```sh
# Copy to /etc/ducktape/coordinator.env and edit the one line below.
# This is NOT a secret — the coordinator holds no keys. It is only the bind addr.
#
# Default (STUN + rendezvous + hole-punch; the common path):
COORDINATOR_LISTEN=0.0.0.0:3478
#
# If you need the ciphertext-RELAY fallback to work, bind the ROUTABLE PUBLIC IP
# instead, because relay-grant endpoints inherit the coordinator's socket IP:
# COORDINATOR_LISTEN=203.0.113.10:3478
```

- [ ] **Step 3: Write the Dockerfile — `ops/coordinator/Dockerfile`**

Multi-stage; build context is the **repo root** (so the whole workspace is available to `cargo build -p coordinator-bin`). Pin the toolchain to the repo's `rustc 1.96`.

```dockerfile
# Build context = repo root:  docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
FROM rust:1.96-bookworm AS build
WORKDIR /src
COPY . .
# Only the coordinator crate and its (workspace) deps are compiled.
RUN cargo build --release -p coordinator-bin

# Distroless cc runtime: glibc + libgcc for the dynamically-linked binary, no
# shell, no package manager, runs as the built-in non-root uid 65532 (nonroot).
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=build /src/target/release/coordinator /usr/local/bin/ducktape-coordinator
USER nonroot:nonroot
# STUN/rendezvous/relay control port. Publish with:  -p 3478:3478/udp
EXPOSE 3478/udp
ENTRYPOINT ["/usr/local/bin/ducktape-coordinator"]
# Override for relay: --listen <container-routable-ip>:3478 (see coordinator.md).
CMD ["--listen", "0.0.0.0:3478"]
```

- [ ] **Step 4: Write `ops/coordinator/README.md`** — a ~20-line index: what the three files are, the one-liner to install the systemd unit (`cp ducktape-coordinator.service /etc/systemd/system/`, `cp coordinator.env.example /etc/ducktape/coordinator.env`, install the binary to `/usr/local/bin/ducktape-coordinator`, `systemctl enable --now`), the one-liner to build+run the container (`docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .` then `docker run --rm -p 3478:3478/udp ducktape-coordinator`), and a pointer to `docs/deploy/coordinator.md` for the full recipe + the relay-bind caveat. State plainly: **holds no keys, no state, untrusted by design.**

- [ ] **Step 5: Write the recipe doc — `docs/deploy/coordinator.md`**

Structure (write real prose for each; this is the operator-facing companion, voiced like `docs/sentry-deployment.md`):

1. **Title + one-paragraph intro.** "How to run `bin/coordinator` as `p2p.ducktape.industries`: an untrusted, non-validator reachability helper." Link the design of record.
2. **Why this is safe (untrusted by design).** Reprise the invariant from the design's §"The invariant everything rests on": the mesh (commonware `authenticated::discovery`) and the WireGuard data plane are both key-authenticated + end-to-end encrypted, so the coordinator sees ciphertext + coarse topology only; it cannot decrypt, impersonate, MITM, serve state, or join consensus. Therefore it is safe to run as throwaway infra with **no key on the box**. Point at the threat-model table in the design.
3. **What it is.** `coordinator --listen <addr>` (default `0.0.0.0:3478`), one UDP socket, stateless, three services (rendezvous / STUN reflexive / ciphertext relay). No TCP, no config file, no disk, no secret.
4. **Deploy A — systemd (bare VPS).** Build (`cargo build --release -p coordinator-bin`), copy `target/release/coordinator` → `/usr/local/bin/ducktape-coordinator`, install `ops/coordinator/ducktape-coordinator.service` + `/etc/ducktape/coordinator.env`, `systemctl enable --now ducktape-coordinator`, verify with `systemctl status` and `ss -lunp 'sport = :3478'`. Call out the DynamicUser / empty-caps / read-only-fs posture and *why* (no secret to steal, no state to corrupt).
5. **Deploy B — Docker/OCI.** `docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .`; `docker run --rm -p 3478:3478/udp ducktape-coordinator`; non-root distroless note. For **relay**, use `--network host` + `--listen <public-ip>:3478` (see the caveat).
6. **The relay-bind caveat (load-bearing).** Verbatim from ground truth: `run_coordinator` binds relay-splice sockets on the coordinator socket's own IP, so `--listen 0.0.0.0:3478` yields undialable `0.0.0.0:<port>` relay grants. STUN reflexive, rendezvous, and hole-punch are unaffected (they carry the *observed source*). **If you need relay fallback, bind the routable public IP.** Cross-reference the gap doc, which notes a future coordinator-side fix (learn/emit the public IP).
7. **DNS + firewall.** Point an A record `p2p.ducktape.industries` → the VPS IP. Open **inbound UDP 3478** (and, for relay, the ephemeral UDP range the OS assigns, or run the public-IP bind so grants are dialable). No TCP port is needed.
8. **Redundancy / not-uniquely-load-bearing.** Run **multiple** coordinators; a v3 invite carries a `Vec` of reach hints and `NatClient::discover_reflexive_failover` walks them (Slice 3). An established *punched* path survives a coordinator restart; only *relay* fallback and *new* rendezvous depend on a live coordinator. (Contrast the in-path sentry SPOF from `docs/sentry-deployment.md`.)
9. **What this recipe does NOT do (forward reference).** The coordinator is live and correct, but **the `ducktape-node` does not yet use it** — reflexive discovery, hole-punch, WireGuard bring-up, and relay are unwired in the node. See `docs/deploy/cross-machine-zero-exposure-runbook.md` (tagged) and `docs/deploy/private-cutover-integration-gap.md` (the handoff). Do not claim a zero-exposure tunnel from this page.

- [ ] **Step 6: Verify the recipe matches the real CLI and the gate stays green**

Run and eyeball:
```bash
cargo build -p coordinator-bin
grep -n 'listen' bin/coordinator/src/main.rs            # confirms --listen + 0.0.0.0:3478 default
grep -n 'COORDINATOR_LISTEN\|--listen' ops/coordinator/ducktape-coordinator.service
grep -n 'cargo build --release -p coordinator-bin' ops/coordinator/Dockerfile
```
Expected: build exits 0; the unit's `ExecStart` uses exactly `--listen` (the binary's only parsed flag); the Dockerfile builds exactly `-p coordinator-bin`. Fix any drift so the docs can never describe a flag the binary doesn't parse.

- [ ] **Step 7: Commit**

```bash
git add docs/deploy/coordinator.md ops/coordinator/
git commit -m "docs(deploy): coordinator deploy recipe + hardened systemd unit + distroless Dockerfile"
```

---

### Task 2: Prove the deploy invocation actually serves — `bin/coordinator/tests/deploy_smoke.rs`

Turn the recipe's central "[WORKS TODAY]" claim into a CI-runnable proof: boot the **actual compiled `coordinator` binary** exactly as the systemd unit does (`coordinator --listen <addr>`), learn its real bound port from stderr, and drive a live `NatClient` `BindRequest` against it. This is the difference between *asserting* the recipe works and *showing* it. Hermetic: `--listen 127.0.0.1:0` lets the OS pick the port; the test parses the port from the binary's own `coordinator listening on {addr}` line, so there is no fixed-port CI collision and no sleep-as-synchronization.

**Files:**
- Modify: `bin/coordinator/Cargo.toml` (dev-dep tokio gains `process` + `io-util`)
- Create: `bin/coordinator/tests/deploy_smoke.rs`

**Interfaces used:** `env!("CARGO_BIN_EXE_coordinator")` (Cargo sets this for integration tests of a crate that defines a `[[bin]]`), `tokio::process::Command`, `tokio::io::{AsyncBufReadExt, BufReader}`, and the already-public `nat_traversal::{NatClient, NodeKey}`.

- [ ] **Step 1: Add the dev-dep features (RED-enabling)**

In `bin/coordinator/Cargo.toml`, extend the **dev-dependencies** `tokio` features so the test can spawn a child and read its stderr line-by-line:

```toml
[dev-dependencies]
nat-traversal = { workspace = true, features = ["simnat"] }
tokio = { workspace = true, features = ["net", "rt-multi-thread", "macros", "process", "io-util"] }
```

(Only dev-deps change; the shipped binary's deps are untouched.)

- [ ] **Step 2: Write the test — `bin/coordinator/tests/deploy_smoke.rs`**

```rust
//! Proof that the DEPLOYED invocation works: boot the real compiled
//! `coordinator` binary exactly as `ops/coordinator/ducktape-coordinator.service`
//! does (`coordinator --listen <addr>`), then drive a live `NatClient` against
//! it. This is the "[WORKS TODAY]" claim in docs/deploy/coordinator.md, made
//! executable. Hermetic: `--listen 127.0.0.1:0` -> the OS picks the port, which
//! the test reads back from the binary's own stderr line (no fixed port, no
//! sleep-as-sync).

use std::net::SocketAddr;
use std::process::Stdio;
use std::time::Duration;

use nat_traversal::{NatClient, NodeKey};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

#[tokio::test]
async fn deployed_coordinator_binary_answers_a_bind_request() {
    // Boot the ACTUAL binary the recipe installs, with the OS choosing the port.
    let mut child = Command::new(env!("CARGO_BIN_EXE_coordinator"))
        .arg("--listen")
        .arg("127.0.0.1:0")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the compiled coordinator binary");

    // The binary prints `coordinator listening on {addr}` to stderr once bound,
    // BEFORE serving. Read the real bound address from that line — this both
    // synchronizes the test and proves the CLI parses `--listen`.
    let stderr = child.stderr.take().expect("piped stderr");
    let mut lines = BufReader::new(stderr).lines();
    let addr: SocketAddr = timeout(Duration::from_secs(10), async {
        while let Some(line) = lines.next_line().await.expect("read stderr") {
            if let Some(rest) = line.strip_prefix("coordinator listening on ") {
                return rest.trim().parse().expect("parse bound addr");
            }
        }
        panic!("coordinator exited before announcing its listen address");
    })
    .await
    .expect("coordinator must announce its listen address promptly");

    // Drive the real STUN reflexive path against the running process.
    let client = NatClient::bind(NodeKey([7u8; 32]), addr)
        .await
        .expect("bind client");
    let reflexive = timeout(Duration::from_secs(5), client.discover_reflexive())
        .await
        .expect("the deployed coordinator must answer a BindRequest")
        .expect("reflexive");

    // Wildcard client bind vs observed loopback source: the port is the
    // load-bearing invariant (same rule as bin/coordinator/tests/smoke.rs).
    assert_eq!(
        reflexive.port(),
        client.local_addr().await.unwrap().port(),
        "the coordinator echoes the client's observed reflexive port"
    );

    // Tidy up (kill_on_drop also covers a panic path).
    let _ = child.start_kill();
}
```

- [ ] **Step 3: Run it**

```bash
cargo test -p coordinator-bin --test deploy_smoke
```
Expected: PASS. Also re-run the existing in-process smoke: `cargo test -p coordinator-bin` (both `smoke.rs` and `deploy_smoke.rs` green).

- [ ] **Step 4: Confirm the merge gate still holds**

```bash
cargo build -p coordinator-bin
```
Expected: exit 0 (the shipped binary's deps are unchanged; only dev-deps grew).

- [ ] **Step 5: Commit**

```bash
git add bin/coordinator/Cargo.toml bin/coordinator/tests/deploy_smoke.rs
git commit -m "test(coordinator): subprocess proof the deployed 'coordinator --listen' serves a BindRequest"
```

---

### Task 3: Cross-machine zero-exposure runbook — `docs/deploy/cross-machine-zero-exposure-runbook.md`

The step-by-step for two NAT'd machines + a coordinator using the **real binaries**, with **every step tagged**. This is the acceptance runbook for the design's §"Acceptance" item 2 — written so it is unambiguous which steps run today and which are blocked on node wiring. It must **not** present the full zero-exposure tunnel as working.

**Files:**
- Create: `docs/deploy/cross-machine-zero-exposure-runbook.md`

- [ ] **Step 1: Write the tag legend + honesty preamble**

Open with the legend and the load-bearing caveat, stated once, up top:

> **Legend.** `[WORKS TODAY]` — runs now with shipped binaries. `[NEEDS NODE WIRING]` — the mechanism exists and is CI-proven in `crates/system/nat-traversal` and/or `crates/system/wireguard-effect`, but `ducktape-node` does **not** call it yet (`nat-traversal` and `wireguard-effect` are not dependencies of `bin/node` — verified). See `docs/deploy/private-cutover-integration-gap.md`.
>
> **This runbook does not yet yield a working zero-exposure tunnel.** It stands up the real coordinator and shows the invite/entry path that works today, then marks precisely where the node must learn to discover its reflexive, hole-punch, bring up WireGuard, and relay. The CI simulated-NAT suite (Slice 3, `crates/system/nat-traversal/tests/simnat_ci.rs`) proves the *logic*; this runbook is what turns it into a *deployment* once the node is wired.

- [ ] **Step 2: Write the topology + prerequisites**

Three hosts: **Coordinator** (public VPS, `p2p.ducktape.industries`), **Validator A** and **Validator B** (each behind its own NAT, neither with an inbound port-forward). Extends the Ducktape-2 live-join rig (see `MEMORY` note *live-join-ducktape-2* and `docs/superpowers/specs/…admission…`). Prereqs: the coordinator deployed per `docs/deploy/coordinator.md`; A and B each have a built `ducktape-node`; a founder able to mint v3 invites.

- [ ] **Step 3: Write the tagged steps**

Each step gets exactly one tag. Concretely:

1. **Deploy the coordinator on the VPS.** `[WORKS TODAY]` — `docs/deploy/coordinator.md`; verify with the Task-2 subprocess proof or `ss -lunp`.
2. **Mint a v3 invite carrying a `Coordinated` reach hint.** `[WORKS TODAY]` (encoding) — Slice 1's `INVITE_PREFIX_V3` + `ReachHint`/`CoordRef` produce `coordinated:<ek>@p2p.ducktape.industries:3478#<coord_key>`; it round-trips through `config.rs` `pack`/`unpack`/`parse`. Note the **partial**: the invite *encodes* the coordinator correctly, but see step 5.
3. **A and B each generate an identity and get admitted** (founder runs `invite-accept`). `[WORKS TODAY]` — unchanged admission path.
4. **A and B boot dial-out-only against the coordinator's reflexive/rendezvous service.** `[NEEDS NODE WIRING]` — the node never constructs a `NatClient`, never sends a `BindRequest`, never `register`s. The coordinator would answer (Task 2 proves it), but nothing in `bin/node` asks. Mechanism: `nat_traversal::NatClient::{bind_multi,discover_reflexive_failover,register}`.
5. **A `Coordinated` hint is consumed as a reachability path.** `[NEEDS NODE WIRING]` — verified stub: `reach_entries()` feeds `coord_addr` into the commonware **TCP** `bootstrappers`, so today the node would try to open a *mesh* connection to the coordinator's **UDP** port and fail. The hint must instead be routed to a `NatClient`. (This is why step 2 is only *partial*.)
6. **A and B publish their reflexive endpoints and rendezvous.** `[NEEDS NODE WIRING]` — no reflexive is discovered or published into `EndpointAdvertisementV1.wireguard_endpoint`; no `lookup`/`recv_punch_sync`. Mechanism exists (`nat-traversal` rendezvous + `wireguard-upgrade` advertisement), unwired.
7. **A and B hole-punch a direct WireGuard tunnel (coordinator-timed simultaneous open).** `[NEEDS NODE WIRING]` — `send_punch_to`/`recv_punch_from` exist and are CI-proven (`drive_simulated`), but the node never drives them, and no WireGuard interface is created (`apply_tunnel_plan`/`DefguardWireGuardEffect` is never called — the crate says so).
8. **On hole-punch failure, fall back to the coordinator ciphertext relay.** `[NEEDS NODE WIRING]` — `request_relay` + `peer_endpoint_override` exist and are CI-proven (`drive_with_relay_fallback`), unwired; **and** the relay-bind caveat applies (coordinator must bind its public IP).
9. **Real state-sync / app-hash flows over the tunnel.** `[NEEDS NODE WIRING]` — depends on 6–8; nothing to run today.

- [ ] **Step 4: Write "what you CAN demo today" vs "what proves the tunnel"**

A short closing section: today you can (a) deploy the coordinator and prove it answers (Task 2), (b) mint/parse a v3 `Coordinated` invite, (c) admit A and B. What you cannot yet do is any step tagged `[NEEDS NODE WIRING]`. The moment the node wiring (gap doc, "Slice 5") lands, this runbook becomes the acceptance §2 procedure verbatim — the tags flip to `[WORKS TODAY]` step by step. Point at the CI sim-NAT suite as the logic-level proof that already exists.

- [ ] **Step 5: Verify no step is untagged, commit**

```bash
grep -nE '^\s*[0-9]+\.' docs/deploy/cross-machine-zero-exposure-runbook.md | grep -vqE '\[WORKS TODAY\]|\[NEEDS NODE WIRING\]' && echo 'UNTAGGED STEP FOUND' || echo 'all steps tagged'
git add docs/deploy/cross-machine-zero-exposure-runbook.md
git commit -m "docs(deploy): cross-machine zero-exposure runbook — every step tagged works-today vs needs-node-wiring"
```
(The grep must print `all steps tagged` before committing.)

---

### Task 4: Integration-gap assessment — `docs/deploy/private-cutover-integration-gap.md` (the honest handoff)

The precise engineering handoff: exactly what remains to wire `nat-traversal` (`NatClient`) and `wireguard-effect` into `ducktape-node`'s networking loop, the composition decision against commonware `authenticated::discovery`, the coordinator-side relay-bind fix, and why the real cross-machine run needs the user's infra. This is the document that lets a future implementer (or "Slice 5") pick the work up cold.

**Files:**
- Create: `docs/deploy/private-cutover-integration-gap.md`

- [ ] **Step 1: Write §1 — Current state (what's proven, what's unwired)**

State the verified facts from this plan's "Verified ground truth": the mechanism is fully built + CI-proven in `crates/system/nat-traversal` and `crates/system/wireguard-effect`; Slice 1 wired v3 invite *encoding*; but `bin/node` depends on **neither** reachability crate and references **neither** (grep evidence). Enumerate the four capabilities the live node lacks: reflexive discovery, hole-punch, WireGuard bring-up, relay.

- [ ] **Step 2: Write §2 — The composition decision (reachability plane vs commonware `authenticated::discovery`)**

The load-bearing design call, argued explicitly:

- There are **two planes**. The **control mesh** is commonware `authenticated::discovery` over **TCP** (`main.rs` ~2641), already frontable via the P1 sentry / coordinator-fronting entry design (`docs/sentry-deployment.md`) — it stays TCP and is **not** the thing this epic hole-punches. The **data tunnel** is validator↔validator **WireGuard** (the `wireguard-upgrade` protocol) — *that* is what needs reflexive discovery + hole-punch + effect bring-up + relay.
- **Decision:** the reachability plane composes **orthogonally** to `authenticated::discovery`. The node runs a `NatClient` on its own UDP socket to (a) discover its reflexive and publish it into `EndpointAdvertisementV1.wireguard_endpoint`, and (b) rendezvous/punch for the **WireGuard** endpoint. It then drives `wireguard_effect::apply_tunnel_plan(..., peer_endpoint_override = punched_reflexive_or_relay)` on tunnel-upgrade. commonware's TCP dialer is **untouched**. Justify why this is right: the mesh is already key-authenticated end-to-end (so any path is safe) and already frontable; folding STUN/punch into the TCP transport would duplicate the entry plane and entangle two independently-shippable layers.
- **Corollary (verified stub to fix):** `Reach::Coordinated` hints must be **split out** of `reach_entries()`/the TCP `bootstrappers` (config.rs ~337–354) and handed to the `NatClient`. Today they are dialed as TCP mesh peers at the coordinator's UDP address — a no-op-at-best. Note the alternative (dial the coordinator over TCP as a mesh forwarder) is explicitly **out** per the design's non-goals and the two-relay separation.

- [ ] **Step 3: Write §3 — The specific integration points (file-anchored checklist)**

Enumerate, each with the file and the exact crate API to call:

1. **Add deps.** `bin/node/Cargo.toml` gains `nat-traversal` + `wireguard-effect` (workspace). Decide: inline in `bin/node`, or a thin new `crates/system/reachability` orchestrator that `bin/node` owns the config plumbing for. Recommend the orchestrator crate (keeps `bin/node` lean, unit-testable against `FakeWireGuardEffect` + `SimNat`), but note `bin/node` already owns the `Reach`/`CoordRef` types, so the split has a seam to design.
2. **Route `Coordinated` hints.** `bin/node/src/config.rs` — add a `coordinator_refs() -> Vec<(coord_addr, coord_key)>` beside `reach_entries()` and stop pushing `Coordinated` into the TCP bootstrapper list.
3. **Boot the client.** At mesh boot (`bin/node/src/main.rs` ~2641, alongside `Network::new`/`network.start()`), construct `NatClient::bind_multi(node_key, coord_addrs)`; `discover_reflexive_failover(per_try)`; publish the reflexive into `EndpointAdvertisementV1.wireguard_endpoint` with a monotonic nonce; `readvertise(nonce+1)` on rebind (mirrors `wireguard_upgrade::MeshView::verify`'s dup rule, which Slice 3's `AdvertBook` already models).
4. **Rendezvous + punch.** `register()`, then per data-peer `lookup(peer)` / `recv_punch_sync()` to learn the peer's reflexive, and `send_punch_to`/`recv_punch_from(expected)` for the coordinator-timed simultaneous open.
5. **Bring up the tunnel.** On a validated `wireguard_upgrade` `TunnelInstallPlan` (from `validate_upgrade`), call `wireguard_effect::apply_tunnel_plan(&mut DefguardWireGuardEffect, ifname, priv_key_b64, listen_endpoint, &plan, Some(punched_reflexive))`. This is the **first-ever** `WGApi::configure_interface` call in the workspace.
6. **Relay fallback.** After bounded hole-punch retries + a signed `DirectDialFailureEvidenceV1`, `request_relay(peer) -> (session, relay_addr)`; re-apply the plan with `peer_endpoint_override = Some(relay_addr)`.
7. **Identity + runtime plumbing.** Map the node's ed25519 signer public key → `nat_traversal::NodeKey([u8;32])`. `NatClient` is plain tokio; the node runs on commonware's `tokio::Runner` — place the client on a dedicated task/thread, mirroring the app-surface thread pattern at `main.rs` ~2577–2607 (which already runs plain axum/tokio alongside the runner).

- [ ] **Step 4: Write §4 — The coordinator-side relay-bind fix**

Document the verified `run_coordinator` behavior: relay-splice sockets bind on the coordinator socket's IP, so `--listen 0.0.0.0:3478` produces undialable `0.0.0.0:<port>` relay grants. Options for a follow-on: (a) require the operator to bind the public IP (documented today in the recipe), or (b) teach the coordinator to learn its public reflexive (it already observes peers' sources) and emit *that* as the relay-grant IP. STUN/rendezvous/punch are unaffected — scope the fix to the relay path only.

- [ ] **Step 5: Write §5 — Why the real run needs the user's infra**

Be concrete: the CI sim-NAT suite (Slice 3) fakes NAT deterministically and uses `FakeWireGuardEffect` because CI has **no** WireGuard userspace runtime and no real NAT. The acceptance §2 demo therefore requires (a) two hosts behind **real, distinct** NATs (cone vs symmetric behavior cannot be faked cross-machine — it decides punch-vs-relay), (b) a **public VPS** for the coordinator with the relay-bind caveat handled, (c) real WireGuard (`DefguardWireGuardEffect`, userspace or kernel) on A and B, and (d) the Ducktape-2 live-join rig extended (per the `live-join-ducktape-2` memory: v2/v3 invite format, NAT-hairpin gotcha, 2-validator-quorum teardown caveat). None of these live in CI; they are the user's boxes.

- [ ] **Step 6: Write §6 — Scope + handoff summary**

A tight closing: this slice ships docs + config + a deploy proof, and does **not** wire the node (kept out to keep the gate cheap and avoid the red node-bin clippy). The node wiring is the enumerated §3 checklist — a self-contained follow-on ("Slice 5 — node reachability wiring"). Estimate the shape (deps + config split + one orchestrator + effect call), and note the acceptance status: §1 done (Slice 3), §3 recipe done (this slice), §2 blocked on §3-checklist + the user's infra.

- [ ] **Step 7: Commit**

```bash
git add docs/deploy/private-cutover-integration-gap.md
git commit -m "docs(deploy): private-cutover integration-gap assessment — the node-wiring handoff"
```

---

### Task 5: Close the loop in the design spec's Acceptance section

Make the design of record point at the two new narrative docs and state the honest status, so a reader of the spec can see that Slice 4 shipped the recipe (real) and drew the works/not-wired line, and that acceptance §2 remains blocked on node wiring + infra.

**Files:**
- Modify: `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`

- [ ] **Step 1: Edit the §"Acceptance" items to cross-link, without overclaiming**

Under item 2 (cross-machine demo), append: *"Procedure + honest status: `docs/deploy/cross-machine-zero-exposure-runbook.md` (every step tagged). Blocked on node wiring — see `docs/deploy/private-cutover-integration-gap.md`; the CI sim-NAT suite (Slice 3) proves the logic."* Under item 3 (real coordinator), append: *"Deployment recipe: `docs/deploy/coordinator.md` + `ops/coordinator/` (systemd unit + Dockerfile); the `coordinator --listen` invocation is regression-proven by `bin/coordinator/tests/deploy_smoke.rs`. Note: a v3 invite can point at the coordinator, but the node does not consume `Coordinated` hints as a reachability path yet — gap doc."* Do **not** mark §2 done; do **not** claim the tunnel.

- [ ] **Step 2: (Optional) add a one-line "Slice 4 shipped" note under §"Epic decomposition"**

Next to the Slice 4 bullet, note it landed as docs + config + a deploy proof, with node wiring deferred to a follow-on, linking the gap doc. Keep it one line.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md
git commit -m "docs(spec): link Slice 4 deploy recipe + runbook + gap doc from the Acceptance section"
```

---

### Task 6: Final gate + epic-readiness note

Run the exact merge gate and record the honest epic status, so whoever merges Slice 4 (and later the epic) knows precisely what is and isn't satisfied.

- [ ] **Step 1: Run the merge gate**

```bash
cargo build -p coordinator-bin && \
test -f docs/deploy/coordinator.md && \
test -f docs/deploy/cross-machine-zero-exposure-runbook.md && \
test -f docs/deploy/private-cutover-integration-gap.md && \
test -f ops/coordinator/ducktape-coordinator.service && \
test -f ops/coordinator/Dockerfile && \
echo GATE_OK
```
Expected: `GATE_OK`. (Also run `cargo test -p coordinator-bin` to keep the deploy proof green; it is not part of the gate but should pass.)

- [ ] **Step 2: Confirm no forbidden node coupling snuck in**

```bash
grep -rn 'nat-traversal\|wireguard-effect\|nat_traversal\|wireguard_effect' bin/node bin/noded || echo 'node still uncoupled (expected)'
```
Expected: `node still uncoupled (expected)` — this slice must not have wired the node.

- [ ] **Step 3: Write the epic-readiness status into the PR body (not a new file)**

When opening the Slice-4 PR into the epic branch, state the honest acceptance status:
- §1 CI simulated-NAT suite — **DONE** (Slice 3, `crates/system/nat-traversal/tests/simnat_ci.rs`).
- §3 real coordinator recipe — **DONE** (this slice; `coordinator --listen` proven by `deploy_smoke.rs`). Caveat: a v3 invite can *address* the coordinator, but the node does not yet *use* `Coordinated` hints — gap doc.
- §2 cross-machine zero-exposure demo — **NOT DONE**; runbook shipped with every step tagged; blocked on node wiring (gap doc §3 checklist) + the user's real infra (two NATs, a VPS, real WireGuard).
- Recommendation: the **epic** should not claim §2 on merge. Either land the follow-on node-wiring slice before the epic→`dev` integration PR, or merge the epic with §2 explicitly marked "mechanism proven + deployed helper; live tunnel pending node wiring + infra."

- [ ] **Step 4: (Handoff) leave a MEMORY-worthy summary for `done`**

At `done` time, record: *"Slice 4 shipped the coordinator deploy recipe (real, proven), a tagged cross-machine runbook, and the node-wiring gap doc. The reachability mechanism is CI-proven but UNWIRED into `bin/node`; `Coordinated` invite hints are still dialed as TCP bootstrappers (stub). Next: Slice 5 — node reachability wiring per `docs/deploy/private-cutover-integration-gap.md` §3."* (This is a handoff note for the human/`done` skill, not a committed file.)

---

## Appendix — quick reference for the implementer

- **Gate (nothing more):** `cargo build -p coordinator-bin` + the five `test -f` doc/artifact checks in "Global Constraints". Do **not** run workspace or node-bin clippy.
- **Real CLI the docs must match:** `coordinator --listen <SocketAddr>` (default `0.0.0.0:3478`), UDP only, stateless, no secret. Stderr announces `coordinator listening on {addr}`.
- **The one caveat that recurs everywhere:** `--listen 0.0.0.0:3478` makes relay grants undialable (`0.0.0.0:<port>`); bind the **public IP** for relay. STUN/rendezvous/punch don't care.
- **The verified stub:** `Reach::Coordinated` → `reach_entries()` → TCP `bootstrappers`; must be re-routed to a `NatClient` (gap doc §2/§3). Do not fix it in this slice.
- **Never claim** the zero-exposure tunnel works today. The mechanism is proven (`nat-traversal`, `wireguard-effect`, Slice 3 sim-NAT suite); the node is unwired.
