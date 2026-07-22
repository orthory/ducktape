# Console Redesign & Refactor Spec

Status: **DRAFT for review** (author: Fable, planning tier). This is the single
north-star every implementer works against. It exists because the console
degraded into inconsistent per-agent work with no shared design/architecture
contract; from here, nothing gets built that isn't in this spec.

North star: **`ducktape-app`** (`../ducktape-app/apps/desktop`) — the design and
architecture the team considers "done well." We follow it as closely as the
current node's real capabilities allow, and where a reference feature is not
implementable here we say so explicitly (§5).

Two hard constraints carried in from project memory:
- **No backwards-compat / fresh genesis** — flag-day changes are fine; delete,
  don't deprecate.
- **Think lightweight** — small-team / self-hosted / solo+agents. Don't port
  enterprise-shaped features (multisig treasury, module marketplace) just
  because the reference sketched them.

---

## 0. The reframing (why this is smaller than it looks)

The desktop app talks to `bin/node` (the real consensus binary), which
genesis-registers **17 modules** and already answers all of them on the same
HTTP/WS wire: `chat, tasks, forge, document, agent, governance, vaults, valset,
profiles, inbox, automations, jobs, memory, files, saga (+ kv, directory
internal)`. So:

- The current app is in places **ahead of** the reference (real onboarding
  park→admit→promote; tasks/document/agent screens the reference reverted).
- Most reference features are **"already implementable, just missing a TS
  client + screen"**, not node work.
- Genuine **node work** is narrow: (1) consensus `round`/leader/peer/finality
  in `/v1/status`; (2) a chat pin op; (3) anything that literally ports the
  reference's multisig vault / module marketplace / gov role-promotion (we are
  **not** doing these). Forge browsing is **not** node work — see §5: the
  desktop is co-located with its node, so the forge git repo is on local disk
  and we read it with a local git2 service (the reference's own pattern).

The problem is therefore **90% design + frontend architecture**, ~10% small
node additions. Scope the work accordingly.

---

## 1. Design system

The reference's `theme/tokens.ts` (color/font/radius/shadow, warm-neutral
editorial palette, strict sans/mono split) **already exists verbatim** in this
repo at `app/src/console/theme/tokens.ts`. We are not inventing a design — we
are applying the reference's proven patterns consistently and killing drift.

Rules (binding for every view):

1. **One shared UI kit.** The reference itself carries drift (4 copies of
   `useHover`, per-module `ModalShell`/`PrimaryButton`/`CenterNote`/`Spinner`
   kits, two competing agent-chip palettes). We do **not** reproduce that.
   Create `app/src/console/components/` as the single kit and route every view
   through it:
   - `Avatar` (human=grey circle / agent=dark rounded-square / me), `Icon`,
     `Button` (primary dark / ghost outline / danger), `Field` (label+value
     row), `TextField`/`TextArea`/`SelectField`, `Modal` (scrim + focus-trap +
     Esc + focus-restore — port the reference's `overlays/Modals.tsx`, it's the
     most complete a11y impl), `Toast`, `Spinner`, `CenterNote`
     (loading/empty/error/gated placeholder), `StatusPill`, `RoleBadge`,
     `Toggle`, `Segmented`, `SectionLabel`, `GroupCard`, `InfoRow`,
     `useHover` (one hook), `clickable` (a11y — already exists).
2. **Status-pill vocabulary is one shape everywhere**: `{label, color, bg, bd}`
   — saturated text, light tint bg, mid tint border. Success/open
   `#5f9e74/#eef5f0/#cfe3d7`; warning/pending `#a07b32/#fbf4e6/#ecdcae`;
   danger/closed `#a35248/#fbeeec/#eccfc9`; neutral `#7a6f9e/#f1edf5/#ddd2e6`.
   Used for node status, task status, forge PR/issue state, everything.
3. **Sans for prose/labels, mono for every technical value** (keys, heights,
   hashes, timestamps, IDs, uppercase badge labels with `letter-spacing`).
4. **Agent vs human is a first-class visual axis** — pick ONE agent palette
   (accent-tinted `#f9f1ea/#e7d2c4/accentVar`) and use it for every AGENT
   tag/chip; humans are neutral grey. `isAgentAuthor()` drives it.
5. Inline `style={}` objects keyed off tokens (repo idiom); no Tailwind classes
   for component styling, no per-view hex literals where a token exists.

Pixel detail lives in the reference files — implementers read
`../ducktape-app/apps/desktop/src/ducktape/{theme,layout,components,overlays}`
and the per-view files named in §4, and match them.

