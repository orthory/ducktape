# Wave 3, step 1: the caller-authorization object — a bearer token bound to a grant

- **Date:** 2026-07-26
- **Status:** proposal (not implemented, not approved)
- **Base:** `origin/dev` `18564f1a4` + open PR #818 (`feat/airlock-plug`)
- **Wave:** 3 of `docs/superpowers/plans/2026-07-25-service-daemons.md` — making a
  THIRD-PARTY daemon safe to run
- **Scope:** the token only. Grant-scope enforcement at the node's privileged
  surfaces is a sibling step; section 5 is the interface it consumes.

Design facts this plan does not contradict: a service has no keypair, no chain
presence and no off-node addressability; the node is the unit of accountability;
the instance id is a NAME minted by the node at `enable`, not a credential.

---

## 1. What the current token actually guarantees

`service-link.token` — `bin/noded/src/services.rs:123-186`, minted at
`bin/node/src/boot/surfaces.rs:207`, read at `bin/node/src/agent/link.rs:106`,
verified at `bin/noded/src/term.rs:684-701` through
`bin/noded/src/stream.rs:974-1008`.

**What it does guarantee, precisely:**

1. **One bit, node-wide: "this process can read the node's workspace."** It is
   32 random bytes, hex, written `create_new` + `0o600` beside `node.toml`
   (`write_owner_only`, `services.rs:150-161`). Presenting it proves filesystem
   access to the node's own directory — the same bar `user.key` and
   `node.key` already set. That is a real raise over "can dial loopback",
   which is what `origin_guard` alone leaves (`bin/noded/src/origin_guard.rs:20-27`
   states outright that a token is not a boundary that file can hold).
2. **Fail-closed when unmintable.** A mint failure yields `link_token: None`
   and `TerminalSessions::attach` then refuses every attach
   (`term.rs:685-688`, reason `no_link_token`). Same when the term plane is not
   wired at all (`!sync_only && http_listen.is_some()` gates the mint).
3. **Per-boot freshness.** Minted on every boot, and `link.rs` re-reads it on
   every attach rather than latching, so a node restart evicts any holder of a
   stale copy. That is the only rotation in the system today.
4. **Singleton, first-attach-wins.** `attach` refuses a second claimant while
   one holds the link (`term.rs:693-696`), so a local impersonator cannot
   displace a live daemon and start receiving `TermCreate` commands.
5. **Constant-time comparison** (`token_matches`, `services.rs:177-186`).

**What it does NOT guarantee — the gap this step exists to close:**

- **It names nobody.** Verification answers `bool`, never "which grant". There
  is no caller identity to enforce against, which is exactly why the sibling
  enforcement step has nothing to hold onto today.
- **It is all-or-nothing.** Holding it means BECOMING the node's interactive
  plane: every `TermCreate`, lent-credential records included
  (`stream.rs:126-146` says so explicitly). A compute daemon that reads it —
  and it can, same uid, same directory — is one ws frame away from being the
  agent plane.
- **It gates exactly one surface.** `/v1/services/hello`, `/v1/services`,
  `/v1/submit`, `/v1/submit/frame`, `/v1/query`, `/v1/files/*`,
  `/v1/term/sessions`, `/forge/*` all sit on the same `public` router with
  `origin_guard` and nothing else (`bin/noded/src/lib.rs:530-660`). The compute
  and airlock daemons present no credential at all — `NodeLink`
  (`bin/noded/src/node_link.rs`) sends no auth header on any call.
- **It survives revocation.** `disable` does not touch it (section 4).
- **It carries no scopes.** `grant.scopes` and `hello.scopes` are read by
  exactly three call sites, all of them renderers or the copy into the record:
  `bin/node/src/services.rs:373, 386, 520, 567, 898` and
  `bin/noded/src/services.rs:392`. Nothing in the tree makes an authorization
  decision from a scope. The consent screen paints them red; the code ignores
  them.

So the honest one-line summary: **today's token proves workspace access and
confers the whole interactive plane, for as long as the node stays up.** It is
a capability with no subject, no scope and no revocation.

