# Sentry Deployment Recipes (Phase 1)

How to front a Ducktape validator with a **sentry** so it never exposes a
public inbound port, while joiners still enter, sync, and promote through it.

This is the operator-facing companion to the design of record,
[Pluggable Network Entry — Reachability Plane](superpowers/specs/2026-07-04-pluggable-network-entry-design.md).
Read that for the *why* (authority vs. reachability, the trust model, the
roadmap to coordinator/private-cutover phases). This document is the *how*.

## Why this is safe

The mesh transport (commonware `authenticated::discovery`) is **encrypted and
key-authenticated end-to-end**. A dialer dials an address and expects a specific
`ed25519` public key; the handshake authenticates that key regardless of what
network path delivered the bytes. Therefore any box on the path is at most a
transparent ciphertext forwarder: it can drop or delay, but it **cannot read
content and cannot impersonate a peer**. A sentry is not a mesh member, is not a
validator, and never serves state — it is pure path.

Concretely, a bootstrap hint is `pubkey@addr` and the validator's `advertised`
address is fully independent of its real `listen` address
(`bin/node/src/config.rs`). Fronting is therefore *configuration only*: point
`advertised` (and joiners' hints) at the sentry, keep `listen` private.

## (A) Forward sentry (Cosmos-style)

The sentry listens on a **public** address and TCP-splices to the validator's
**private** `listen` address. Requires sentry→validator reachability (shared
private network, VPC, or a firewall exception).

```
joiner ──▶ sentry(public :443) ──▶ validator(listen 10.0.0.7:52200, private)
             (transparent TCP splice; ciphertext only)
```

Validator config:

```toml
listen     = "10.0.0.7:52200"          # private; reachable only from the sentry
advertised = "sentry.example.com:443"  # what peers dial
```

Invites / bootstrap hints handed to joiners carry the **validator's key** at the
**sentry's** address:

```
<validator_pubkey_hex>@sentry.example.com:443
```

The joiner dials the sentry, the sentry splices to the validator, and the
encrypted mesh handshake terminates at the validator through the pipe — the
joiner never learns (and never needs) the validator's private `listen`.

Realizations of the splice, cheapest first:

- **`nginx stream`** — `stream { server { listen 443; proxy_pass 10.0.0.7:52200; } }`
- **HAProxy TCP mode** — `mode tcp` frontend/backend.
- **A small Rust forwarder** — an accept loop that, per connection, dials the
  target and runs `std::io::copy` in both directions. (The Phase-1 regression
  test `bin/node/tests/sentry_e2e.rs` stands up exactly such a forwarder to
  prove a joiner enters through it.)

Run multiple sentries per validator for redundancy — each is just another
`pubkey@sentry_addr` hint in the bootstrap set. Sentries rotate without touching
the validator's key.

## (B) Reverse tunnel

The validator dials **out** to an edge and the edge becomes its public face. No
inbound port on the validator at all — ideal when the validator is behind NAT
with no port-forward.

```
validator ──(dials out)──▶ edge(public :443) ◀── joiner
   listen private            the "sentry address" is the tunnel edge
```

Tooling: `frp`, `rathole`, `ssh -R`, or a cloudflared-style tunnel. The edge's
public `host:port` is what goes into `advertised` and into joiners' hints,
exactly as in recipe (A) — from the mesh's point of view the two recipes are
identical ("dial this address, expect this key").

## Caveats

- **`allow_private_ips` coupling (forward sentry on a private network).** The
  node builds its mesh with the `local` discovery preset, which sets
  `allow_private_ips: true` (`bin/node/src/main.rs`, at the
  `discovery::Config::local` call ~line 1430; see the inline comment there). A
  **forward** sentry that splices from a private source IP (recipe A over a VPC)
  depends on this. Switching to a preset with `allow_private_ips: false` would
  make the validator's listener **reject the forwarded connection from a private
  source IP** as an anti-DoS measure. If you change presets, front the validator
  with a **public-IP** sentry or use a **reverse tunnel** (recipe B) instead.

- **DNS pinning at boot.** A DNS-named edge (`advertised = "sentry.example.com:443"`)
  is resolved **once at startup** (`resolve_one` in `bin/node/src/config.rs`) and
  pinned to that IP for the process lifetime. This is fine for a static A-record;
  if the edge's IP can change, restart the node after the record moves, or front
  it with a stable IP.

- **State-sync still terminates at a validator.** The sentry is pure path — it
  never serves state. A joiner's `--sync-only` source is chosen **by key**, and
  only a validator (never the sentry, which has no key in the descriptor) can be
  selected (`choose_sync_source` in `bin/node/src/config.rs`). State-sync flows
  *through* the sentry pipe but *terminates at* the validator behind it.

- **Availability dependency.** Any entry helper is an availability/censorship
  dependency **for new connections** only — established connections survive edge
  downtime via keepalive. Mitigate with multiple independent sentries/edges (the
  bootstrap set is a `Vec`), self-hosting, and falling back to a direct hint.

## Scope

Phase 1 ships **no consensus/production behavior change** — it converts an
already-working, configuration-only capability into a regression-guarded,
documented one (`bin/node/tests/sentry_e2e.rs`). The typed reach hint
(`Direct`/`Fronted`), coordinator/STUN rendezvous, and the private (WireGuard)
cutover are later phases; see the
[design of record](superpowers/specs/2026-07-04-pluggable-network-entry-design.md).
