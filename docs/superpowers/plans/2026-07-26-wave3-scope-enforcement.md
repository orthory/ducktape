# Wave 3, step 1: make `grant.scopes` actually enforced

- **Date:** 2026-07-26
- **Status:** proposal — investigation complete, nothing implemented
- **Base read:** `origin/dev` @ `18564f1a4` + PR #818 (`feat/airlock-plug`,
  worktree `.worktree/feat-airlock-plug` @ `03039cb20`). Every file:line below
  was read in that tree.
- **Depends on:** the per-grant bearer-token plan (sibling agent). §4 states the
  exact interface this step needs from it.
- **Predecessors:** `2026-07-25-services-extraction.md` (wave 1),
  `2026-07-25-service-daemons.md` (wave 2).

## The finding, restated precisely

`grant.scopes` is written once and read never.

- Minted: `bin/node/src/services.rs:895-899` copies the hello's `scopes` into
  the `ServiceGrant` at `commit_enable`.
- Declared: `bin/node/src/services.rs:1050 scopes_for()` — a closed match on
  `Daemon` returning `saga.runs`, `credential.lent`, `gateway.credentials`,
  `gateway.routes`.
- Rendered: `render_enable_summary` (`services.rs:567`, painted RED on the
  consent screen) and `render_status` (`services.rs:521`).
- Read to gate something: **nowhere.** `grep -rn "grant_for\|\.scopes"` across
  `bin/` and `crates/` returns exactly two consumers of a grant, and neither
  looks at `scopes`:
  - `bin/node/src/validator/announce.rs:76` — reads `grant.capabilities` to
    decide what the node announces on chain.
  - `bin/node/src/config/resolve.rs:177` — `grant_for(...).is_some()`, presence
    only, to decide whether a `[sandbox]` table yields a compute backend.

So a grant today is a consent *record*, not a consent *boundary*. The user
reviews a red list of scopes and approves it; nothing on any transport ever
consults the result.

## The ceiling, stated before anything else

Enforcing scopes at the node's transports is worth exactly as much as the
answer to "what else can the caller already do?" — and today the answer is
"everything, without asking".

1. **Every `ducktape service run <kind>` boots by calling `config::resolve`,
   which loads the node's ed25519 PRIVATE key.** `bin/node/src/config/resolve.rs:23`
   (`pub signer: ed25519::PrivateKey`), loaded at `:291`, handed to all three
   daemons at `bin/node/src/services.rs:1016-1032`. A daemon holding `node.key`
   does not need `/v1/submit` — it can sign frames itself.
2. **The node's whole `/v1` surface is unauthenticated by design.** The only
   middleware is `origin_guard::guard` (`bin/noded/src/lib.rs:659`), and it
   refuses only requests that *carry* an `Origin` header not on an
   allowlist that is empty by default (`origin_guard.rs:74-97`). A request with
   no `Origin` — every non-browser client — passes untouched. The file says so
   itself (`origin_guard.rs:25-27`): "a local process can already read
   `user.key` off the disk, so that is not a boundary this file can hold."
3. **No daemon presents any identifier over HTTP.** Verified in both client
   layers: `bin/node/src/node_http.rs:61,99` and `bin/noded/src/node_link.rs:125-134`
   build bare `reqwest` requests; the only header set anywhere is a content type
   (`node_link.rs:104-107`). No `default_headers` on the client built at
   `node_link.rs:48-52`.
4. **`/v1/admin/*` is merged by default.** `AdminExposure::Loopback` is
   `#[default]` (`bin/noded/src/admin.rs:91`) and loopback peers are trusted with
   no PoP (`admin.rs:12-17`, `require_loopback`). So any local process reaches
   `POST /v1/admin/shutdown` and `POST /v1/admin/module-code/stage` today.