---

## 2. The replacement: one token per grant

The grant record already IS the consent object (`ServiceGrant`,
`bin/node/src/services.rs:113-132`). Give it a bearer secret and the node can
answer "which grant is this caller acting under" from the same file that
answers "is this kind still enabled".

### 2.1 Shape

- **Secret:** 32 random bytes from `OsRng`, lowercase hex — the same width and
  encoding as the link token it replaces. Reuse `write_owner_only`.
- **Verifier in the record:** a new `ServiceGrant` field

  ```rust
  /// sha256("ducktape/service-token/v1\0" ‖ token), lowercase hex. The record
  /// carries the VERIFIER, never the secret: services.toml is written with
  /// default permissions (`save`, services.rs:284) and is meant to be read.
  pub token_digest: String,
  ```

  `Services::validate` checks it is 64 lowercase hex, beside the existing
  `instance` and `nonce` checks (`services.rs:181-195`).
- **Secret on disk:** `<workspace>/service-<kind>.token`, `0600`, one file per
  grant, beside `node.toml` exactly where the node-wide token lives today.

Storing the digest rather than the secret is what makes revocation total: the
record is the single source of truth, so a leftover token file authorizes
nothing (section 4).

### 2.2 Minting — at `enable`, with the instance id

`commit_enable` (`bin/node/src/services.rs:877-919`) already mints the nonce and
the instance id inside one function. The token joins them: same call, same
random draw point, same write. Decide-then-write is preserved — `plan_enable`
still writes nothing; `commit_enable` performs three effects in a fixed order:

1. write `service-<kind>.token` (0600, `create_new`) — the secret first, so a
   crash between steps leaves an unreferenced file, never a grant nobody can use;
2. insert the grant (with `token_digest`) and `save()` services.toml;
3. log `service enabled` with kind + display id (unchanged; never the token).

The token is minted by the same act that mints the id, from the same consent,
which is what makes "the id and the token rotate together" true by construction
rather than by discipline.

### 2.3 How a daemon finds ITS token

The filename is a pure function of the kind the daemon was launched as
(`ducktape service run compute` → `service-compute.token`). No discovery, no
handshake, no registry.

This is safe because the kind grammar is already a validated trust boundary at
every point that can produce one — `kind_is_well_formed` in
`bin/noded/src/services.rs:253-259` (hello boundary),
`bin/node/src/services.rs:209-215` (`plan_enable` and `Services::validate`).
`[a-z0-9-]{1,32}` admits no `/`, no `.`, no NUL, so the path cannot traverse and
the preimage cannot be made ambiguous. That property already exists and is
already tested (`a_malformed_hello_is_refused_at_the_boundary` covers
`compute/../etc` and `a\0b`).

**Never latch, read at the point of use.** `link.rs:104-105` already argues this
for the attach path; the same rule now covers the hello beat and every
`NodeLink` call. It is what makes enable-at-run work with no extra signaling: a
daemon that started before its grant existed picks the token up on the next
heartbeat (≤10 s, `HEARTBEAT = HELLO_TTL/3`) with no restart and no state
machine.

> `ponytail:` one `read_to_string` of a 65-byte file per privileged call. The
> named upgrade is an mtime-keyed cache — the shape PR #818 already uses for the
> airlock credential store — added only if it ever shows in a profile.

### 2.4 Presentation

- **HTTP:** header `x-ducktape-service-token`, matching the repo's
  `x-ducktape-admin-*` convention (`bin/noded/src/admin.rs:76-80`). Not
  `Authorization`, so no proxy or client library forwards it by accident.
  Threaded through `NodeLink` in one place: `NodeLink::with_service_token(path)`
  and one `.header(...)` in `post_json` / `submit_frame`.
- **ws:** the existing `ClientMsg::ServiceAttach` frame's `token` field
  (`stream.rs:139-146`), which already exists and already carries a secret.
- **hello:** a new `Hello.token: Option<String>` — `None` for a kind with no
  grant, which is the pre-consent state the plug-and-play order requires
  (section 6).

Never logged, never in a path or query string, never in an `error` body.