---

## 2. Frontend architecture (the "리팩토링")

Current state: one **918-line `DucktapeProvider.tsx`** + a **61-field
`ConsoleState`** mixing domain projections, transient UI state (`hoverMsg`,
`msgMenuId`, `activeThread`), and workspace/onboarding lifecycle; one giant
`refresh()` `Promise.all` pulling every module every block. This is the
god-object. Target = the reference's shape, adapted to our **already-decomposed
per-module node**.

### 2.1 Store file split (mechanical, do first)

Split `store/` into the reference's five roles:
- `state.ts` — state shape + `createInitialState()` + `applySnapshot()` (the
  single gateway that writes ONLY domain-projection fields, never UI fields).
- `reducer.ts` — trivial: two ops, `patch` (merge partial) and `update` (merge
  fn of state). No business logic.
- `actions.ts` — all business logic / wire calls / validation. Every mutation:
  call node → get result → `patch(applySnapshot(...))`, plus any explicit UI
  patch (e.g. close a menu) as a second `patch` in the same action.
- `context.ts` — just `createContext` (kept separate to break the
  provider→registry→views→context import cycle).
- `DucktapeProvider.tsx` — wiring only: reducer + initial state, inject the
  transport/node source, hydrate once, fan block events to slice refreshes,
  one nav-event listener. No logic.

### 2.2 State-ownership rule (the litmus test for all 61 fields)

Data ownership decides state ownership — **not** "is it transient":
- Field read/reset only inside one screen's subtree → **local** (`useState` /
  a view-local context, like the reference's `forge-context.tsx`). The shell
  renders exactly one screen at a time, so unmount is a free reset.
- Field that must be reset by *other* screens' navigation, or is touched as a
  side effect of a centralized wire mutation → **global**, in state, mutated
  via the actions facade.
- Purely cosmetic (hover/focus) → **always local**, via the one `useHover`.

Immediate consequence: `hoverMsg`/`msgMenuId` and per-view selection that leaked
into the global store move **local to the chat view** (they need only chat-local
"one menu open at a time"), unless a concrete cross-screen reset requires
otherwise. Onboarding/workspace lifecycle stays global (it gates the whole
shell).

### 2.3 Per-module slices over one god-refresh

Because the node is per-module decomposed (each module has its own `/v1/query`
target + typed client in `app/src/domain/*-client.ts`), model each console
module as a **slice**: its own state fields + its own `refresh(transport)` that
queries only its module + its own actions. `DucktapeProvider` subscribes to the
block stream once and calls each active slice's refresh on a finalized block
(replacing the monolithic `Promise.all`). A slice may instead **self-fetch on
demand** (the forge/vault pattern) when its data is large or navigational
(forge file contents, doc blocks) — don't force everything through per-block
refresh.

Keep `NodeStatus`/connection/workspace/onboarding in a small **session core**
slice that every screen can read.

### 2.4 Module contract + registry (already partly here)

Keep/【extend the existing `modules/registry.ts` to the reference's `AppModule`
contract: `{ id, nav: {icon,label,order,section}, Screen, tier? }`, one
`MODULES[]` array, pure selectors for sidebar + screen resolution. Adding a
screen = one folder + one registry line. The sidebar/shell must know no module
by name (it already mostly follows this).

### 2.5 Domain layer (already good — keep, fill gaps)

`domain/` per-module typed clients over `transport.ts` is already the right
shape. Fill the missing clients as screens need them (valset, governance, inbox,
automations, jobs, memory, files, vaults) using the same
`send()`/guard pattern. Add a generic `send()`+`arrayOf()` validate helper if
not present.

---

## 3. Scope & phasing

**Phase 1 — bring the EXISTING surface to reference quality + do the store
refactor.** This is the bulk of "everything's messed up." Screens: chat, tasks,
forge, document, agent, **node ops panel**, settings, onboarding, tray. No new
modules. This phase is what we execute first and fully.