5. **`gateway.routes` is not a node surface at all.** Airlock exercises it with
   a local file write — `crate::gateway_routes::register(&workspace, ...)` at
   `bin/node/src/airlock.rs:90` → `gateway_routes.rs:216-231`. There is no route
   to gate.

**Therefore: the boundary wave 3 can actually draw is the workspace, not the
network.** A scoped caller is one that was launched *without* the node's
workspace directory. Everything below is written to that. A plan that gates
`/v1` while still handing a third-party daemon `--config` and a workspace path
is the same theater as the decorative scope, one layer up.

---

## 1. Attack-surface inventory

Counts: **50 HTTP route registrations** (43 public + 5 admin + 2 browser-gateway),
**20 websocket message kinds** (8 client frames + 7 topic families + 5 side-plane
control ops), **4 service-link commands**, **6 workspace filesystem objects**.

Every row was read. "no grant today" is the column that matters: it is what a
local process with zero consent can do right now.

### 1a. HTTP — the public router (`bin/noded/src/lib.rs:530-642`)

Gate on all 43: `origin_guard::guard` only → **an `Origin`-less caller reaches
every one of them with no grant.**

| route | what it lets a caller do | covered by an existing scope? |
|---|---|---|
| `POST /v1/submit` (`lib.rs:531`, handler `:677`) | forge a consensus op into **any** module, signed by the node's key; caller supplies a free-form `origin` attribution string (`:680-684`). Also stages the op payload into the blobstore, fetchable by op hash (`:702`) | partly — `saga.runs` gestures at it, unqualified by module |
| `POST /v1/submit/frame` (`:535`) | submit an already-signed frame; identity is inside the frame, not borrowed from the node | no |
| `POST /v1/query` (`:536`) | read committed state of **any** module — gateway credential records, identity bindings, saga state, chat | partly — `gateway.credentials`, `credential.lent` |
| `GET /v1/status`, `/v1/peers`, `/v1/blocks` (`:537-539`) | node identity, height, peer set, block bodies | no |
| `GET /v1/index/status`, `/v1/index/{module}/ops`, `/v1/index/{module}/scan`, `POST /v1/index/{module}/view` (`:542-547`) | the derived read model of every module | no |
| `GET /metrics` (`:549`) | full Prometheus exposition | no |
| `POST /v1/log-filter` (`:550`) | **rewrite the live tracing filter of the node process** | no |
| `GET /v1/ws`, `/v1/call/ws`, `/v1/presence/ws` (`:551-553`) | upgrade → §1c | no |
| `POST /v1/gateway/proxy` (`:554`) | proxy a request out through the node's overlay gateway lane | no |
| `GET /v1/gateway/browser` (`:560`) | the browser-gateway base URL (compute reads this as the credential `via` — `compute/mod.rs:335`) | no |
| `POST /v1/files/blob`, `GET /v1/files/blob/{digest}` (`:561-567`) | write arbitrary bytes into the node-local blobstore; **read any blob by digest — including every submitted op's payload** | no |
| `POST /v1/files/{stage,commit,pin,watch}` (`:574-583`) | duckfs **writes**: they encode the duckfs wire server-side and thread it through the same submit actor (`:568-573`) — consensus writes wearing a wrapper | no |
| `GET /v1/files/{stat,ls,read,find,grep,history,refs,diff,has-chunks}` (`:584-603`) | read the whole network filesystem | no |
| `PUT/GET/DELETE /v1/files/object/{*path}` (`:593`) | S3-shaped single-change commit / read / rm | no |
| `POST /v1/term/sessions`, `POST /v1/term/sessions/{id}/close` (`:611-612`) | **create an interactive pty session** on this node or direct one to a mesh peer, naming a credential (`term.rs:1113-1132`); close any session | `term.sessions` names the plane, but no daemon calls this route — the CLI/app do |
| `POST /v1/services/hello`, `GET /v1/services` (`:617-618`) | occupy a kind in the signaling catalog; read every signaling daemon's declared offers/scopes | n/a — deliberately unprivileged (`noded/services.rs:413-428`) |
| `POST /v1/fs/workspaces`, `POST /v1/fs/workspaces/{id}/commit`, `DELETE /v1/fs/workspaces/{id}` (`:619-627`) | create / commit / delete managed checkouts under the injected root | no |
| `GET /forge/{repo}/info/refs`, `POST /forge/{repo}/git-upload-pack` (`:628`, `:639`) | clone any forge repo | no |
| `POST /forge/{repo}/git-receive-pack` (`:635`) | **push to any forge repo** (ref-CAS) | no |