### 2.5 Two fields that stop being sent

`ServiceAttach { kind, build, token }` becomes `ServiceAttach { token }`.

The token resolves to a grant, and a grant has a kind — so sending `kind`
alongside creates a disagreement path (frame says `agent`, token says `compute`)
that has to be adjudicated for no gain. `take_service_link` instead reads the
kind off the resolved grant. `build` goes for the reasons in section 6.

---

## 3. Verification

### 3.1 Where the grant set lives

`ServiceGrant`, `Services`, `load`, `save`, `mint_instance` and the kind grammar
**move from `bin/node/src/services.rs` to `noded::services`**, beside `Hello`,
`ServiceCatalog` and the token helpers that are already there.

This is forced, not preference: verification happens on the node's HTTP/ws
surface, which is `bin/noded`, and `bin/node` depends on `noded`, never the
reverse. `bin/node/src/services.rs:32` already re-exports the kind constants
FROM `noded::services`, so this finishes a split that is half done. The CLI
keeps the clap surface, the verbs, the renderers, `plan_enable`/`commit_enable`
and `EnablePlan`.

**This is a structural refactor (relocating code across a crate boundary), so
per the house rules it is ask-first. It is called out here as its own step
rather than bundled.**

### 3.2 The primitive

```rust
/// the workspace whose services.toml carries this node's grants. Carried on
/// `NodeHandle` beside `ServiceCatalog`. `None` — a handle with no workspace
/// (router tests, simnode) — refuses every token, the same fail-closed shape
/// `duckfs_workspaces` already uses for /v1/fs/workspaces.
pub struct ServiceGrants { workspace: PathBuf }

/// what a presented service token proves. Everything the node knows about the
/// caller, and nothing it does not: there is no principal beyond the grant.
pub struct CallerGrant {
    /// the granted kind — `compute`, `agent`, `airlock`.
    pub kind: String,
    /// the 32-byte instance id, lowercase hex: the container label
    /// (`io.ducktape.managed`), the audit key, the consent epoch.
    pub instance: String,
    /// the scopes the user reviewed at consent. Carried, NOT interpreted here —
    /// what a scope permits is the enforcement step's decision.
    pub scopes: Vec<String>,
}
```

```rust
impl ServiceGrants {
    /// Resolve a presented token to the grant it proves. THE decide-fn: writes
    /// nothing, logs nothing, holds no state, re-reads services.toml each call.
    pub fn authorize(&self, presented: Option<&str>) -> Result<CallerGrant, GrantRefusal>;
}
```

Implementation is four lines of decision: absent → `Absent`; `load()` fails →
`Unreadable`; `token_digest(presented)` matched against each grant's
`token_digest` with the existing constant-time `token_matches` → the grant, or
`Unknown`. Grants are one per kind and capped at a handful, so the scan is over
~3 entries.

### 3.3 The refusal

```rust
/// why a service token did not resolve to a grant. Typed rather than a string,
/// exactly like `HelloRefusal`: status, reason and message all derive from the
/// variant so they cannot drift.
pub enum GrantRefusal {
    /// no token was presented.
    Absent,
    /// a token was presented and no live grant matches it.
    Unknown,
    /// services.toml could not be read, so this node can decide nothing.
    Unreadable,
}
```

| variant | `reason()` | `status()` | when |
|---|---|---|---|
| `Absent` | `service_token_absent` | 401 | header/field missing or empty |
| `Unknown` | `service_token_unknown` | 403 | no grant carries this digest — **including a revoked one** |
| `Unreadable` | `service_grants_unreadable` | 503 | file corrupt, unreadable, or no workspace on the handle |

Statuses follow `admin.rs` (401 = no credential, 403 = credential rejected,
503 = node cannot serve the check), not `HelloRefusal`'s 400/409/503, because
this is a bearer gate and that is the neighbour it should read like.

**Revoked collapses into `Unknown` on purpose.** Distinguishing them would mean
remembering retired grants — durable state whose only consumer is an attacker
probing whether a token was ever real. The operator's diagnosis path is
`ducktape service status` (the kind is simply not enabled) plus the daemon's own
`grant_revoked` log line, not a richer refusal.

