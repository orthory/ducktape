# Forge dev-mode dogfooding — design

**Date:** 2026-07-07
**Branch:** `feat/forge-dev-dogfooding` (off `origin/dev`)
**Status:** approved, implementing

## Goal

In dev, host the ducktape source in ducktape's own `forge` module — real repo,
full history — so the desktop Forge view browses ducktape's actual commits.
Dogfooding by literally using ducktape as the git host for ducktape.

## Background: what forge already provides

`forge` is a git-backed consensus module (`crates/apps/forge`). Its state is a
set of real on-disk git repos at `<forge_base>/<repo>`; the module root is a
sha256 over each repo's committed `refs/heads/main` oid. The app's daemon
(`bin/node`, spawned as the `ducktape-node` sidecar) registers forge with base
`<storage>/forge-repo` and, via `noded`'s shared axum `router()`, already serves
a **git smart-HTTP remote**:

- `GET  /forge/{repo}/info/refs`         — ref advertisement
- `POST /forge/{repo}/git-receive-pack`  — push  (body cap `GIT_PACK_BODY_LIMIT = 512 MB`)
- `POST /forge/{repo}/git-upload-pack`   — fetch/clone

`git_receive_pack` parses the pkt-line stream (including git's flush-probe for
pushes over `http.postBuffer`), stores the **whole packfile as one node-local
blob** via `blobs.put_chunk` (no `MAX_CHUNK_SIZE` enforcement on this path), and
submits a `forge::ForgeMsg::Push { repo, prev_oid, new_oid, pack_digest }`. Only
the **32-byte digest + oids** cross consensus; `forge::materialize` later fetches
the pack by digest and moves the on-disk ref. This is proven end-to-end by
`bin/noded/tests/daemon_e2e.rs::git_push_over_http_lands_in_forge_head`
(create + fast-forward + non-fast-forward reject).

## Why the earlier push attempt failed (root cause)

The 4.6 MB ducktape pack has **no real blocker** — it hit the wrong doors:

- `POST /v1/files/blob` is hard-capped at `files::MAX_CHUNK_SIZE` (4 MB) by
  design ("one chunk per request, so the body cap IS the chunk cap"). A
  whole-repo pack was never meant to go there → **413**.
- Routing the pack through consensus hits the p2p frame cap — also by design;
  the pack must **never** enter consensus.

The intended door is `git-receive-pack` (512 MB, digest-only through consensus).
4.6 MB ≪ 512 MB, so a normal `git push` just works. **No size limit needs
changing; no `files` / `Push` / consensus change is in scope.**

## The two gaps to close

### 1. Reader-path bug (`app/src-tauri/src/forge_git.rs`)

The desktop Forge view (`ForgeView.tsx` → `forge-git-client` → `forge_git.rs`)
reads the on-disk materialized repo. But the reader opened the **base container**
`<storage>/forge-repo`, while the module materializes repos one level down at
`<storage>/forge-repo/<name>`. So it never found a repo and `forge_head()` always
returned `None` on desktop — the view was empty even after a successful push. It
also **hardcoded** the single repo's identity as `"ducktape"`
(`forge-git-client.ts`), which would mislabel whatever repo was actually opened.

**Fix — repo-name-generic, end to end (no hardcoded name anywhere):**

- `forge_list_repos` (new command): enumerate every repo materialized under the
  base(s) by its **real on-disk directory name**, with its committed head
  (`BTreeMap`-sorted, deduped, first base wins). Whatever was pushed
  (`ducktape`, `default`, …) shows up under its own name. (forge inits
  **non-bare** repos, so the `<sub>/.git` check holds.)