### 1b. HTTP — admin + browser gateway

| route | what it lets a caller do | gate |
|---|---|---|
| `GET /v1/admin/ping` (`admin.rs:385`) | authenticated liveness | loopback-trust by default |
| `POST /v1/admin/shutdown` (`:386`) | **stop the node** | loopback-trust by default |
| `GET /v1/admin/logs/tail` (`:387`) | drain the node's log ring | loopback-trust by default |
| `POST /v1/admin/module-code/stage` (`:391`) | **ingest a wasm artifact and fan it out to members** | loopback-trust by default |
| `GET /v1/admin/module-code/{digest}` (`:395`) | staging status | loopback-trust by default |
| `POST /.duck/ws-token` (`gateway_http.rs:394`) | mint a 30 s single-use ws capability token, bound to an origin the **caller supplies in its own body** (`:616-623`) | separate loopback listener, **no `origin_guard`** |
| `GET /.duck/ws/{token}` (`:395`) | bridge a live ws to a resolved overlay publisher | token TTL + origin pin, with an `Origin: null` bypass (`gateway_ws_token.rs:87`) |

The admin namespace is the sharpest item in the whole inventory: today a
third-party daemon can shut its node down and stage module code, and no scope
name exists to describe that.

### 1c. Websocket — `/v1/ws` (`bin/noded/src/stream.rs`)

Upgrade at `lib.rs:922` performs no check; `stream_session` (`stream.rs:775`)
enters its loop with no principal.

**Client frames** — one `match` at `stream.rs:797-858` over `ClientMsg`
(`stream.rs:55-153`), 8 variants:

| frame | what it lets a caller do | existing check |
|---|---|---|
| `subscribe` / `unsubscribe` | join any topic below (cap 64/conn) | none |
| `term_input` (`:1130`) | write raw keystrokes into any pty, local or forwarded to the host node | `term_entitled` — **circular**: `topics.contains_key("term:<id>")` (`:1098`), and subscribing is unconditional, so a connection self-grants by subscribing first |
| `term_resize` (`:1183`) | resize that pty | same circular gate |
| `term_command` (`:1228`) | submit a command line into a shared session's ordered lane with a caller-chosen `origin` attribution | same circular gate |
| `run_output` (`:1068`) | inject a line into **any** run's output ring, broadcast to every overlay peer | **none, deliberately** (`stream.rs:101-110`); only shape checks (64-hex id, 16 KiB line) |
| `service_attach` (`:974`) | **become this node's interactive plane** — §1d | the only real authn on the socket: `kind == "agent"`, build equality, node-wide link token, single holder |
| `agent_event` (`:1015`) | append pty output to any session ring / end a session | requires a successful attach on the same connection |

**Topics** — `prepare_topic` (`stream.rs:1281-1365`), 7 families, **none gated**:

| topic | what it reads |
|---|---|
| `module:<id>` (`:1286`) | every committed op of any indexed module, decoded |
| `files:watch` (`:1300`) | the same, pinned to `files` |
| `logs` (`:1308`) | the node's entire 4096-line log ring |
| `run-output:<id>` (`:1315`) | any run's stdout/stderr tail |
| `term-cmd:<session>` (`:1328`) | the ordered command log — `text`, of which `term.rs:111` says "can carry secrets — never logged" |
| `term:<session>` (`:1344`) | raw pty bytes of any session, local **or remote-hosted** (`term.rs:185` `append_remote`); full scrollback via `resume` |
| `metrics` (`:1359`) | full Prometheus exposition, re-sampled per heartbeat |