`Unreadable` logs once on transition, latched, the way
`CapabilityAnnouncer::granted` already latches `grant_unreadable`
(`bin/node/src/validator/announce.rs:75-97`) — a corrupt toml must not emit one
warn per request.

---

## 4. Revocation must actually revoke

### 4.1 What `disable` does today

`bin/node/src/services.rs:1249-1276`: removes the grant from services.toml,
saves, prints the retired display id, and prints "a running `service run <kind>`
keeps serving what it already holds". The prose is honest — that is the fix from
the earlier review — but the behaviour behind it is thin. The only thing that
actually stops is the **capability announce**: `CapabilityAnnouncer` re-reads
`services.toml` every drain tick and intersects it with the live hello
(`announce.rs:99-119`), so the tags retract within a tick.

Everything else survives `disable` today:

| survives | why |
|---|---|
| the agent daemon's ws command link | authorized once at attach; nothing re-checks |
| `service-link.token` | node-wide, per-boot, unrelated to any grant |
| every `NodeLink` call (`/v1/submit`, `/v1/query`, `/v1/files/*`) | ungated entirely |
| the daemon's containers | reaped by the daemon at ITS startup, never by `disable` |

### 4.2 What it must do

`disable` becomes: remove the record → `save()` → unlink
`service-<kind>.token` (best-effort tidy, **not** the authority) → print.

Revocation then propagates without any new machinery, because every check reads
the live record:

| surface | latency | mechanism |
|---|---|---|
| any privileged HTTP call | next request | `authorize` re-reads services.toml |
| the agent ws command link | ≤ 3 s | re-authorized on the ws heartbeat arm (§4.3) |
| the signaling catalog row | ≤ 30 s | `HELLO_TTL`; a granted-kind hello now needs a token |
| the capability announce | ≤ 1 drain tick | unchanged, already correct |

The unlink is deliberately not load-bearing. If it fails — permissions, a
crash between the two writes — the token still authorizes nothing, because
`authorize` matches against grant records and there is no record. That is the
whole reason the digest lives in the record instead of the secret living only in
the file.

### 4.3 The one open door: a live ws link

An already-attached connection is an open door that no per-request check
reaches. Close it on the tick that is already firing.

`stream_session` runs a `tokio::time::interval` at `HEARTBEAT_INTERVAL_MS`
(3 s, `stream.rs:20, 782`). On that arm, a connection that holds the link
re-runs `authorize` for the instance it recorded at attach; a refusal drops the
`AttachGuard`. Dropping it is already the fully-built "the daemon went away"
path — it detaches the link and ends every session with `term_ended`
(`term.rs:644-660, 703-719`), which is exactly the right behaviour for a
revoked grant, and it is already tested.

Cost: one file read per 3 s on at most one connection (`attach` is a
singleton). Named predicate `link_still_granted`; the check itself is pure and
unit-testable without I/O.

### 4.4 The daemon's side of a revocation

On `403 service_token_unknown` the daemon logs `reason = "grant_revoked"` at
`error` and **exits**. Not a serving↔signaling state machine: that would be new
dual-path state to hold a case systemd already handles. It restarts, re-signals,
finds no grant, and parks in the signal-only resting state `serve_kind` already
implements (`bin/node/src/services.rs:1004-1013`). Exiting is safe because there
is no transient 403 — an unreadable grant file answers 503, not 403.

### 4.5 What `disable` still does not do — stated, not hidden

**Containers keep running.** Reaping a service's containers means dialing that
service's private podman socket, which is the daemon's, and the daemon is a live
supervisor holding it. The wave-2 doc's "`disable compute` reaps exactly
compute's containers" is a separate piece of work and is not in this step.
`disable`'s printed message must say what is true:

> disabled compute; compute#deadbeef is retired (a re-enable mints a fresh id)
>   its link drops within seconds and every privileged call is now refused
>   its containers keep running until you stop the daemon

---

## 5. Lifecycle rules — and their consequences

