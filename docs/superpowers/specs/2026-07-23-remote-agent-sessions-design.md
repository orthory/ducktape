# Remote Agent Sessions — `ducktape agent pty --node <host> --cred <name>`

2026-07-23. Status: approved design, pre-plan.

## Goal

A network member uses `claude` / `codex` inside a Podman sandbox on **another
member's node**, with **their own credential**, from **their own terminal** —
real pty streaming, like ssh.

```
jess$ ducktape agent pty --node eddy --cred jess-fable-1 --cpu 1 --mem 2g
```

- jess's terminal drives the native TUI keystroke-by-keystroke.
- The sandbox runs on eddy's node, fenced by the existing Podman interactive
  path (`capability_host::InteractiveSession` — Direct backend refused).
- jess's Anthropic/OpenAI credential never touches eddy's box: eddy's broker
  holds no credential and forwards API traffic to the gateway co-hosted with
  jess's node (existing airlock execution/auth separation, body AEAD #702).

Compute and credential compose freely — the broker forwards to the owner's
gateway from wherever it runs, so every mix works: the inverse (eddy
**grants** his credential to jess, who runs on her own node with eddy's
subscription: `ducktape agent pty --cred eddy-fable-1`, no `--node`); host's
compute with the host's own granted credential (jess types, eddy's box and
eddy's subscription: `agent pty --node eddy --cred eddy-claude-1` — the
transport-trivial case, the broker hits its own node's loopback gateway); or
any third-party mix (jess on eddy's compute with bob's granted credential).

## Non-goals

- No TEE requirement for a self-hosted gateway (the owner may read their own
  credential; TEE remains the posture for third-party-hosted gateways).
- No conversation privacy from the host: the pty lives on the host node, so
  the host operator can read the terminal bytes. API request/response bodies
  stay AEAD-sealed end-to-end; the screen is the host's own pty. Stated, not
  fixed.
- No new public listener and no CLI-to-remote-node dialing. The only client
  surface stays "your own node"; everything cross-node rides the existing
  authenticated peer mesh.
- No admission policy beyond membership in v1: any network member may create
  a session on any node that has a sandbox image configured, bounded by the
  existing caps (`MAX_TERM_SESSIONS = 4`, 4 h wall clock, broker spend caps).
  A host allowlist is a follow-up config knob if a real network wants it.

## CLI surface

```
ducktape agent pty  [<provider>] [--node <name>] [--cred <name>] [--cpu <cores>] [--mem <size>]
ducktape agent sched [<provider>] --cred <name> [--node <name>] [--cpu] [--mem] -- "<prompt>"
ducktape user cred add <provider> [name]   # register a named credential (owner = this
                                           # account). <provider> = claude | codex picks
                                           # which vendor login to wrap and the registry
                                           # kind; [name] defaults to <display>-<provider>-<n>
ducktape user cred list [--json]
ducktape user cred remove <name>
ducktape user cred grant <name> <account>   # allow another account to use it
ducktape user cred revoke <name> <account>
```

- `agent pty` is the interactive verb, `agent sched` the headless one — a
  clean pair inside the `agent` family (no flat-alias exception needed).
- `<provider>` (`claude` | `codex`) is OPTIONAL when `--cred` is given: the
  registry entry's kind decides what to launch. Without `--cred` it is
  required; an explicit provider that contradicts the cred's kind is an error.
- Provider availability is preflighted: `agent pty`/`sched` create fails with
  a clear error when the executing node cannot launch that provider (no
  sandbox image, or the provider spec/binary is absent from the image), and
  `cred add` checks the vendor CLI exists on the owner's box (`which`) before
  wrapping its login.
- No `--node` = your own node (the existing local session path, unchanged).
- No `--cred` = the host node's own configured broker source (today's
  behavior); a cross-node session with no `--cred` is an error — a guest must
  bring a credential.
- `--cpu`/`--mem` map onto the existing `IsolationSpec` `cores`/`mem_gb` →
  Podman `--cpus`/`--memory`/`--memory-swap`. Absent flags = host defaults.
- `agent sched` is the headless variant: same targeting and credential
  resolution, one prompt on stdin via the existing headless `[invoke]` path,
  output lands in the run-output plane. Phase 3.

## Architecture

Three planes with different transports, each the one whose property the plane
actually needs:

### Data plane — real pty streaming (peer mesh, no consensus, no polling)

```
jess CLI ◀─local ws─▶ jess node ◀─peer stream─▶ eddy node ◀─pty master─▶ sandbox TUI
   raw mode            existing               new input lane;
   + resize            client surface         output fan-out already exists
```