**Side planes**, both unauthenticated past a liveness/shape check:
`/v1/call/ws` (`call.rs:202`) — inject mic/camera frames into a huddle, set the
fan-out peer set, and **displace the incumbent session** for a channel
(`call.rs:100,317`); `/v1/presence/ws` (`call.rs:355`) — read and write live
caret positions on any page.

### 1d. The service link (transport C)

The highest-value surface in the tree. Node → daemon push, 4 command kinds
(`crates/services/agent/src/wire.rs:33-47`): `TermCreate` (carrying the
node-resolved **lent credential record**, `term.rs:923-929`), `TermInput`,
`TermResize`, `TermClose`.

Claimed by `ServiceAttach`; admitted by `take_service_link`
(`stream.rs:974-1008`) on three predicates: `kind != AGENT_KIND` refuse
(`:986`), `build != build_identity()` refuse (`:994`, `None` fails closed),
`terminals.attach(token)` (`:1006` → `term.rs:684-690`), constant-time compare
against `service-link.token`.

That token is **one node-wide secret minted fresh each boot**
(`noded/services.rs:137-148`), written 0600. Holding it *is* the interactive
plane, for every session and every lent credential — the file's own doc says so
(`services.rs:128-132`).

### 1e. Workspace filesystem (transport D — no chokepoint exists, none possible)

| object | what holding it grants |
|---|---|
| `node.key` (via `config::resolve`, `resolve.rs:291`) | the node's consensus identity — sign anything |
| `service-link.token` | become the interactive plane (§1d) |
| `services.toml` | the grants themselves; written by `save()` with plain `std::fs::write` (`services.rs:284`) — **not** 0600, unlike `mint_link_token` |
| `gateway-routes.json` | publish any loopback port under any of the account's route names (`airlock.rs:90`) |
| `node.toml` | listen addresses, sandbox config, storage root |
| `storage_dir` | blobstore, index, per-service podman roots |

---

## 2. Surface → scope

### 2a. All four existing names are wrong, and they are wrong the same way

Each names a *purpose*, not a surface. Checked against the verified call
inventory:

- `saga.runs` — compute submits to the `saga` module (`compute/intake.rs:205,382`,
  `compute/mod.rs:141`) **and** the `runs` module (`agent_provision/session.rs:257`)
  **and** posts signed frames (`session.rs:233`) **and** writes duckfs
  (`agent_provision/duckfs.rs:52,192`) **and** pushes forge
  (`agent_provision/forge.rs:889`). One name, five authorities, three of them
  unmentioned.
- `credential.lent` — on compute it is four ordinary committed queries
  (`compute/cred.rs:65,78,92,108`: `gateway.Credential`, `saga.Get`,
  `identity.OfNode`, `gateway.Registrations`). On agent it is not a call at all —
  the node *pushes* the record down the link. One name, two mechanisms, opposite
  directions.
- `gateway.credentials` — airlock's single `/v1/query` on the `gateway` module
  (`airlock.rs:168`). This is `query:gateway` and nothing more.
- `gateway.routes` — a local file write (`airlock.rs:90`). No transport, so no
  chokepoint, so unenforceable by construction.

### 2b. The replacement: five names, two parameterized on the existing module registry

Partitioned by **which authority the call spends**, which is the only axis a
refusal can be written against. No new taxonomy: the module id is already a
first-class name (`store.module_ids()`, `stream.rs:1288`).