| event | instance id | token | consequence |
|---|---|---|---|
| **node restart** | unchanged | unchanged | the daemon keeps working across a node restart with no re-attach dance. The node holds no token state at all — nothing to rebuild. |
| **daemon restart** | unchanged | unchanged | re-adopts its own `io.ducktape.managed=<kind>#<hex8>` containers, which is the second reason ids must survive restart. |
| **`disable`** | retired | dead the moment the record is gone | §4 |
| **re-`enable`** | **fresh** (fresh nonce → fresh preimage) | **fresh** | a new consent epoch. Any daemon still holding the old token is refused `service_token_unknown` and exits; its containers wear the old label and are no longer its own. |
| **upgrade / rebuild** | unchanged | unchanged | consent is not re-opened by a rebuild. This is why version and build are hello metadata and never enter the id preimage. |

**The consequence to state plainly: per-boot rotation is deleted.** Today a node
restart invalidates every holder of the link token. After this change the
token's lifetime is the grant's, so a leaked token is good until `disable`
instead of until the next restart.

That trade is right, and here is the argument rather than the assertion:

1. The id and the token are two halves of one grant. If the token rotated per
   boot and the id did not, the id would stop being the consent-epoch marker the
   design says it is, and "an upgrade does not re-open the consent boundary"
   would be false for the token.
2. Per-boot rotation was never a revocation mechanism — it could not be, since
   there was nothing to revoke. It was a lifetime bound on a secret that had no
   subject.
3. The compensating controls are strictly stronger: the token no longer confers
   the interactive plane to whoever holds it (it confers exactly one grant), and
   `disable` now actually revokes, which it did not before. A holder of a leaked
   compute token can submit ops as compute; it cannot become the agent plane.