- The CLI puts the local terminal in raw mode and attaches to its own node's
  existing ws client surface (the `TermInput`/`TermResize` ops and the
  `term:<id>` output topic already exist in `noded`'s stream plane).
- Output direction already crosses nodes: the term ring's peer-forwarder feed
  (`term_plane`) fans session output out to peer nodes. The CLI subscribes on
  its own node and rides that.
- Input direction is the new seam: `TermInput`/`TermResize` arriving at a node
  that does not own the pty are forwarded over the same peer stream plane to
  the owning node, which writes them to the pty master. Bytes, not commands —
  the `term-<id>` chat command lane and its projector are not involved in this
  path at all.
- The shared-session consensus command lane keeps existing independently for
  its own use case (attributed multi-party prompts); an interactive attach
  session does not create one.

### Control plane — session lifecycle (peer mesh request/response)

- `create` is a directed request from the guest's node to the host node over
  the peer mesh, carrying: provider (`claude`/`codex`), credential name,
  cpu/mem, and the guest's member identity (the mesh channel is already
  node-authenticated; the create is attributed to the requesting member).
- Host-side admission: sandbox image configured, session cap not exceeded,
  credential name resolvable. Failure returns an immediate error to the
  guest's terminal — an interactive session on an offline/unwilling host must
  fail now, not durably queue. That is why create is mesh, not consensus.
- `close` mirrors create; the existing lifecycle backstops (child exit → EIO
  → EOF, 4 h wall clock, kill-on-drop) are unchanged.
- Only the session creator's node may drive input for an attach session; the
  host enforces creator-only input on the forwarded lane.

### Credential plane — named registry + owner-co-hosted gateway (consensus + airlock)

Recon corrections baked in: `airlock` is an OFF-consensus library crate (no
module, no app-hash state); the on-chain home for the registry is the
**gateway module**, which already is the signed name→account→publisher-node
registry, and today's single co-hosted credential gateway already registers
itself there as `RouteName::named("airlock")` (`bin/node/src/airlock_serve.rs`,
`bin/node/src/boot/surfaces.rs`). Also there is no `BrokerKind::Airlock` —
airlock is a credential-SOURCE arm (`AnthropicAuth::Airlock` in
`capability-host/src/broker.rs`), and it currently exists for the Anthropic
lane only.

- The GATEWAY module gains credential records in consensus state:
  `name → { owner account, publisher node key, provider kind, gateway seal
  public key, granted accounts }`. Registration, removal, and grant/revoke
  are ordinary owner-signed messages (`ducktape user cred
  add/remove/grant/revoke`). The registry stores routing metadata + the seal
  PUBLIC key ONLY — never secret material. This is a consensus-module state
  change: app-hash flag day (fine, no live networks), wasm guest regen +
  schema-fingerprint/parity updates per the module-dev flow.
- Self-hosted gateway runs WITHOUT TEE: attestation becomes a mode. The
  existing gateway demands `--attest` and real configfs-tsm quotes; the
  self-host mode skips the quote entirely and the trust anchor is the
  owner-signed consensus record carrying the gateway's seal public key — the
  broker seals request bodies to the ON-CHAIN seal_pk instead of one read
  from quote REPORTDATA, so body AEAD end-to-end survives without hardware.
  The gateway's credential store becomes multi-named (today it is a single
  in-memory slot) and, in self-host mode, persists on the owner's own disk
  (owner's box, plaintext-by-design like node keys).
- The codex lane is NEW work: the gateway proxy + broker airlock arm are
  Anthropic-shaped today. Phase 1 lands the claude lane first to prove the
  path, then adds the OpenAI-Responses upstream + codex broker arm.
- `cred add` WRAPS the vendor's own login command instead of importing the
  personal login or reimplementing OAuth: it spawns `claude setup-token` /
  `codex login` on a local pty (`InteractiveSession::spawn_on_pty` is already
  backend-agnostic) with the tool's config home (`CLAUDE_CONFIG_DIR` /
  `CODEX_HOME`) pointed at a per-credential directory in the gateway store,
  parses the auth URL / device code from the stream to present it in
  ducktape's own flow, passes user input through, and the login artifact
  lands directly where the gateway serves from. Parse failure fails OPEN —
  the raw TUI is shown as-is. Each named credential is its own fresh login
  session with its own refresh token, so it never contends with the owner's
  personal local login. A TTY secret prompt covers the raw-API-key case.
  Only the metadata message goes on-chain.
- Phase 1 must LIVE-VALIDATE the broker's OAuth refresh path (constants are
  marked `PENDING live validation` in `broker.rs`) — a lent subscription is
  served long-term from the gateway's own store, so refresh must actually
  work.
