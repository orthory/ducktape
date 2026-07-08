# Call test-bed (`ops/callbed`)

A reproducible **two-validator network that carries a real call**, with a driver
that **self-verifies audio AND video actually cross the mesh**. This fills a gap:
the repo could peer two nodes (`bin/node/examples/demo-invite.sh`) OR expose an
app HTTP surface, but nothing stood up two peered nodes *with* the `/v1/call/ws`
surface and proved call media flows between them.

## Why this exists

The huddle call path is: app webview → `/v1/call/ws` → node call hub
(`bin/node/src/voice.rs`) → data-plane datagrams over the authenticated TCP mesh
(`CHANNEL_VOICE`/`CHANNEL_VIDEO`) → remote hub → playout. The node's own tests
wire two hubs through *in-process channels* — they never exercise the **real
mesh**. This bed does, on two separate `ducktape-node` processes/containers.

**A single node cannot host a real call**: the hub fans media out by *node key*
and excludes self, so two webviews on one node produce an empty recipient set.
You need two peered nodes, one member each. (This is also why `ops/fleet.sh`,
which seeds a *solo* workspace per worktree, can't do it.)

## What's verified

Verified green **in-container via `docker compose`** (and independently on the
host). The `driver` exits `0` only when all of these cross the real mesh:

- **audio**, both directions — a synthesized tone plays out at **RMS ≈ 5490**
  vs **0** for silence;
- **video**, both directions — a synthetic multi-fragment frame fragments across
  `Service::Video`/`CHANNEL_VIDEO` and reassembles **byte-exact** on the far node;
- **control** — `peerBeacon` presence frames cross both ways.

The nodes reach a live 2-of-2 quorum (`height ≥ 1`) before the driver runs, so
the result is deterministic. **Scope:** no mic/camera needed — the driver
synthesizes PCM and (opaque) encoded-frame bytes. The video test proves the
TRANSPORT (fragment / reassemble / mesh routing); it does **not** test VP8
encode/decode, which is browser-side (WebCodecs) and out of scope for a
headless bed.

## Run it

```bash
# from the repo root, on a docker-capable box:
docker compose -f ops/callbed/docker-compose.yml build
docker compose -f ops/callbed/docker-compose.yml run --rm driver
```

`bootstrap` runs the offline ceremony (`init → invite → join → admit → invite →
join`) and writes `node0`/`node1` configs to a shared volume; the two nodes boot
and reach a live 2-of-2 quorum; `driver` then synthesizes tones + camera frames
on each node's call session and asserts they arrive on the other.

Do **not** use `up --abort-on-container-exit`: the one-shot `bootstrap` exits
first and would tear the whole stack down before the nodes start.

**Expected tail:**

```
A -> B audio crossed real mesh: YES ✓  (base=0 tone=~5490)
B -> A audio crossed real mesh: YES ✓  (base=0 tone=~5490)
A -> B video crossed real mesh: YES ✓  (byte-exact multi-fragment reassembly)
B -> A video crossed real mesh: YES ✓
```

The `driver` service's exit code is the test result (`0` = all crossed).
`node0`'s surface is also published on the host at `127.0.0.1:8080`
(`/v1/status`, `/v1/call/ws`); `node1` at `127.0.0.1:8081`.

Teardown: `docker compose -f ops/callbed/docker-compose.yml down -v`.

## Pieces

| File | Role |
|------|------|
| `Dockerfile.node` | builds `ducktape-node`, ships it + the scripts + curl |
| `bootstrap.sh` | offline peering ceremony → `node0`/`node1` configs in `/shared` |
| `node-entry.sh` | waits for its config, runs the validator |
| `docker-compose.yml` | `bootstrap` (one-shot) → `node0` + `node1` → `driver` |
| `call-driver.ts` | bun ws client: synth tone + camera frame; assert audio RMS + byte-exact video reassembly; verdict |
| `virtual-mic.sh` | PulseAudio null-sink → a capture source (for the app profile) |

Nodes bind `0.0.0.0` but **advertise their compose service name** (`node0`,
`node1`) so peers dial by DNS on the compose network.

## The app profile (follow-up, not yet wired)

To drive the **real desktop app** against this network instead of the scripted
driver, two things are needed that this bed documents but doesn't yet ship as a
service:

1. **A virtual mic.** Headless WebKitGTK has no audio input, so
   `getUserMedia({audio})` fails. Run `virtual-mic.sh` inside the app container
   before launching the app (optionally `VMIC_TONE=1` to feed a tone), and point
   the app process at the same `PULSE_SERVER`.
2. **The app owns its own member node.** The desktop app *manages* a local node
   (via `DUCKTAPE_NODE_BIN` + a seeded workspace registry), it does not attach to
   a remote one. So an app container must `join` this network as its **own**
   member node (extend `bootstrap.sh` to emit a member invite), seed a member
   workspace pointing at that node (see `ops/fleet.sh:seed_workspace`), then
   launch the app + `x11vnc` (see `skills/tauri-debug`). Two app containers =
   two members who can huddle.

**Caveat — video rendering vs. transport:** the video *transport* is proven here
(fragment/reassemble over the mesh). What WebKitGTK can't do is *render* it: the
webview has no WebCodecs VP8, so `supportsVideoCalls()` returns false and the
in-app call degrades to roster + audio (video tiles need a Chromium-based
webview). So in the app on this bed, "working call" = **audio + roster +
presence**, even though the mesh carries video fine.