**Phase 2 (optional, later, per-user pull) — new screens that are already
node-backed:** members/validator roster (valset+profiles), governance/approvals
viewer (gov Propose/Vote/Execute over `{AddValidator,RemoveValidator,Signal}`),
vaults-as-secrets (the real `vaults` module = secrets manager, NOT the
reference's multisig treasury), and any of inbox/automations/jobs/memory/files.
Each is "TS client + screen," zero node work. Not in Phase 1.

**Explicitly NOT doing** (see §5): reference's multisig treasury vault,
governance role-promotion, runtime module marketplace, software-update panel as
a core feature.

Baseline first: the current tree is one large uncommitted pile from multiple
agents. Before Phase 1 code, checkpoint it to a branch (recoverable, reviewable)
so the refactor diff is legible. Publishing (commits/push/PR/merge) is done by
Fable/Opus/human — **never Codex**.

---

## 4. Per-screen plan

Legend: **PORT** = follow reference closely; **FRESH** = no reference screen,
design in the reference's language using the named template; **NODE-PANEL** =
§4.1.

| Screen | Plan | Reference / template | Adaptation notes |
|---|---|---|---|
| Chat | PORT | `views/chat/*` | Single left-aligned column, grouped by author, day dividers, hover bar, inline "N replies" → right ThreadPanel, InspectorPanel over `NodeStatus`/message metadata. **All messages uniform left-aligned Slack style** (no right "mine" bubble — user rejected it). Timestamps are unix **seconds** → `*1000`. Wire supports edit/delete/reactions/thread already. |
| Node ops panel | NODE-PANEL | historical `views/node/*` (§4.1) | The user's priority. Rebuild rich; map to real data; honest placeholders for what the node doesn't expose yet. |
| Settings | PORT | `views/settings/SettingsView.tsx` | NETWORK / YOUR IDENTITY / PREFERENCES / (SOFTWARE: drop or stub — no updater dep) / DANGER ZONE. Wire to workspace + profiles `SetName` (already there). |
| Onboarding | PORT (light) | `views/onboarding/*` | Current onboarding is already more real than the reference; reskin to the reference's visual (Welcome/Create/Join/Provisioning/Live) over the existing `OnboardingGate`/`JoinProgress` logic — don't regress the real phase machine. |
| Forge | PORT (local git2 read service) | `views/forge/CodeBrowser + ForgeList` + reference `crates/forge` read side | See §5. **Reads** (repo/log/tree/file/diff) via a local Tauri git2 service opening the node's on-disk repo — ports the reference's `forge-client`/CodeBrowser almost directly (shapes match). **Writes** build Git objects off-chain and advance refs through `PushRefs`; the legacy `ForgeMsg::Commit` wire is retired. Single branch `main` (node's model). **No PRs/issues** (not lightweight). |
| Tasks | FRESH | reference has none; template = Members list + StatusPill | Wire = `TaskMsg{CreateTask,UpdateStatus}` / `TaskQuery::List` (already fully wired). Design a clean task list with status pills + a composer, in the reference's language. |
| Documents | FRESH | reference reverted its docs module; template = a simple block editor | Wire = full `DocMsg`/`DocQuery` (already wired). Keep the block-list editor; reskin to tokens. |
| Agents | FRESH | reference has none (agents = a member kind there) | Wire = full `AgentMsg`/`AgentQuery` (already wired: agents/watches/runs). Design a roster + run timeline in the reference's language. |
| Tray | DONE (verify) | `tray/TrayPopover.tsx` | Already rebuilt master/detail dark-vibrancy; verify against reference, keep. |

### 4.1 Node operations panel (priority)

Rebuild the reference's **deleted-but-recovered** rich Node screen (git
`936764c`: `NodeHeader` + Overview/Permissions/Activity/Ledger tabs), adapted to
what our node actually serves. Be **honest** — show real data, mark the rest.

- **NodeHeader** (always visible): "This node" + a **StatusPill** from
  `NodeStatus` (synced/stopped), a **role pill** from workspace
  (`founder`/`member` → admin/member; validator via valset), a mono meta line
  (`peer {workspace.pubkey} · ducktape-node v{version}`), and **run controls**
  Start/Stop/Restart — REAL (managed workspaces: `daemon_spawn` /
  POST `/v1/shutdown` / `workspace_select`). A sub-tab bar.
- **Overview tab**:
  - YOUR ACCESS role card (from workspace role — simplify the reference's
    3-tier admin/maintainer/viewer to what valset/workspace actually knows:
    validator vs guest; don't invent roles the node lacks).
  - NETWORK stat cards: **HEIGHT** (real), **ROUND** / **PEERS** / **FINALITY**
    → honest `"—"` placeholders today (the reference did the same) — these are
    the one genuine node-work follow-up (add fields to `/v1/status`, §5).
  - **STATE COMMITMENT** — our real strength: show `appHash` + **per-module
    merkle roots** from `NodeStatus.modules` (each `{id, root}`),
    click-to-copy. This is real, verifiable, and the honest centerpiece of the
    panel. Feature it.
- **Permissions tab**: a capability matrix, but only over roles the node really
  has (validator vs guest). Don't reproduce the reference's 3-tier fiction.
- **Activity tab**: node log tail — only if we add a cheap Tauri command to tail
  the workspace `daemon.log` (small app work); otherwise defer.
- **Ledger tab**: an event ledger needs an `events_since`-style query the node
  doesn't expose → **defer** (or show a live block-height/appHash feed from
  `/v1/ws`). Note as node-work.

---

## 5. Not directly implementable → adaptation decisions

| Reference feature | Decision for this app |
|---|---|
| **Forge rich browsing** (log/tree/file/diff) | **Local git2 read service — matches the reference exactly.** Assumption (per user): the desktop is co-located with / attached to its validator node (the desktop already *spawns* the node with a `--storage` dir it controls), so the node's forge git repo is on **local disk** at a deterministic path: `{storage}/forge-repo` for `bin/node`, `{storage}/forge-git` for legacy `noded`. A small Tauri-side git2 reader (new command surface `forge_git`, mirroring the reference's `commands/forge_git.rs` + `crates/forge` read side) opens that repo **read-only** and serves `RepoInfo/CommitInfo/TreeEntry/FileDiff` — full log (revwalk), tree, file content (blob), and diff — richer than the node wire would cheaply give. It reads the committed `refs/heads/main` (= the consensus-committed HEAD, `root = sha256(HEAD oid)`), so it is consistent with finalized state. **No node/consensus changes.** Web/remote-node builds simply don't get forge browsing (consistent with existing `isTauri()` gating). **Writes** create commit objects off-chain, replicate their pack, then advance the consensus-owned head through `PushRefs`. Single branch `main` (the node only materializes agreed refs). This is Tauri+frontend work (Opus), not a Fable node seam. |
| **Forge PRs/issues/comments** | **Not doing.** No node analog; the reference's own is 100% local KV, never consensus. Not lightweight. |
| **Members with role/status/kind** | Roster is implementable now from `valset.Validators` + `profiles.All` (needs a valset client). But **role/liveness/kind have zero node backing** — valset is flat/roleless, nothing tracks presence. Phase-2 roster shows identity + display name + validator-or-not honestly; no fake presence/roles. |
| **Vault (multisig treasury/transfers/approvals)** | **Not doing.** The node's `vaults` module is a **secrets manager** (ACL + ciphertext), a different product. If we build a vaults screen (Phase 2) it's a **secrets manager** UI, not a treasury. |
| **Governance role-promotion / module-install proposals** | **Not doing.** `GovAction` is only `{AddValidator, RemoveValidator, Signal}`. A Phase-2 governance screen is a proposal viewer/voter over exactly those. |
| **Module marketplace (install/enable/disable at runtime)** | **Not doing.** Genesis module set is compile-time fixed; no runtime install API. Cuts against deterministic genesis. |
| **Software update panel** | Defer — no `tauri-plugin-updater` dep here; not node-related; the reference itself only stubs it. Drop from Settings for now. |
| **Consensus round / leader / peers / finality** | Small **node work**: add fields to `NodeCommand::Status` / `NodeStatus` reading the Simplex engine. Until then, honest `"—"` in the node panel. |
| **Chat pin** | Small node work (`Channel.pinned` exists, no op). Optional; defer unless wanted. |

---

## 6. Execution model (tiers)

- **Fable** (planning tier): this spec; the store slice contract +
  `applySnapshot` gateway design; any `/v1/status` field additions if we do the
  round/peers node work (wire-contract seam); the review gate on everything.
  (Forge is no longer a Fable node seam — it's a local git2 read service, below.)
- **Opus 4.8 xhigh** (implementer): the frontend + the Tauri-side forge git2
  read service — store split + slices, the shared UI kit, the `forge_git`
  command surface, and each view rebuilt to the reference. Claude-native,
  matches repo idiom, in-session.
- **Codex/GPT-5.5**: may take an isolated backend brief or a read-only
  adversarial review pass — but **never publishes** (no commit/push/PR/merge).
- Publishing (baseline checkpoint, commits, push, PR) is Fable/Opus/human.

Sequencing within Phase 1: (a) baseline checkpoint; (b) store split +
`applySnapshot` + slice contract (Fable designs, Opus migrates, no behavior
change first); (c) shared UI kit; (d) view-by-view reskin to reference
(chat → node panel → settings → onboarding → tasks/document/agent → forge with
its local git2 read service); (e) live verification each step. Each step is its
own reviewed increment.