- Use authorization: a session may name a credential only if its creator is
  the owner or a granted account. Enforced twice — the executing node checks
  committed registry state at create (fast error), and the owner's gateway is
  the FINAL enforcement, since the traffic terminates there. The gateway sees
  the mesh-authenticated executing node; when the creator runs on their own
  node (the lending case) that check is cryptographically solid, and when
  creator ≠ executing-node account (third-party mix) creator attribution is
  host-attested — as trustworthy as the host node. Creator-signed binding is
  a named follow-up, as are per-grantee spend caps; v1 caps are the broker's
  and gateway's global ones.
- The credential itself lives with its owner: a gateway co-hosted with the
  owner's node (the existing `airlock-gateway`, minus the TEE requirement for
  self-hosting) holds the actual API key/OAuth material.
- The executing node's broker holds no credential and forwards each API
  request to the owner's gateway; the existing body-AEAD + SSE streaming lane
  (#701/#702) carries the traffic. The transport ALREADY EXISTS end-to-end:
  `Gateway::remote(handle, via)` sends the request through the local node's
  gateway door, which resolves the signed route and pushes it over the
  overlay's `Service::Gateway` plane to the publisher node, whose gateway
  plane proxies to the co-hosted loopback gateway (`gateway_plane.rs` +
  `airlock_serve.rs`). When owner node == executing node it short-circuits to
  loopback. What is new is only RESOLUTION: `--cred <name>` → gateway-module
  credential record → owner's airlock route + seal_pk, plumbed per-run
  (today's `DUCKTAPE_AIRLOCK_*` env-at-boundary becomes a programmatic
  config built by the node from the resolved record).
- Spend remains bounded by the broker's existing request/byte caps, plus
  whatever caps the owner's gateway enforces — the credential owner always has
  the last word, because the traffic terminates at their gateway.

### Scheduling (`agent sched`) — consensus (phase 3)

- A headless run directed at a named node: the runs/saga plane already carries
  `pinned_assignee` on the wire; sched submits a run with the assignee pinned
  to the resolved node key, credential name and cpu/mem in the run spec.
- Durability is the point: the target node may be offline now and execute on
  reconnect. Output lands in the existing run-output ring; `ducktape agent
  sched` prints the run id, and the existing run-output surfaces view it.

## Name resolution

- `--node <name>` resolves via identity `display_name` to an account, then to
  that account's node key. If the account operates more than one node, the
  CLI errors listing candidates by node key; the flag also accepts a raw node
  key to disambiguate. No new naming registry.
- `--cred <name>` resolves via the gateway module's credential records. Names
  are unique network-wide at registration (first registration in consensus
  order wins).

## Security posture (summary)

| Threat | Answer |
|---|---|
| Guest reaches host's files/secrets | Podman fence; interactive spawn refuses Direct backend (existing) |
| Host reads guest's credential | Broker holds none; key material never leaves owner's gateway; bodies AEAD-sealed (existing) |
| Host reads guest's conversation | **Possible by construction** — host owns the pty. Stated non-goal. |
| Non-creator injects input | Creator-only input enforcement on the forwarded lane (new, host-side) |
| Resource abuse of host | Member-only create, session cap 4, 4 h wall clock, `--cpu`/`--mem` ceilings, broker spend caps (existing) |
| Ungranted member burns a lent credential | Owner-signed grant list checked at create + at the owner's gateway (final word) |
| Credential name squatting | First-registration-wins in consensus order; owner-signed removal |

## Phases

1. **Credential registry + co-hosted gateway** — gateway-module credential
   records (wasm regen + fingerprints), multi-named non-TEE gateway store,
   `user cred` CLI family (login-wrap pty), broker named resolution over the
   existing gateway plane; claude lane first, then codex. Foundation for both
   session kinds.
2. **Remote interactive sessions** — directed create/close over the peer
   mesh, the peer input lane (creator-only), and the CLI attach client
   (`ducktape agent pty`, raw mode + resize, `--node/--cred/--cpu/--mem`).
3. **`agent sched`** — pinned-assignee headless runs with the same resolution.

Each phase lands as its own PR(s) against `dev` from a worktree, per repo
delivery rules.

## Testing

- Unit: flag → `IsolationSpec` mapping; credential-name resolution (found /
  missing / duplicate registration); grant enforcement (owner / grantee /
  ungranted / revoked); creator-only input rejection; create admission
  failures (no image, cap exceeded, unknown cred).
- Cluster e2e (real-socket lane): two nodes — guest creates a session on the
  host node, drives a scripted child over the forwarded input lane, observes
  the echoed bytes on the guest node's `term:<id>` topic, closes, and the
  host's container is reaped. Synchronize on stream events, never on sleeps
  (house rule).
- Broker forward: existing airlock gateway tests extend with a named-registry
  resolution case; the AEAD/SSE lanes already have coverage (#701/#702).
