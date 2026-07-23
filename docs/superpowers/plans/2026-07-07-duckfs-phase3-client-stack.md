# duckfs Phase 3 — Checkout/Commit Engine + `ducktape fs` CLI (issue #220)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.
>
> **STATUS: PLANNED.** Forks from `dev` AFTER PR #185 (and the Phase-2 stack) merges — assume the duckfs module, the noded duckfs HTTP surface, and the cluster e2e harness are on `dev`. Lands as its own PR to `dev`. If executed stacked on `feat/duckfs` instead, rebase when the predecessor merges.

**Goal:** Ship the client half of duckfs: a `duckfs-client` library crate (checkout/commit engine over a versioned `.duckfs/` index), a `ducktape-fs` CLI binary, the three missing node surfaces the engine needs (`/v1/files/refs`, `/v1/files/diff`, a `HasChunks` probe), and the sandbox-workspace RPC seam (`/v1/fs/workspaces`) — proven by a two-node cluster checkout→edit→commit→checkout-on-the-other-node e2e.

**Architecture:** The engine is a lib crate at `crates/system/duckfs-client` (blobstore precedent: small, workspace-versioned). Pure logic (index format, worktree scan/status, change planning, chunking/hashing) is separated from I/O; all node access goes through one small **synchronous** `NodeApi` trait so a colocated-odb fast path can slot in for Phase-4 FUSE (which is sync callback-driven) — Phase 3 ships exactly one implementation, `HttpNode` (reqwest blocking) over the noded HTTP surface. Clients never hand-frame ops: staging rides `POST /v1/files/stage`, commits ride `POST /v1/files/commit` (the forge smart-HTTP push handler in `bin/noded/src/lib.rs` is the exemplar standalone-client path). The workspace RPC is a thin wrapper over the same engine, driven through an in-daemon `NodeApi` adapter over the `NodeCommand` actor lane (no self-dial). The CLI is a hand-rolled-arg-dispatch binary (`bin/node`'s `parse_flags` style at `bin/node/src/main.rs:2420` — no clap anywhere in the workspace, keep it that way).

**Tech stack:** Rust workspace; `files` crate pure core (wire types, `objects::object_id`, `paths::canonical`); `bin/noded` axum router; `reqwest` (workspace dep, `blocking` feature added per-crate); `bin/node/tests/common` cluster harness.

**Spec:** `docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md` — §"Client stack", §"Determinism boundary", §"Caps", §"Testing" are binding.

## Global constraints

- Every commit: `git -c commit.gpgsign=false commit ...`, trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- **No mono-files:** explicit file layouts below are mandatory; ~600-line soft cap per file. `bin/noded/src/lib.rs` is already over cap — Task 2 extracts the duckfs surface rather than growing it.
- **Determinism boundary (spec):** nothing in this phase touches `execute → state → root()` except the additive `HasChunks` *query* (queries are not part of the root-hash; adding a variant is consensus-safe). The purity gate `cargo check -p files --no-default-features` stays green after every task.
- **Error-string contracts** the engine keys on (pin them in tests, never restate them loosely): `"files: conflict: <path> changed since base"` (`crates/apps/files/src/fs.rs:1024`), `"files: base snapshot not resolvable"` (`fs.rs:835-837`), `"files: chunk not available"` (`fs.rs:1015`). Over HTTP these arrive as 400 `{"error": <msg>}` (`bin/noded/src/lib.rs:465,1133`).
- Atomicity is non-negotiable: >`MAX_CHANGES_PER_COMMIT` (4096) changed paths **fails with a clear error before any op is submitted** — never split into multiple commits (spec: the commit is the atomic unit).
- Gates per task: `cargo test -p <touched crates>`; `cargo check --workspace`; `cargo fmt -- --check` on touched crates; `cargo clippy -p <touched crates> -- -D warnings` (note the repo's `--no-deps` reconciliation note from the Phase-2 backlog if host/dispatch/saga noise appears).
- Comment style: lowercase, explain constraints not mechanics.

## Task list

### Task 1: `FilesQuery::HasChunks` — the "which chunks are you missing" probe

The engine must stage only chunks the cluster lacks. No probe exists. Presence = staged (`refs.staging`) OR already in the odb — exactly the commit-time availability rule at `fs.rs:1009-1017` minus in-block pending. The reply is **advisory** (odb contents can differ per node between GC sweeps); the commit re-validates, so a stale answer costs one redundant stage or one clean rejection, never corruption.

**Files:**
- Modify: `crates/apps/files/src/wire.rs` — `FilesQuery::HasChunks { ids: Vec<DigestHex> }`; `FilesReply::HasChunks { present: Vec<bool> }` (order matches request); reuse `MAX_SYNC_IDS` (`wire.rs:63`, = 256) as the id cap.
- Modify: `crates/apps/files/src/queries.rs` — arm in the `query()` dispatch (`queries.rs:32-70`): reject `ids.len() > MAX_SYNC_IDS` with `"files: too many ids"`; reject non-hex ids; per id, `fs.refs_view().staging.contains_key(&id) || <odb has>` (in-crate access via `store_ref()`, like `diff` at `queries.rs:569`).
- Modify: `crates/apps/files/tests/queries_core.rs` — coverage.

**Interfaces:** Produces the probe Tasks 2/6 consume. Consumes nothing new.

- [ ] **Step 1 (RED):** in `queries_core.rs`, seed one *staged-only* chunk (putblob, commit_block), one *committed* chunk (inline commit), then query `HasChunks { ids: [staged, committed, absent] }`. Assert reply is `FilesReply::HasChunks { present: vec![true, true, false] }`. Fails to compile (no variant) — that is the red.
- [ ] **Step 2 (RED):** 257 ids → `Err` containing `"too many ids"`; a malformed hex id → `Err`.
- [ ] **Step 3:** implement wire variant + handler; green.
- [ ] **Step 4:** gates: `cargo test -p files`, `cargo check -p files --no-default-features`, fmt/clippy. Commit `feat(duckfs): HasChunks query — client staging probe`.

### Task 2: noded — route refs/diff/has-chunks; extract the duckfs surface into its own module

`FilesQuery::Refs`/`Diff` exist but are unrouted (router at `bin/noded/src/lib.rs:509-524` stops at history). Also `lib.rs` is 2185 lines — move the whole duckfs product surface (existing handlers `files_stage`…`files_history`, `lib.rs:1101-1450`) into a new file before adding three more routes.

**Files:**
- Create: `bin/noded/src/files_http.rs` — the moved handlers + param structs + `files_submit`/`files_query`/`files_query_error`/`wrong_reply` helpers, plus NEW: `GET /v1/files/refs` (→ `FilesQuery::Refs {}` → `Json(RefsInfo)`: `{head, pins, window_len}`), `GET /v1/files/diff?from=&to=&prefix=` (→ `Json({"entries": Vec<DiffEntry>})`), `GET /v1/files/has-chunks?ids=<comma-separated hex>` (→ `Json({"present": Vec<bool>})`; >256 ids or bad hex → 400 pass-through of the module rejection).
- Modify: `bin/noded/src/lib.rs` — `mod files_http;`, route registrations, delete the moved block. `files_submit` needs `pub(crate)` visibility on `NodeHandle::send` or stays reachable via crate-internal fn — keep helpers in `files_http.rs` with `pub(crate) use` where the workspace RPC (Task 9) will reuse them.
- Modify: `bin/noded/tests/router.rs` — fake-actor contract tests for the 3 new routes.
- Modify: `bin/noded/tests/daemon_e2e.rs` — extend the duckfs round-trip: refs before/after a commit (head None → Some), diff between the two snapshots, has-chunks flipping false→true across a stage.

**Interfaces:** Produces the complete HTTP read/probe surface `HttpNode` (Task 8) consumes. Consumes Task 1.

- [ ] **Step 1 (RED):** router.rs: `GET /v1/files/refs` against the current router → assert 200 with a `head` field; red because the route 404s today.
- [ ] **Step 2:** extraction commit first (pure move, zero behavior change; daemon_e2e still green) — `refactor(noded): extract the duckfs http surface into files_http.rs`.
- [ ] **Step 3:** add the three routes + handlers; router + daemon_e2e green. Pin one contract test: a files module rejection surfaces as 400 `{"error": "files: ..."}` **verbatim** (the engine's conflict taxonomy depends on this envelope).
- [ ] **Step 4:** gates: `cargo test -p noded`, `cargo check --workspace`, fmt/clippy. Commit `feat(duckfs): route refs/diff/has-chunks on the noded surface`.

### Task 3: `duckfs-client` crate skeleton — pure core (chunk ids + index v1)

**Files:**
- Modify: root `Cargo.toml` — member `"crates/system/duckfs-client"`, workspace dep `duckfs-client = { path = "crates/system/duckfs-client" }`.
- Create: `crates/system/duckfs-client/Cargo.toml` — package `duckfs-client`, `edition.workspace`/`version.workspace`; deps: `files = { workspace = true, default-features = false }` (pure core only: wire types, `objects`, `paths`), `serde`, `serde_json`, `sha2`, `base64`, `thiserror`, `reqwest = { workspace = true, features = ["blocking"] }`; dev-deps: `tempfile`, `files = { workspace = true }` (native, for the mock module), `sdk`, `futures`, `async-trait`.
- Create: `crates/system/duckfs-client/src/lib.rs` — crate doc (the determinism-boundary paragraph: this crate is OS-side, consensus never sees it) + re-exports.
- Create: `crates/system/duckfs-client/src/chunk.rs` — pure: `chunk_ids(bytes) -> Vec<ObjectId>` (1 MiB fixed via `files::CHUNK_SIZE`), `file_object_id(size, chunks, meta)` via `files::objects::{FileObj, object_id, Kind}` — byte-identical to what the module derives.
- Create: `crates/system/duckfs-client/src/index.rs` — index format **v1**: `{"version":1, "base_snapshot": Option<hex>, "prefix": String, "node": String, "entries": {path -> {object: hex, size, mtime_secs, mtime_nanos, exec, kind: file|symlink, meta}}}` stored at `<dir>/.duckfs/index.json`; load rejects unknown versions with a "re-checkout" remedy; save is `tmp → rename` atomic. `meta` is carried so `file_object_id` recomputes exactly (the file id preimage includes meta).
- Create: `crates/system/duckfs-client/tests/core.rs`.

**Interfaces:** Produces the hashing + index primitives everything downstream uses.

- [ ] **Step 1 (RED):** `tests/core.rs`: for a 2 MiB + 1 byte pattern buffer (the `large_file_e2e.rs` pattern fn), assert `chunk_ids` yields 3 ids equal to `object_id(Kind::Chunk, <each slice>)` and `file_object_id(..)` equals `object_id(Kind::File, FileObj{..}.encode())`. Red: crate doesn't exist.
- [ ] **Step 2 (RED):** index round-trip (save → load → equal); a `{"version":2}` file → `Err` whose message names re-checkout.
- [ ] **Step 3:** implement; green. `cargo check -p files --no-default-features` still green (nothing changed there — cheap sanity).
- [ ] **Step 4:** gates: `cargo test -p duckfs-client`, `cargo check --workspace`, fmt/clippy. Commit `feat(duckfs-client): crate skeleton — chunk hashing + .duckfs index v1`.

### Task 4: worktree scan + status (git index discipline)

**Files:**
- Create: `crates/system/duckfs-client/src/scan.rs` — walk the checkout dir (skip the `.duckfs` entry at the root), producing per-path `{size, mtime, exec, kind (file|symlink|dir)}`; relative paths joined with the index prefix.
- Create: `crates/system/duckfs-client/src/status.rs` — `status(dir) -> Status { added, modified, removed, clean }`: mtime(+nanos)+size equal to index → clean **unless racily clean** (entry mtime ≥ index file's own mtime → rehash — git's rule for coarse-granularity filesystems); size differs → modified without hashing; suspicion → rehash via `chunk.rs` and compare file object ids; symlink target changes compare the target string; exec-bit-only changes are `modified`.
- Create: `crates/system/duckfs-client/tests/status.rs`.

**Interfaces:** Produces `Status` for Task 6's planner and the CLI `status` verb. Consumes Task 3.

- [ ] **Step 1 (RED):** the racy-clean trap: write a file, record it in the index, then rewrite **same-size different content** and backdate its mtime (via `std::fs::File::set_times`) to the recorded value while making the index file appear no newer. Assert `status` reports it `modified`. A naive mtime+size fast path returns clean — that is the red.
- [ ] **Step 2 (RED):** added / removed / exec-flip / symlink-retarget cases; untouched big file is `clean`.
- [ ] **Step 3:** implement; green.
- [ ] **Step 4:** gates as Task 3. Commit `feat(duckfs-client): worktree scan + status with the racy-clean rehash rule`.

### Task 5: `NodeApi` trait + module-backed mock + checkout/materialize

**Files:**
- Create: `crates/system/duckfs-client/src/api.rs` — the **sync** trait: `refs() -> RefsInfo`, `stat`, `ls`, `find(prefix, snapshot, after, limit)`, `read(path, snapshot, offset, len) -> (Vec<u8>, eof)`, `history(limit)`, `diff(from, to, prefix)`, `has_chunks(ids) -> Vec<bool>`, `stage_chunk(bytes) -> DigestHex`, `commit(base, message, changes) -> CommitReceipt{height}`, `pin(snapshot, name)`; `ApiError { Rejected(String), NotFound, Transport(String) }`; `ConflictReport { base, head, ours: Vec<String>, theirs: Vec<String>, clashing: Vec<String> }`.
- Create: `crates/system/duckfs-client/src/checkout.rs` — `checkout(api, dir, prefix, snapshot?)`: resolve the snapshot once (explicit or `refs().head`; `None` head → empty checkout, base `None`); enumerate via `Find` with prefix `"<dir-prefix>/"` (string-prefix semantics — the trailing slash makes it a subtree match), paged by the `next` cursor; materialize dirs (incl. empty), files via paged `Read` (≤ `MAX_READ_BYTES` per call, loop to `eof`, verify assembled size and recomputed file object id against `EntryInfo.object` — a transport that lies is caught here), symlinks (`std::os::unix::fs::symlink`), exec bits; **case-collision rule:** probe the target filesystem once (create `.duckfs/CaseProbe`, stat `.duckfs/caseprobe`); if case-insensitive and the fetched tree has case-folding sibling collisions → fail with a structured error listing the colliding paths (test hook `force_case_insensitive` to simulate on Linux); write the v1 index last (checkout is resumable by re-running).
- Create: `crates/system/duckfs-client/tests/support/mod.rs` — `ModuleNode`: `NodeApi` implemented over a real `files::Files` on a tempdir, driven exactly like `crates/apps/files/tests/commit.rs:26-90` (a local `TestCtx` copy of `crates/apps/files/tests/harness/mod.rs`, `execute` + `commit_block` per op, `futures::executor::block_on` at top level); counters for `stage_chunk`/`commit` calls (dedup assertions).
- Create: `crates/system/duckfs-client/tests/checkout.rs`.

**Interfaces:** Produces the transport seam (Phase-4 odb fast path implements the same trait) and materialization. Consumes Tasks 1, 3.

- [ ] **Step 1 (RED):** seed the mock with a tree under `/shared/ws`: nested dirs, an empty dir, an exec file, a symlink, a 2 MiB+1 file (staged via putblob) and NFC-named files. `checkout` into a tempdir → assert every byte identical, exec bit set, symlink target exact, empty dir present, `.duckfs/index.json` records the head as base. Red: no checkout exists.
- [ ] **Step 2 (RED):** seed case-colliding siblings (`/shared/ws/Readme` + `/shared/ws/readme` — legal in consensus); with `force_case_insensitive` → checkout fails and the error lists both paths.
- [ ] **Step 3:** implement; green. Also assert a re-run checkout over a half-materialized dir converges.
- [ ] **Step 4:** gates as Task 3. Commit `feat(duckfs-client): NodeApi seam + checkout materialization with case-collision guard`.

### Task 6: change planning + staged commit

**Files:**
- Create: `crates/system/duckfs-client/src/plan.rs` — `Status → Vec<Change>`: added/modified files → `Change::Put` (content: `Inline{b64}` while the running inline total stays ≤ `MAX_INLINE_COMMIT_BYTES`, else `Chunks{size, chunks}`); removed → `Rm`; new empty dirs → `Mkdir` (non-empty parents ride implicitly through `Put` — assert this against the mock; if the module requires explicit parents, emit `Mkdir`s, cheap either way); symlinks → `Symlink`; kind changes → `Rm`+`Put`; **no `Mv` detection** (rename = Rm+Put; noted out of scope). Early rejects with clear errors: non-NFC local filenames (via `files::paths::canonical` on the joined duckfs path — fail before any network op, the module would reject anyway), path/name/depth cap breaches, and `changes.len() > MAX_CHANGES_PER_COMMIT` → error naming the cap and the count; nothing submitted.
- Create: `crates/system/duckfs-client/src/commit.rs` — orchestration: `refs()` → plan from index+scan → collect all `Content::Chunks` digests → `has_chunks` in ≤256-id batches → `stage_chunk` each missing (1 MiB PutBlob each; each stage is one block — sequential, no parallel hammering of the submit lane) → re-probe just before submit (staging TTL insurance) → ONE `commit(base = index.base_snapshot, message, changes)` → resolve the new snapshot id: `history(limit)` entry whose `height == receipt.height` (fallback `refs().head`) → rewrite the index (new base, refreshed entries with fresh mtimes).
- Create: `crates/system/duckfs-client/tests/commit_engine.rs`.

**Interfaces:** Produces `commit()` for CLI/workspaces. Consumes Tasks 4, 5.

- [ ] **Step 1 (RED):** checkout → edit small file + 2 MiB file + delete one + add empty dir → `commit` → mock head advances; fresh checkout into a second dir is byte-identical; index base equals the new snapshot id (resolved by height). Red: no commit path.
- [ ] **Step 2 (RED):** dedup: commit a NEW path whose bytes duplicate an already-committed file → `stage_chunk` counter is 0 (HasChunks said present). And: an interrupted upload resumes — stage one of three chunks out-of-band, commit → exactly 2 stage calls.
- [ ] **Step 3 (RED):** 4097 planned changes → error mentions `MAX_CHANGES_PER_COMMIT`; the mock records zero submits. A local NFD filename → error naming the path, zero submits.
- [ ] **Step 4:** implement; green. Gates as Task 3. Commit `feat(duckfs-client): change planning + HasChunks-probed staged commit`.

### Task 7: CAS conflict — auto-rebase or structured report

**Files:**
- Modify: `crates/system/duckfs-client/src/commit.rs` — on `ApiError::Rejected` matching `"files: conflict:"`: refetch `refs().head`; `diff(index.base, head, prefix)`; if `theirs ∩ ours == ∅` → resubmit once with `base = head` (bounded: max 3 rebase attempts, then report); else return `ConflictReport` (no silent merge, ever). On `"files: base snapshot not resolvable"` (base fell out of the 1024-window): fail with a report whose remedy says re-checkout (spec: "the client re-checks out"). If the diff itself rejects (oversized / unresolvable) → treat as genuine conflict, fail safe.
- Modify: `crates/system/duckfs-client/src/api.rs` — finalize `ConflictReport` (+ serde, the workspace RPC serializes it).
- Create: `crates/system/duckfs-client/tests/conflict.rs`.

**Interfaces:** Produces the conflict contract Tasks 9/11/12 surface. Consumes Task 6.

- [ ] **Step 1 (RED):** (a) upstream touched our path → `commit` returns `ConflictReport` with that path in `clashing`, mock shows the failed submit and NO second content submit; (b) rebase arm pinned with a unit test over a scripted `NodeApi` stub that returns the conflict error once and clean the second time, asserting exactly one resubmit with the new head as base. Also assert disjoint-path concurrent commits need no rebase.
- [ ] **Step 2 (RED):** GC'd base: `files.set_history_window_for_tests(2)` (`crates/apps/files/src/module.rs:221`), advance 3 upstream commits, then engine commit → error/report advising re-checkout, zero rebase attempts.
- [ ] **Step 3:** implement; green. Gates as Task 3. Commit `feat(duckfs-client): CAS conflict handling — bounded auto-rebase, structured conflict report`.

### Task 8: `HttpNode` transport + full-stack daemon proof

**Files:**
- Create: `crates/system/duckfs-client/src/http.rs` — `HttpNode::new(base_url)` (reqwest blocking, short connect timeout, no proxy surprises): `stage_chunk` → `POST /v1/files/stage` raw body → `{digest}`; `commit` → `POST /v1/files/commit` (snake_case `CommitBody`, `bin/noded/src/lib.rs:1207`) → **camelCase** `BlockSummary` `{height, rootHash}`; reads → the GET endpoints (snake_case replies); `has_chunks` → `GET /v1/files/has-chunks?ids=`; `pin` → `POST /v1/files/pin`; status ≥400 with `{"error": msg}` → `ApiError::Rejected(msg)` (404 → `NotFound`) — the conflict strings must pass through verbatim.
- Create: `crates/system/duckfs-client/tests/http_contract.rs` — a hand-rolled `std::net::TcpListener` stub (the `daemon_e2e.rs` raw-HTTP house style, inverted) serving canned responses; asserts exact request lines/bodies and error mapping. No axum dev-dep needed.
- Modify: `bin/noded/tests/daemon_e2e.rs` + `bin/noded/Cargo.toml` (dev-dep `duckfs-client`) — the real proof: against a spawned `ducktape-noded`, engine `checkout → edit → commit → second checkout` round-trip through `HttpNode`, including one >1 MiB file (stage path) and one conflict (two checkouts, same path) surfacing a `ConflictReport`.

**Interfaces:** Produces the shipping transport. Consumes Tasks 2, 6, 7.

- [ ] **Step 1 (RED):** contract test: `stage_chunk(b"abc")` sends `POST /v1/files/stage` with body `abc` and parses `{digest}`; commit reply parses camelCase `rootHash`. Red: no `http.rs`.
- [ ] **Step 2:** implement; contract green.
- [ ] **Step 3 (RED→GREEN):** the daemon_e2e round-trip above.
- [ ] **Step 4:** gates: `cargo test -p duckfs-client -p noded`, workspace check, fmt/clippy. Commit `feat(duckfs-client): blocking http transport + real-daemon round-trip`.

### Task 9: workspace RPC — the jobs/sandbox seam

Greenfield seam (no jobs/agent wiring in this phase — RPC only). Managed checkouts under an injected root; state lives on disk (`<root>/<id>/.duckfs`), so the daemon holds no in-memory registry that a restart loses.

**Files:**
- Create: `bin/noded/src/workspaces.rs` — routes: `POST /v1/fs/workspaces` `{prefix, snapshot?}` → 200 `{id, path, snapshot}` (id = random 16-hex slug, `[a-z0-9]` only — reject anything else on the path params, no traversal); `POST /v1/fs/workspaces/{id}/commit` `{message}` → `{snapshot, height, rebased}` or **409** with the serialized `ConflictReport`; `DELETE /v1/fs/workspaces/{id}` → `{ok:true}`. Handlers wrap engine calls in `tokio::task::spawn_blocking` (the git-pack precedent, `lib.rs:1959`); per-workspace serialization via an in-process mutex map (two concurrent commits on one workspace must not interleave scans).
- Create: `bin/noded/src/actor_api.rs` — `ActorNodeApi`: `duckfs_client::NodeApi` over the `NodeCommand` lane (encode `FilesMsg`/`FilesQuery`/putblob frame; `futures::executor::block_on` the mpsc send + oneshot — safe on a `spawn_blocking` thread, the actor lives on its own thread; futures channels are executor-agnostic).
- Modify: `bin/noded/src/lib.rs` — `NodeHandle` gains `duckfs_workspaces: Option<PathBuf>` + `with_duckfs_workspaces(..)` (the `forge_repo`/`index` precedent, `lib.rs:376-425`); unset → 503 (router-test fake handle). Route registration. Move `duckfs-client` from dev-dep to dep in `bin/noded/Cargo.toml`.
- Modify: `bin/noded/src/main.rs:184-187` — `.with_duckfs_workspaces(storage.join("duckfs-workspaces"))`.
- Modify: `bin/node/src/main.rs:3994` — same builder call with `storage.join("duckfs-workspaces")`.
- Modify: `bin/noded/tests/router.rs` (503 when unconfigured; slug validation) + `bin/noded/tests/daemon_e2e.rs` (create → files appear on disk at `path` → edit → commit → read back via `/v1/files/read` → delete → dir gone; conflicting workspace commit → 409 with `clashing` populated).

**Interfaces:** Produces the sandbox lifecycle RPC (Phase 5+/jobs consume it). Consumes Tasks 6–8.

- [ ] **Step 1 (RED):** daemon_e2e: `POST /v1/fs/workspaces {prefix:"/shared/job1"}` → today 404. Assert 200 `{id, path}` and that `path` contains `.duckfs/index.json`.
- [ ] **Step 2:** implement adapter + handlers + injection; green, including the 409 conflict shape.
- [ ] **Step 3:** gates: `cargo test -p noded`, workspace check, fmt/clippy. Commit `feat(duckfs): workspace rpc — managed checkouts over the actor-lane NodeApi`.

### Task 10: `ducktape-fs` CLI — skeleton + read verbs + in-process harness

**Files:**
- Create: `bin/fs/Cargo.toml` — package `fs-bin` (the `node-bin` naming precedent), `[[bin]] name = "ducktape-fs"`; deps: `duckfs-client`, `files = { workspace = true, default-features = false }` (display types), `serde_json`; dev-deps: `noded`, `host`, `sdk`, `files` (native), `futures`, `tokio` (rt-multi-thread/net/macros), `tempfile`. Add to workspace members.
- Create: `bin/fs/src/main.rs` — verb dispatcher (the `bin/node/src/main.rs:2350` match style) + help text; `mount` verb present but returns a clear "mount arrives in phase 4 (FUSE)" error (reserve the verb).
- Create: `bin/fs/src/args.rs` — `parse_flags` (copied shape from `bin/node/src/main.rs:2420`) + node-address resolution: `--node <http-url>` flag → `DUCKTAPE_NODE` env → for worktree verbs, the `.duckfs` index's stored node URL → else a clear error.
- Create: `bin/fs/src/read_cmds.rs` — `ls <path> [--snapshot S] [--limit N]`, `cat <path> [--snapshot S]` (stdout bytes, paged reads), `stat <path>`, `history [--limit N]`, `diff <from> <to> [--prefix P]` — thin veneers over `NodeApi` with stable line-oriented output (scriptable, greppable).
- Create: `bin/fs/tests/cli_e2e.rs` + `bin/fs/tests/support/mod.rs` — the in-process node: a real `host::Host::genesis` with ONLY the files module on a dedicated thread pumping `NodeCommand`s (mirror `bin/noded/src/main.rs:340-410`), `noded::router(handle)` served on a local tokio listener; run `env!("CARGO_BIN_EXE_ducktape-fs")` subcommands against it. (Cross-package `CARGO_BIN_EXE` doesn't exist — this is why the CLI e2e lives in `bin/fs` and the cluster e2e in Task 12 drives the engine as a library.)

**Interfaces:** Produces the CLI read surface. Consumes Task 8.

- [ ] **Step 1 (RED):** `ducktape-fs ls /shared --node <harness-url>` against a seeded harness → exits 0, output lists the seeded entries; red: no binary. Also: no `--node`, no env → exit ≠ 0 with the resolution error; `ducktape-fs mount x y` → the phase-4 stub error.
- [ ] **Step 2:** implement dispatcher/args/read verbs; green (`cat` byte-exactness incl. a >1 MiB file).
- [ ] **Step 3:** gates: `cargo test -p fs-bin`, workspace check, fmt/clippy. Commit `feat(ducktape-fs): cli skeleton + read verbs over the duckfs surface`.

### Task 11: CLI working-copy verbs — checkout / status / commit / pin

**Files:**
- Create: `bin/fs/src/work_cmds.rs` — `checkout <prefix> <dir> [--snapshot S]`; `status [dir]` (default `.`; prints A/M/D per path, exit 1 when dirty — script-friendly); `commit [dir] --message <m>` (prints new snapshot id; on conflict prints the report — base, head, clashing paths — and exits 2; `--no-rebase` to disable the auto-rebase); `pin <snapshot> <name>`. Worktree verbs read the node URL from the index (with `--node` override).
- Modify: `bin/fs/src/main.rs` (dispatch), `bin/fs/tests/cli_e2e.rs`.

**Interfaces:** Consumes Tasks 5–8, 10.

- [ ] **Step 1 (RED):** e2e: `checkout` → edit files → `status` shows M/A/D and exits 1 → `commit --message` → `status` clean (exit 0) → fresh `checkout` elsewhere matches bytes. Red: verbs missing.
- [ ] **Step 2 (RED):** conflict path: two checkouts, both edit the same path, commit both → second exits 2 and stderr names the clashing path.
- [ ] **Step 3:** implement; green. Gates as Task 10. Commit `feat(duckfs): cli checkout/status/commit/pin — the working-copy loop`.

### Task 12: two-node cluster e2e — the phase proof

Mirror the Phase-2 restart/joiner harness (`bin/node/tests/common/mod.rs` `Cluster`, `common::serial()`, `poll_until`; duckfs precedent in `restart_e2e.rs` / `large_file_e2e.rs`). Drives the **engine as a library** over each node's real HTTP surface (`Cluster.http_ports` — add a small `pub fn http_base(idx)` helper).

**Files:**
- Create: `bin/node/tests/duckfs_client_e2e.rs`.
- Modify: `bin/node/Cargo.toml` — dev-dep `duckfs-client`.
- Modify: `bin/node/tests/common/mod.rs` — `http_base(idx)` helper.

- [ ] **Step 1 (RED):** two validators `[0,1]`; `HttpNode` at node 0: checkout `/shared/e2e` (empty, base None) → write a small file + a 2 MiB+1 pattern file (forces the stage path through real consensus) + an empty dir + a symlink → `commit`; then `HttpNode` at node 1: checkout → byte-identical (`poll_until` finality).
- [ ] **Step 2:** edit-and-recommit from node 1's checkout; re-checkout on node 0 shows the edit (the full round-trip both directions).
- [ ] **Step 3:** conflict over the real cluster: two checkouts of the same prefix, same-path edits → second engine commit yields a `ConflictReport` naming the path; disjoint-path concurrent commits both land.
- [ ] **Step 4:** workspace RPC over node 0's surface: create → edit on disk → commit → node 1 reads the change → delete.
- [ ] **Step 5:** gates: `cargo test -p node-bin --test duckfs_client_e2e -- --test-threads=1`; whole suite `cargo test -p node-bin` unaffected. Commit `test(duckfs): two-node checkout→edit→commit→checkout round-trip + workspace rpc e2e`.

### Task 13: docs + hygiene

**Files:**
- Modify: `docs/src/content/docs/en/human/modules/product-modules.mdx` + `ko` mirror — the duckfs section gains the client story: `ducktape-fs` verbs, `.duckfs` index, conflict semantics (auto-rebase disjoint only), workspace RPC; note `mount` is Phase 4.
- Modify: `docs/src/content/docs/en/agent/reference/repository-map.mdx` + `ko` — `crates/system/duckfs-client`, `bin/fs`.
- Sweep: every new file under the ~600-line cap; error prefixes consistent (`"files: "` on module-side, plain on client-side); no stray `println!` debugging.

- [ ] Steps: docs → hygiene → commit `docs(duckfs): phase-3 client stack — cli, index, workspaces`.

## Whole-phase gates

- `cargo test --workspace` green (includes the new `duckfs-client`, `fs-bin` suites).
- `cargo check -p files --no-default-features` green (the wasm-purity standing gate — Task 1 touched the core).
- `cargo check --workspace` + `cargo fmt -- --check` + clippy on `files`, `noded`, `duckfs-client`, `fs-bin`, `node-bin`.
- E2E: `cargo test -p noded --test daemon_e2e`, `cargo test -p fs-bin --test cli_e2e`, `cargo test -p node-bin --test duckfs_client_e2e -- --test-threads=1`.
- Grep gates: no `clap` in the workspace; no `duckfs-client` dependency from any consensus-path crate (`crates/apps/*`, `crates/kernel/*`).

## Risks (read before executing)

1. **mtime granularity / racy index** — coarse mtimes can hide same-second edits; Task 4's racily-clean rehash rule is the mitigation and has a dedicated red test. Do not "optimize" it away.
2. **Staging TTL (4096 blocks) vs slow uploads** — chunks staged early in a huge commit can expire before the `Commit` lands (quota is only released at commit; TTL sweep is consensus). Mitigations: re-probe `has_chunks` immediately before submit (Task 6); on `"files: chunk not available"` rejection, re-stage the missing set and retry once. Also: all HTTP stages ride owner `"noded"` (`DEFAULT_ORIGIN`) — the 1 GiB owner quota and 4096-entry owner cap are shared by every client of one node; concurrent large commits can starve each other. Documented, not solved, in Phase 3.
3. **`MAX_CHANGES_PER_COMMIT` overflow** — fail with a clear error, never chunk the commit into pieces (atomicity is the spec's point). Test pinned in Task 6.
4. **String-typed conflict detection** — the engine keys on `"files: conflict:"` / `"files: base snapshot not resolvable"` passing verbatim through the HTTP 400 envelope. Pinned by contract tests (Tasks 2, 8); if module error text changes, those tests are the tripwire. A structured error channel is a welcome follow-up, not this phase.
5. **`HasChunks` is advisory** — odb contents may include not-yet-GC'd extras on one node; worst case is a redundant stage or a clean commit rejection, both handled. Never treat the probe as a consistency proof.
6. **One stage = one block** — commit throughput of large files is block-rate-limited by design (spec: "ingest speed is consensus speed"). The engine stages sequentially; no parallel submit hammering.
7. **Blocking I/O in the daemon** — workspace handlers do disk scans + actor round-trips; `spawn_blocking` + `block_on` over futures channels is the chosen shape (deadlock-safe: the actor lives on its own thread). Never `block_on` on the axum worker itself.
8. **Case/Unicode edges** — collision probe is a heuristic (per-directory filesystems can fool a root-level probe); acceptable for Phase 3, noted in the checkout error text. Windows (exec bits, symlinks) is unsupported/untested this phase.

## Out of scope (explicitly)

- **FUSE / `mount`** (Phase 4, #221) — the verb is reserved with a stub error; the sync `NodeApi` trait is the seam the odb fast path will implement.
- **TS client mirror** — folds into Phase 5 (#222).
- **jobs/agent ↔ workspace wiring** beyond the RPC seam itself; no module consumes `/v1/fs/workspaces` yet.
- **Rename (`Mv`) detection**, auto-merge, content-defined chunking, an `unpin` HTTP route/verb, per-request submitter identity on the duckfs HTTP lane (stays the trusted-client convention), watch subscriptions in the client.