| scope | covers |
|---|---|
| `query:<module>` | `POST /v1/query`; `GET /v1/index/{module}/{ops,scan}`; `POST /v1/index/{module}/view`; the duckfs **read** wrappers (`stat/ls/read/find/grep/history/refs/diff/has-chunks`, `object` GET) as `query:files`; `GET /forge/*/info/refs` + `git-upload-pack` as `query:forge`; ws topics `module:<id>` and `files:watch` |
| `submit:<module>` | `POST /v1/submit` (target = the request's module); `POST /v1/submit/frame` (target decoded from the frame); duckfs **writes** (`stage/commit/pin/watch`, `object` PUT/DELETE) as `submit:files`; `/v1/fs/workspaces*` as `submit:files`; `POST /forge/*/git-receive-pack` as `submit:forge` |
| `blob` | `POST /v1/files/blob`, `GET /v1/files/blob/{digest}` — the node-local content-addressed store, which is where every submitted op's payload lands (`lib.rs:702`) |
| `term.link` | `ServiceAttach` + `AgentEvent` — hold the interactive plane and publish into it. Replaces the `kind == AGENT_KIND` check as the discriminant |
| `run.output` | the ws `run_output` publish frame |

**Everything not in that table is unreachable by a scoped caller. No name, no
access, no exception.** That covers `/v1/admin/*`, `/v1/log-filter`,
`/v1/gateway/proxy`, `/v1/call/ws`, `/v1/presence/ws`, `POST /v1/term/sessions`,
`term_input`/`term_resize`/`term_command`, and the `logs` / `metrics` /
`term:<id>` / `term-cmd:<id>` / `run-output:<id>` topics. Deny-by-default costs
zero names and is the whole point.

Three surfaces stay ungated on purpose, and the reason is written down rather
than assumed:

- `POST /v1/services/hello`, `GET /v1/services` — signaling is pre-consent by
  construction; a grant cannot gate the request that asks for one.
- `GET /v1/status`, `/v1/peers`, `/v1/blocks`, `/v1/gateway/browser` — public
  chain/topology facts a peer learns anyway. Compute reads `gateway/browser`
  (`compute/mod.rs:335`); inventing a scope for a public base URL is the exact
  bloat this section exists to refuse.
- The workspace filesystem — §1e, no chokepoint. Bounded by not handing a
  third-party daemon the workspace, not by a scope.

`gateway.routes` is **deleted, not reserved.** A reserved name for a route that
does not exist is a decorative scope, which is the defect being fixed. When the
first third-party serving plug needs to publish a loopback port, that step adds
`POST /v1/gateway/routes` and mints the name with it (§5, step 7).

### 2c. What the three shipped daemons actually need

Derived from every verified call site, not from `scopes_for`:

- **agent**: `term.link`. That is the entire list — it makes no `/v1` call
  beyond the shared hello, subscribes to no topic, submits nothing, holds no
  keypair (`agent/mod.rs:43`, `agent/link.rs:45-201`).
- **airlock**: `query:gateway`. Also the entire list (`airlock.rs:168` is its
  only node call). Its route registration is a file write and stays outside
  scope enforcement.
- **compute**: `query:saga`, `query:gateway`, `query:identity`, `query:files`,
  `query:forge`, `submit:saga`, `submit:runs`, `submit:files`, `submit:forge`,
  `blob`, `run.output`. Eleven. `scopes_for` currently declares two.

That gap — 2 declared vs 11 real — is the single most useful output of this
investigation, and it is why step 1 below ships before any enforcement.

---

## 3. Where the check goes

### Transport A (HTTP) — one chokepoint, two tiers, and the reason for two

**The chokepoint exists**: `.layer(from_fn(origin_guard::guard))` at
`lib.rs:659` already wraps public + admin as one unit. A sibling `scope_guard`
goes at the same seam.

It must be `route_layer`, not `layer` — for the reason `admin.rs:404-406`
already documents (a `layer` also wraps the merged fallback, turning every
unmatched path into a gate refusal instead of a 404), and because the required
scope is a function of the **matched route pattern**, not the raw path.
`axum::extract::MatchedPath` is populated by the router before the route's
service runs, so a `route_layer` middleware can read it. *Verify this at
implementation time* — it is the one mechanical assumption in this plan I could
not confirm by reading ducktape code, only by reading the axum contract.

The two tiers are forced by data, not by taste. `/v1/submit` and `/v1/query`
name their module **in the body**, and middleware cannot see a body without
buffering it — which `admin.rs:44-48` already refused to do for
`module-code/stage`, for a reason that has not changed.

So: one decide-fn, one enum, one `match`, no `_` arm:

```rust
/// what the matched route requires of a scoped caller.
enum Requirement {
    /// no scope names this route; a scoped caller may never reach it.
    Denied,
    /// pre-consent or public-fact; any caller passes.
    Open,
    /// the route alone determines the scope.
    Fixed(Scope),
    /// the module is in the body; the handler checks it (4 call sites).
    FromTarget,
}

fn requirement(route: &str) -> Requirement   // one match over MatchedPath
```

Tier 2 is exactly four handlers — `submit`, `submit_frame`, `query`,
`index_view`/`index_ops`/`index_scan` share one target-decode step — and each
calls one shared `require(&scopes, Scope::Submit(module))`. Four call sites is
not "sprinkled": it is the irreducible set where the authority is
data-dependent, and each one sits at the line where the target first becomes
known. The middleware still owns identification and the deny half; the handler
owns only the module argument.

### Transport B (ws) — one chokepoint, plus one in-seam refactor

`stream_session`'s `match serde_json::from_str::<ClientMsg>` at
`stream.rs:797-858` is already the single dispatch. The check is one function
`fn frame_scope(msg: &ClientMsg) -> Requirement` with a `match` over the 8
variants (no `_`, so a new frame fails the build until it is routed), evaluated
before the arm delegates.

Topic subscription needs a second, inner check — `prepare_topic`
(`stream.rs:1281-1365`) is where a topic name becomes a subscription. It is
currently an `if let Some(x) = topic.strip_prefix(...)` ladder over 7 prefixes,
which is a house-rule wart independent of this work. **Labeled in-seam
mechanical refactor**: parse the topic once into a `Topic` discriminant, then
one `match` decides both the `TopicState` and the required scope. Its existing
`Result<_, ServerFrame>` return already carries refusals, so the refusal path
costs nothing new.

The circular `term_entitled` (`stream.rs:1098`) is **deleted**, not layered
over. Once frames are scope-gated, "subscribed therefore entitled" is a second,
weaker discriminant for the same decision — dual-path, which the repo forbids.
Note the consequence honestly: for un-tokened callers (the CLI driving a pty)
this *removes* a check rather than adding one, so it must land in the same PR as
the topic gate, never before it.

### Transport C (service link) — the chokepoint already exists

`take_service_link` (`stream.rs:974`) is a single decide-fn returning
`Result<_, &'static str>`. It gains one predicate (`term.link` in the presented
grant's scopes) and **loses** one: `kind != AGENT_KIND` (`:986`) becomes dead
once scopes are the discriminant, and keeping both would be a ladder over two
loosely-related facts. Its `&'static str` errors become a typed
`ScopeRefusal` so the reason token is derived, not typed by hand — the
`HelloRefusal` shape (`noded/services.rs:204-246`).

### Transport D (filesystem) — no chokepoint, and saying so is part of the plan

Nothing in-process can gate a `read(2)` by a process running as the same user.
The only fix is a launch contract: a third-party daemon is started with a base
URL and a token, never with `--config` or a workspace path. That belongs to the
token plan and the `service run` surface, not here, but this plan is void
without it.

### The refusal type

One enum, mirroring `HelloRefusal` exactly (`noded/services.rs:191-246`) so
`reason()`/`status()`/`message()` are derived from the variant and cannot drift:

| variant | `reason()` | `status()` |
|---|---|---|
| `Unscoped` | `unscoped_caller` | 401 |
| `UnknownGrant` | `grant_unknown` | 401 |
| `ScopeMissing` | `scope_not_granted` | 403 |
| `RouteDenied` | `route_not_scopable` | 403 |

Logging: a refusal is per-request and locally drivable in a loop, so an
unconditional `warn!` is a log-ring DoS — the same hazard `error_response`
(`lib.rs:483-511`) already latches for. **Reuse `crate::log::Latch`**
(`lib.rs:499`), keyed by `reason` and never by path or token. Never log the
token, the URI, or the module argument of a refused call.

---

## 4. What this step needs from the per-grant token plan

Interface only. This step does not care how tokens are stored, delivered, or
rotated.

1. **One resolver on `NodeHandle`**, and nothing else:
   ```rust
   fn caller(&self, presented: &str) -> Option<GrantedCaller>;
   struct GrantedCaller { instance: [u8; 32], kind: String, scopes: ScopeSet }
   ```
   Constant-time compare — `noded::services::token_matches`
   (`noded/services.rs:177`) already exists; do not write a second one.
2. **One token per grant, minted at `commit_enable`** (`services.rs:877`), dying
   with `disable`. Same consent-epoch rule the instance id already follows
   (`services.rs:222-238`): re-enable mints fresh, so a retired token resolves to
   `UnknownGrant` rather than to a stale scope set.
3. **Storage must be 0600.** `save()` currently uses plain `std::fs::write`
   (`services.rs:284`); `mint_link_token` already has the right helper
   (`noded/services.rs:151-161`). Putting a secret in a world-readable
   `services.toml` would be a regression against a file that is presently only
   consent metadata.
4. **One header name for HTTP**, decided by the token plan. It must be a header,
   **not** a path or query parameter — the logging doctrine forbids logging URIs
   precisely because `/.duck/ws/{token}` already carries a capability in a path.
5. **The ws lane reuses the existing field**: `ServiceAttach.token`
   (`stream.rs:139-148`) carries the per-grant token instead of the node-wide
   one, and `service-link.token` is **deleted** — one node-wide secret whose
   holder becomes the entire interactive plane is the thing being replaced, not
   a fallback to keep beside it.
6. **The one question only the token plan can answer** — and both plans are
   blocked on it: *what happens to a caller that presents no token?*

   - If un-tokened callers keep full authority, a third-party daemon simply
     omits the header and enforcement buys nothing. That is not a boundary.
   - So un-tokened must be `Unscoped` → refused, and every first-party caller
     gains a credential.

   The cost of that is smaller than it looks and is enumerated in §5 step 5:
   the CLI already resolves the workspace for every verb, so its credential can
   be a file only the owner can read; the Iced app makes **no** HTTP call at all
   (`grep -rn "node_http\|NodeLink\|reqwest\|ws://" app/src` returns nothing —
   it reaches the node in-process), so it is not a caller here. The real cost is
   that `bin/node` opens `reqwest::blocking::Client::new()` ad hoc in at least
   seven places (`node_http.rs:61,99`, `userkey_cli.rs:818`,
   `agent_cli.rs:181,204,399,463`) instead of going through `node_http` — so
   step 5's first move is consolidating to one client constructor, which is
   worth doing regardless.

---

## 5. Sequencing and blast radius

Each step is a PR. Steps 1 and 2 are independent; 3-6 are strictly ordered.

**Step 1 — truthful scopes, still unenforced.** Replace the four decorative
names with `Scope` (the five in §2b, parse/render round-trip), and rewrite
`scopes_for` (`services.rs:1050`) to the real lists from §2c. Delete
`gateway.routes`. Blast radius: **none** — nothing reads scopes, so this cannot
break a daemon. What it changes is that the red list on the consent screen stops
being a lie: compute's grant goes from 2 tokens to 11, and a user who approves
it is approving what compute does. Ship this even if the rest slips.
Check: one unit test asserting each daemon's declared set equals the set written
down here, so a future call site added without a scope fails a review that has
something to fail against.

**Step 2 — the token seam.** Sibling agent's plan. Not this step's work; §4 is
the contract.

**Step 3 — service link (transport C).** `take_service_link` requires
`term.link`; the `kind == AGENT_KIND` predicate is deleted; errors become
`ScopeRefusal`. Blast radius: **agent only, and it keeps working** — its grant
carries `term.link` after step 1. This is the smallest real enforcement in the
tree and the right place to prove the shape.
Check: the daemon attaches and is admitted with `term.link`; the same attach
without it receives a `scope_not_granted` error frame. The test waits on the
frame, not on a duration.

**Step 4 — ws frames and topics (transport B).** `frame_scope` over the 8
`ClientMsg` variants; the `prepare_topic` prefix ladder becomes one `Topic`
discriminant + scope decision (labeled in-seam refactor); `term_entitled` is
deleted in the same PR. Blast radius: **compute's `run_output` publish**
(`compute/link.rs:118`) now needs `run.output` — it has it after step 1. The CLI
and the app subscribe un-tokened; under §4.6's answer they must present a
credential, which is why step 5 cannot trail far behind.

**Step 5 — HTTP middleware, the deny half (transport A tier 1). This is the
expensive one.** `scope_guard` as a `route_layer`, the `Requirement` match over
every route in §1a/§1b, and — the actual cost — every first-party HTTP caller
gains a credential: consolidate the seven ad-hoc `reqwest` constructors onto one
in `node_http`, thread the header through `NodeLink` (`node_link.rs:48`),
`duckfs_client::http::HttpNode` (`crates/duckfs/client/src/http.rs:35`), and the
forge git remote (`http.extraHeader`). Blast radius: **every CLI verb, every
duckfs operation, and forge push**, all at once. Nothing else in the plan comes
close.

**Step 6 — HTTP module targets (tier 2).** Four handlers call one shared
`require`. Small, mechanical, and only meaningful after step 5. Blast radius:
compute's submit/query legs, all covered by step 1's list.

**Step 7 — only when a third-party serving plug exists.** `POST /v1/gateway/routes`
and re-mint `gateway.routes` with it. Not built now: nothing needs it, and a
name without a route is what this whole document is fixing.

### Honest cost

- Steps 1, 3: half a day each. Small, self-contained, real value on their own.
- Steps 4, 6: about a day each.
- Step 5: **several days, and it is the one that can go wrong.** It is not the
  middleware — that is a hundred lines. It is that "un-tokened is refused" turns
  the node's `/v1` surface from trusted-local into authenticated, which touches
  every client in the tree and every QA recipe that curls a node. Cost it as its
  own campaign, not as a step.
- Cross-cutting: root hash untouched, no consensus wire change, no module
  change. Nothing here is a flag day for the network — only for local clients.

## What I could not substantiate

- **`MatchedPath` availability under `route_layer`** (§3, transport A). Read from
  the axum contract, not from ducktape code. If it is not populated, tier 1 falls
  back to matching on `req.uri().path()` prefixes, which is worse (it
  re-implements routing) and should be treated as a reason to reconsider the
  middleware shape, not as a detail to patch.
- **Whether any non-first-party client already dials `/v1`** beyond the CLI,
  the daemons, duckfs, and git. I enumerated every `reqwest` constructor in
  `bin/` and `crates/` and found none outside those, but an out-of-tree script
  or QA recipe would still break at step 5 and would not appear in any grep.
- **Airlock's `/attestation`, `/session`, `/credential`** endpoints
  (`crates/airlock/src/client.rs:40-53`) and `POST /v1/run-action`
  (`agent_provision/session.rs:179`) are surfaces the *daemons themselves host*
  for their sandboxed runs, not node surfaces a daemon reaches. They are out of
  scope for grant enforcement and are named here only so a later reader does not
  mistake their absence for an oversight.