4. Restarting the node is no longer the operator's revocation tool. `ducktape
   service disable <kind>` is, and it works in seconds. That is a better tool.

---

## 6. Interface for the enforcement step

Everything the sibling agent needs, and nothing that constrains their policy.

**One call.** `handle.grants().authorize(presented)` → `CallerGrant` or
`GrantRefusal`. It resolves the caller; it decides no policy. What a scope
permits, which routes require which scope, and whether an unlisted scope is a
refusal or a warning are entirely theirs.

**Two entry points onto that one call:**

1. **HTTP — an axum extractor.**

   ```rust
   /// a request that proved it holds a live service grant.
   pub struct ServiceCaller(pub CallerGrant);

   impl FromRequestParts<NodeHandle> for ServiceCaller {
       type Rejection = Response; // GrantRefusal, rendered
   }
   ```

   A handler that needs a grant takes `ServiceCaller` as an argument; a handler
   that does not take one is **visibly** ungated at its signature. That is
   preferable to a middleware here because the privileged surfaces are not a
   contiguous namespace — `/v1/submit` serves the CLI and the app as well as the
   compute daemon, so `admin.rs`'s `route_layer` shape (`admin.rs:383-403`) does
   not transfer. Extract per handler; keep the blanket layer out of it.

2. **ws — the raw call.** `ServiceAttach { token }` calls `authorize` directly
   in `take_service_link` (`stream.rs:974-1008`) and matches on the resolved
   `grant.kind`.

**The refusal wire**, identical on both, and identical in shape to the existing
hello refusal (`services.rs:437-451`):

```json
{ "error": "<operator-facing sentence>", "reason": "service_token_unknown" }
```

`reason` is the stable snake_case token from the table in §3.3, derived from the
variant exactly the way `HelloRefusal::reason()` derives its own.

**Three things the enforcement step should know before it designs:**

- **`grant.scopes` is currently attacker-suppliable.** The chain is
  `scopes_for(kind)` (daemon, `bin/node/src/services.rs:1050-1069`) →
  `hello.scopes` → catalog → `plan_enable` → `commit_enable` copies it into the
  record verbatim (`services.rs:896-899`). Any local process that occupies the
  kind in the catalog before consent chooses what the operator reviews and what
  the record stores. Section 7 closes the post-consent half; the pre-consent
  half is residual and the operator's review is the boundary. **A scope-based
  gate is only as good as that review** — worth pinning `scopes_for(kind)` as
  the authority for first-party kinds rather than trusting the record's copy.
- **`NodeLink` is the daemon-side chokepoint** — one `.header()` in `post_json`
  and one in `submit_frame` covers `/v1/submit`, `/v1/submit/frame`, `/v1/query`.
- **`/v1/files/*` has no seam.** `NodeLink::files()` returns a
  `duckfs_client::http::HttpNode` built from a bare base URL
  (`crates/duckfs/client/src/http.rs:26-35`), with no header hook anywhere in
  the type. Gating the files routes therefore costs a public API change on
  `HttpNode`. Budget it or leave the files routes ungated in the first pass —
  but decide it deliberately, do not discover it mid-implementation.

---

## 7. The build gate: verdict — DELETE it as a refusal

`bin/noded/src/services.rs:351` (`hello.build != mine → HelloRefusal::BuildMismatch`)
and its twin at `bin/noded/src/stream.rs:994-1001`.

I set out to refute the reasoning and could not. Two of the three premises
verify; one is overstated and I state it correctly below; and the code carries a
fourth argument nobody had named.

**(a) "hello confers nothing" — overstated, and the correction does not save the
gate.**

Scopes and needs are inert: nothing in the tree reads them for an authorization
decision (§1, verified call sites). But `capabilities` are **not** inert. The
announce is `grant.capabilities ∩ live-hello.capabilities`, re-evaluated every
drain tick (`announce.rs:99-119`), and the result is a committed
`CapabilityMsg::Announce`. So a hello does have a consensus-visible effect:
occupying a granted kind's catalog row lets a caller shrink an announce to
nothing, or hold one alive after the real daemon has died so that dispatch keeps
placing runs that will never execute. Both are denial of service, not privilege
gain — no hello causes anything to execute, and `daemon_for` returns `None` for
every kind this binary hosts no plane for (`bin/node/src/services.rs:43-50`).

The correction changes nothing for the gate, because a build stamp does not
authenticate the caller (premise b) and section 8's token does.

**(b) "a hostile local process can read the stamp anyway" — verified, and worse
than stated.** The process does not need to read the binary. It sends any hello,
and the node hands the stamp over in the refusal body:

```rust
HelloRefusal::BuildMismatch => format!(
    "this node runs build {}; restart the service daemon from the same build",
    build_identity().unwrap_or("unknown")
)                                          // services.rs:232-235
```

and `hello()` returns `refusal.message()` verbatim in the 409 body
(`services.rs:443-449`). The gate publishes its own secret to any unauthenticated
caller on the first wrong guess. It authenticates nobody.

**(c) "it excludes every third-party binary by construction" — verified.**
`build_identity()` is `option_env!("DUCKTAPE_BUILD")` (`services.rs:118-120`),
stamped by `bin/noded/build.rs` as `git rev-parse --short HEAD`, plus a
`DefaultHasher` of `git diff HEAD` when the tree is dirty. A third-party daemon
would have to hardcode the operator's exact commit — and, on a dirty tree, a
hash the build script's own comment admits is not stable across toolchains — at
ITS compile time. The gate does not raise the bar for a hostile local process
(it is one 409 away); it raises it to infinity for an honest third party. That
is precisely backwards, in the wave whose purpose is third-party daemons.

**(d) the argument nobody named: the gate is a live landmine on non-git
builds.** `build.rs`'s own doc comment claims that with git absent "the env var
is simply left unset and `build_identity` falls back to the package version"
(`build.rs:11-13`). It does not. `build_identity()` returns `None`, and `None`
FAILS CLOSED — `services.rs:346-348` refuses every hello and
`stream.rs:991-993` refuses every service link. So a source tarball, a vendored
build, or any Docker build without `.git` produces a node with **no compute, no
agent pty and no airlock**, whose only symptom is a 503. Documented behaviour
and actual behaviour disagree, in the fail-closed direction, on the most common
non-developer build path.

**What the gate genuinely protects, and what replaces it.**

I looked for the thing that would refute the deletion — a correctness hazard
only build equality catches. It is not there: the node↔daemon protocol is serde
JSON with a decode boundary on every frame. A skewed daemon's unknown command
is dropped with `malformed_command` (`agent/link.rs:184-192`), an undecodable
`ClientMsg` earns a `BadFrame` error (`stream.rs:849-857`), and an unknown hello
field is refused by `deny_unknown_fields`. Skew degrades to named, countable
refusals — never corruption. Under the no-compat doctrine "speak the current
protocol or be refused" is already enforced per frame, at the boundary where it
belongs.

What is really lost is a **diagnostic**: the loud "restart your daemon" for the
ordinary dev loop (edit, rebuild the node, yesterday's daemon still running).
That is worth keeping, and it does not need a refusal:

1. `Hello.build` stays on the wire as metadata; `Signaling` gains a `build`
   field; `service status` prints the daemon's build beside the node's.
2. The hello OK response carries the node's build
   (`{"ttl_secs":30,"build":"…"}`), so the daemon can name its own skew.
   Handing the stamp to a caller that reached the hello route is not a leak —
   §7(b) established the node already does exactly that.
3. `send_hello` warns on the transition into skew and infos on the transition
   out, latched the way `grant_unreadable` is — never once per beat.
4. `render_status`'s `EnabledAbsent` hint stops naming `build_mismatch` as a
   cause (`bin/node/src/services.rs:526-541`) and names the real ones.

**Code deleted:** the `hello.build != mine` check, the `build != mine` check in
`take_service_link`, `HelloRefusal::BuildMismatch`,
`HelloRefusal::BuildIdentityUnavailable`, `ServiceAttach.build`, and the
`build_gate_tests::a_hello_from_a_different_build_is_refused…` test.
`build_identity()` and `build.rs` stay — they produce the metadata, and with the
gate gone their `None` case becomes an honest "unknown build" instead of a
node-wide outage. The build.rs doc comment then describes what the code does.

---

## 8. Catalog spoofing

**Yes, one local process can overwrite another's entry, and yes it matters.**

`ServiceCatalog::admit` (`services.rs:342-380`) keys on `hello.kind` and
`entries.insert` — **last write wins**, unconditionally, for any caller that can
reach `POST /v1/services/hello`. The route sits on the `public` router behind
`origin_guard` only, and the module's own AUTH note says so: "Signaling is
deliberately unprivileged: an entry grants NOTHING" (`services.rs:413-428`).
That premise held when a hello really did grant nothing. It stopped holding when
the announce became `grant ∩ live hello`, because the live half is now this map.

Two concrete harms:

1. **Post-consent.** With `compute` granted and its daemon running, a hostile
   local process posts a `compute` hello with an empty capability list. The
   intersection empties; the node retracts its announce; no work is placed on a
   perfectly healthy compute node. Or the inverse: the daemon dies and the
   squatter keeps the tags alive, so dispatch keeps placing runs nothing will
   execute.
2. **Pre-consent.** A squatter occupies `compute` before the honest daemon
   starts. `plan_enable` reads the catalog (`services.rs:858-866`), the operator
   reviews the squatter's offered tags and scopes on the consent screen
   (`render_enable_summary`), and `commit_enable` records them. The grant is not
   *addressed* to the squatter — grants are per kind, and after this plan the
   token is written to a 0600 file, so the squatter gains no execution — but the
   operator consented to a description that was not the daemon's.

**What this design changes.** One rule, expressible as a single match on one
`Option`:

```
match grants.grant(hello.kind) {
    None          => admit unauthenticated,      // the pre-consent path
    Some(grant)   => require the presented token to resolve to `grant`,
}
```

- **Granted kind → authenticated hello.** Harm 1 is closed completely: only the
  grant holder can refresh, shrink or expire the row that feeds the announce.
- **Ungranted kind → unauthenticated hello, as today.** This is not an oversight;
  it is the plug-and-play order. A daemon signals *before* consent exists, so
  there is nothing for it to authenticate with. Harm 2 therefore stays open.

**The residual, stated rather than papered over.** A squatter can still poison
the consent screen for a kind nobody has enabled yet. It cannot make that
consent *do* anything: the grant's token goes to a 0600 file the squatter must
read the workspace to get, and the announce intersects the record against the
live hello — so once the honest daemon takes the row back, the poisoned
capabilities that the real daemon does not offer simply drop out. A poisoned
enable produces a visible symptom (announced tags narrower than the consent
screen showed), not a silent compromise. Squatting also requires *continuous*
re-signaling, since `HELLO_TTL` is 30 s.

Closing it properly needs a principal that exists before consent — which is what
"a service has no keypair" forecloses by design. It is the honest price of the
signal-then-consent order and should be named in the operator docs, not
engineered around.

---

## 9. Steps

1. **Move the grant record into `noded::services`.** Pure relocation, no
   behaviour change; root hash untouched. **Ask-first (structural).**
2. **Delete the build gate**, carry `build` as catalog metadata, add the node
   build to the hello OK body, skew warning latched on the daemon side. Ships
   alone so a bisect can attribute it.
3. **Mint the per-grant token**: `token_digest` on the record, the 0600
   `service-<kind>.token` file, `commit_enable`'s three ordered effects,
   `disable`'s unlink and honest message.
4. **Verify it**: `ServiceGrants` on `NodeHandle`, `authorize`, `GrantRefusal`,
   the `ServiceCaller` extractor, `take_service_link` on the token,
   `ServiceAttach { token }`, the ws heartbeat re-check.
5. **Present it**: `NodeLink::with_service_token`, hello carries it, daemon exits
   on `service_token_unknown`. **Delete `mint_link_token`, `read_link_token`,
   `LINK_TOKEN_FILE` and the `link_token` field in the same commit** — no dual
   read path, no fallback, per the no-legacy rule.

Steps 3-5 are one flag day for the node↔daemon link and should land together or
in immediate succession; there is no live network, so no staging is owed.

### Tests (event-driven, never timed)

- `authorize` round trip: minted token resolves to its grant; a wrong token,
  an absent one and a token whose grant was removed each give the right variant.
  Pure, no I/O beyond a `tempfile::tempdir`.
- **The revocation test that is the point of this step:** mint, authorize OK,
  `disable`, authorize refused `service_token_unknown` — with the token file
  deliberately left on disk, proving the record is the authority.
- Re-`enable` mints a different instance AND a different digest; the old token
  is refused. Extends `the_instance_id_survives_a_daemon_restart`.
- The ws link drops on revocation: `#[tokio::test(start_paused = true)]`
  advancing the heartbeat interval on tokio's virtual clock — a clock advance,
  not a sleep — and asserting the guard dropped and sessions ended.
- Hello standing: ungranted kind admitted without a token; granted kind refused
  without one and refused with another grant's; admitted with its own.
- A token never appears in any log line or any `error` body. Source-parsing lint
  test if the shape proves fragile.

### Gates

`cargo clippy -p noded -p node-bin --tests --no-deps`;
`cargo test -p noded --lib`; `cargo test -p node-bin --bin ducktape`;
`cargo check -p files --no-default-features`; root hash untouched (no
`crates/modules/` file is in scope).

---

## 10. Ceilings

- **Same-uid siblings.** `0600` separates a daemon from another *user*, not from
  a same-uid sibling process — and first-party daemons normally run as the same
  uid. The improvement is real but bounded: a compute daemon that reads
  `service-agent.token` still becomes the agent plane. Closing that needs a uid
  per service (systemd `DynamicUser`, or `User=` per unit), which is an operator
  deployment decision, not a protocol one. Today's node-wide token has the same
  ceiling and, additionally, hands over the interactive plane to anything that
  reads one file.
- **Pre-consent catalog squatting** — §8, structural, named not fixed.
- **Containers outlive `disable`** — §4.5, separate work.
- **`/v1/files/*` ungated** until `HttpNode` grows a header seam — §6.
- **A per-call file read** on the token and grant paths — `ponytail:` mtime cache
  is the named upgrade, the airlock store is the precedent.