- The read commands (`forge_head`/`forge_log`/`forge_tree`/`forge_read_file`/
  `forge_diff`) take a `repo` param and open `<base>/<repo>` via `open_named_repo`
  — the caller (the UI's repo selection) says which repo to read.
- `forge-git-client.ts` returns the real repos from `forge_list_repos` and
  threads the repo name into every read; the hardcoded `"ducktape"` label and the
  single-repo `forgeRepoInfo` shim are deleted.
- `ForgeView.tsx` (which already has full multi-repo UI — repo cards, a repo
  menu, `selectedRepo`) threads the selected repo's real name into the tree/file
  reads via an `activeRepoRef`. Nothing browses a repo under the wrong label.

### 2. Dev-mode dogfood trigger — a static `ducktape-dev` remote

Rather than boot-time magic, register a real git remote and make the dogfood
target the freshness gate. Raw pushes are not the supported refresh path
because they skip the canonical fetch and equality check.

**`ops/dogfood-forge.sh`** (invoked by `make dogfood-forge`):

1. Resolve the forge base URL:
   - `DUCKTAPE_DEV_FORGE_URL` env override, else
   - the active workspace's `http_listen` from
     `~/.ducktape/registry.json` (`active`) →
     `~/.ducktape/workspaces/<active>/node.toml` (`http_listen = "127.0.0.1:<port>"`;
     the workspace flow assigns a **random** http port, so this is not a fixed
     `:8844`), else
   - fall back to `127.0.0.1:8844` (the web/legacy default).
2. `FORGE_REPO` (default `ducktape`) → remote URL `http://<addr>/forge/<repo>`.
3. Idempotently set the remote: add `ducktape-dev`, or `set-url` if it exists.
4. Fetch canonical `origin/dev` and freeze this fetch's `FETCH_HEAD` OID.
5. Push that exact OID to Forge `main`, read the remote ref back, and require
   equality before agent work may start.

Re-running the target updates the forge repo (a fast-forward push of new
commits). CAS/non-fast-forward semantics are git's own — the endpoint reports
them faithfully.

## Explicitly out of scope

- No `MAX_CHUNK_SIZE` bump, multi-chunk packs, or any `files`/`Push`/consensus
  change — the git-native path carries full history within limits.
- No production behavior change — the remote points at a **local** node and is a
  dev convenience; nothing auto-runs in a release build.
- `forgeListRepos` stays single-repo (the view shows one repo); enumerating
  multiple materialized repos is future work.

## Testing / verification

- **Rust:** `cargo build -p node-bin -p noded` and `cargo test -p forge`
  (no logic change to forge; the reader change is in the tauri crate).
  `cd app && bun run typecheck` for the tauri Rust + TS.
- **End-to-end (the real thing):** with a dev node running, `make dogfood-forge`,
  then confirm `git ls-remote ducktape-dev` shows `refs/heads/main` at the exact
  fetched `origin/dev` OID, and the desktop Forge view (via `tauri-debug` /
  `forge_head`) reports that OID with real commit history and file tree.
- **Reader unit-ish:** point `open_forge_repo` at a temp base containing a
  `<base>/<name>/.git` repo and assert it opens (guards the descend fix).

## Files touched

- `app/src-tauri/src/forge_git.rs` — `forge_list_repos` + `open_named_repo`; the
  read commands take a `repo` param (open `<base>/<repo>`).
- `app/src-tauri/src/main.rs` — register `forge_list_repos`.
- `app/src/domain/forge-git-client.ts` — real repo enumeration; repo param on
  every read; drop the hardcoded `"ducktape"` label.
- `app/src/console/views/forge/ForgeView.tsx` — thread the selected repo's real
  name into the reads (`activeRepoRef`).
- `ops/dogfood-forge.sh` — new; resolve URL, register `ducktape-dev`, fetch
  canonical `origin/dev` (or honor an explicit `SRC_REF`), push the frozen OID,
  verify Forge `main`, and warn on cross-worktree repoint.
- `Makefile` — new `dogfood-forge` target.
- `docs/superpowers/specs/2026-07-07-forge-dev-dogfooding-design.md` — this doc.
