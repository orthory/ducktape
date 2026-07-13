# Account-first UI (Conditional NODE Rail) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement ADR A6/A5 in the desktop shell — the NODE rail becomes a conditional surface revealed only when node control is available; Members/Governance/Explorer move to the USER (account) rail; a remote client sees zero node chrome.

**Architecture:** One new store predicate (`nodeControlAvailable`) is the single gate. The module registry's availability filter switches from a `clientMode` boolean to a `{ nodeControl, clientMode }` filter; the sidebar renders the USER/NODE toggle only when the gate passes. No view component moves or merges; in-view role checks keep owning op-level authority.

**Tech Stack:** React/TypeScript (`app/`), vitest, Tauri desktop shell.

**Spec:** `docs/superpowers/specs/2026-07-14-account-first-ui-design.md`

## Global Constraints

- Worktree: `.worktree/account-first-ui`, branch `account-first-ui`, PR against `dev`.
- Frontend only — no Rust, no wire change.
- Test suites live under `app/src/test/` with SHORT names, never inside `console/` (repo test-layout rule).
- All commands below run from the worktree's `app/` directory unless stated.
- Commit messages end with `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

### Task 1: The gate — `nodeControlAvailable` + node-console default

**Files:**
- Modify: `app/src/console/store/state.ts` (~line 573 `DEFAULT_OPERATOR_SCREEN`, ~line 615 next to `isClientMode`)
- Test: `app/src/test/nav.test.ts` (new)

**Interfaces:**
- Produces: `nodeControlAvailable(state: Pick<ConsoleState, "workspace" | "managed">): boolean` exported from `console/store/state.ts`; `DEFAULT_OPERATOR_SCREEN === "status"`. Task 2 consumes both.

- [ ] **Step 1: Write the failing test**

Create `app/src/test/nav.test.ts`:

```ts
// The account-first navigation contract (ADR 2026-07-14 account-node-access-
// model, A5/A6): the operator rail exists only while node control is
// available; network-data surfaces (members, governance, explorer) live on
// the account rail; a direct remote client sees no node chrome at all.

import { describe, expect, it } from "vitest";

import type { Workspace } from "../domain/workspace-client";
import {
  DEFAULT_OPERATOR_SCREEN,
  nodeControlAvailable,
} from "../console/store/state";

const ws = { id: "w" } as unknown as Workspace;

describe("nodeControlAvailable (ADR A5, interim form)", () => {
  it("a managed local workspace is controllable", () => {
    expect(nodeControlAvailable({ workspace: ws, managed: true })).toBe(true);
  });
  it("a direct remote client is not", () => {
    expect(nodeControlAvailable({ workspace: null, managed: false })).toBe(false);
  });
  it("a workspace without a managed daemon is not", () => {
    expect(nodeControlAvailable({ workspace: ws, managed: false })).toBe(false);
  });
});

describe("operator rail default", () => {
  it("is the node console", () => {
    expect(DEFAULT_OPERATOR_SCREEN).toBe("status");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/test/nav.test.ts`
Expected: FAIL — `nodeControlAvailable` has no export, and `DEFAULT_OPERATOR_SCREEN` is `"members"`.

- [ ] **Step 3: Implement in `state.ts`**

At ~line 567-573, the persistence comment + constant change (the comment's
"(chat / members)" aside names the registry's first-in-section screens):

```ts
// ... keeping the store free of the
// views graph.
const VIEW_MODE_KEY = "ducktape.viewMode";
export const DEFAULT_USER_SCREEN = "chat";
export const DEFAULT_OPERATOR_SCREEN = "status";
```

(and update the preceding comment's `(chat / members)` to `(chat / status)`).

At ~line 618, directly after `isClientMode`:

```ts
/** Node control is available (ADR A5, interim form): the active workspace's
 *  node is a managed local daemon — ours to control even while it is stopped
 *  (Start lives on the node console, so reachability is deliberately NOT a
 *  term here). When the public/private RPC split (A2) lands, this grows
 *  `|| (owner key && private RPC reachable)`; the UI gate moves with it and
 *  nothing else does. */
export const nodeControlAvailable = (
  state: Pick<ConsoleState, "workspace" | "managed">,
): boolean => state.workspace !== null && state.managed;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/test/nav.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add app/src/console/store/state.ts app/src/test/nav.test.ts
git commit -m "feat(app): nodeControlAvailable gate + node-console operator default

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Registry re-sort, ModuleFilter, and all consumers

**Files:**
- Modify: `app/src/console/modules/module-def.ts` (NavSection doc + new `ModuleFilter`)
- Modify: `app/src/console/modules/registry.ts` (section moves + filter signatures)
- Modify: `app/src/console/store/actions.ts` (three normalization sites: `applyNavSnapshot` ~1454, `landOn` ~1690, `setViewMode` ~1733)
- Modify: `app/src/console/layout/Sidebar.tsx` (conditional toggle)
- Modify: `app/src/console/views/tray/TrayPopover.tsx` (~line 112 rail memo)
- Test: `app/src/test/nav.test.ts` (extend)

**Interfaces:**
- Consumes: `nodeControlAvailable` from Task 1.
- Produces: `ModuleFilter { nodeControl: boolean; clientMode: boolean }` exported from `module-def.ts`; `moduleAvailable(id, filter)`, `modulesInSection(section, filter)`, `defaultScreenForSection(section, filter)` in `registry.ts`. Old boolean-arg signatures are DELETED (no back-compat, repo mandate).

- [ ] **Step 1: Extend the test with failing registry cases**

Append to `app/src/test/nav.test.ts`:

```ts
import {
  defaultScreenForSection,
  moduleAvailable,
  modulesInSection,
} from "../console/modules/registry";

const owner = { nodeControl: true, clientMode: false };
const client = { nodeControl: false, clientMode: true };

describe("module availability (ADR A6)", () => {
  it("the operator rail exists only under node control", () => {
    expect(modulesInSection("operator", owner).map((m) => m.id)).toEqual([
      "status",
      "gateway",
      "modules",
      "sandbox",
      "metrics",
    ]);
    expect(modulesInSection("operator", client)).toEqual([]);
  });

  it("network-data surfaces live on the account rail", () => {
    expect(modulesInSection("user", owner).map((m) => m.id)).toEqual([
      "chat",
      "pages",
      "files",
      "browser",
      "forge",
      "agent",
      "members",
      "governance",
      "explorer",
    ]);
  });

  it("a client keeps account surfaces except the A3-pending ones", () => {
    const ids = modulesInSection("user", client).map((m) => m.id);
    expect(ids).toContain("explorer");
    expect(ids).not.toContain("members");
    expect(ids).not.toContain("governance");
  });

  it("the operator rail defaults to the node console, else account fallback", () => {
    expect(defaultScreenForSection("operator", owner)).toBe(DEFAULT_OPERATOR_SCREEN);
    expect(defaultScreenForSection("operator", client)).toBe("chat");
  });

  it("unknown ids are unavailable", () => {
    expect(moduleAvailable("nope", owner)).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/test/nav.test.ts`
Expected: FAIL — type errors (filter objects passed where booleans expected) and wrong section contents.

- [ ] **Step 3: `module-def.ts` — NavSection doc + ModuleFilter**

Replace the NavSection comment block (~lines 11-14) and add `ModuleFilter`:

```ts
/** The two sidebar partitions: the "user" (account) surfaces — participant
 *  apps plus the network-data views any account with standing may read — and
 *  the "operator" node-control surfaces. The operator rail is a CONDITIONAL
 *  surface (ADR 2026-07-14 account-node-access-model, A5/A6): it exists only
 *  while node control is available, absent — not disabled — otherwise.
 *  Within a rail, in-view role checks own op-level authority. */
export type NavSection = "user" | "operator";

/** Availability inputs for the registry filters. */
export interface ModuleFilter {
  /** ADR A5: the operator section exists only while the connected node is
   *  controllable (today a managed local daemon; later also a remote node
   *  whose private RPC an owner key can reach). */
  nodeControl: boolean;
  /** Direct remote client (no local workspace): account surfaces whose
   *  client-mode data plane is pending (ADR A3) are hidden. */
  clientMode: boolean;
}
```

- [ ] **Step 4: `registry.ts` — re-sort + filter**

Replace the section comment (~lines 20-26), the `MODULES` array, and everything from `moduleAvailable` down:

```ts
// The sidebar's view-mode toggle partitions these into two rails:
//   USER — the account surfaces: participant apps plus the network-data
//          views (members, governance, explorer) any account with standing
//          may read. Admin affordances inside them stay role-gated in-view.
//   NODE — node control (console, gateway, modules, sandbox, metrics). This
//          rail exists only while node control is available (ADR A5/A6).
// `order` is a sort key WITHIN a section, so the two rails number from 0
// independently. Cross-module search is NOT a module — it is the ⌘K overlay
// the shell owns (see SearchModal), reachable from either rail.
export const MODULES: AppModule[] = [
  // ── User / account surfaces ──
  { id: "chat", nav: { icon: "chat", label: "Chat", order: 0, section: "user" }, Screen: ChatView },
  { id: "pages", nav: { icon: "pages", label: "Pages", order: 1, section: "user" }, Screen: PagesView },
  { id: "files", nav: { icon: "files", label: "Files", order: 2, section: "user" }, Screen: FilesView },
  { id: "browser", nav: { icon: "browser", label: "Browser", order: 3, section: "user" }, Screen: BrowserView },
  { id: "forge", nav: { icon: "forge", label: "Forge", order: 4, section: "user" }, Screen: ForgeView },
  { id: "agent", nav: { icon: "agent", label: "Agents", order: 5, section: "user" }, Screen: AgentView },
  { id: "members", nav: { icon: "members", label: "Members", order: 6, section: "user" }, Screen: MembersView },
  { id: "governance", nav: { icon: "governance", label: "Governance", order: 7, section: "user" }, Screen: GovernanceView },
  { id: "explorer", nav: { icon: "hash", label: "Explorer", order: 8, section: "user" }, Screen: ExplorerView },
  // ── Node control (conditional rail) ──
  { id: "status", nav: { icon: "node", label: "Node", order: 0, section: "operator" }, Screen: StatusView },
  { id: "gateway", nav: { icon: "link", label: "Gateway", order: 1, section: "operator" }, Screen: GatewayView },
  { id: "modules", nav: { icon: "modules", label: "Modules", order: 2, section: "operator" }, Screen: ModulesView },
  { id: "sandbox", nav: { icon: "sandbox", label: "Sandbox", order: 3, section: "operator" }, Screen: SandboxView },
  { id: "metrics", nav: { icon: "metrics", label: "Metrics", order: 4, section: "operator" }, Screen: MetricsView },
];

export const moduleById = (id: string): AppModule | undefined =>
  MODULES.find((m) => m.id === id);

/** Account surfaces whose client-mode projections aren't wired yet (the ADR's
 *  pending A3 work); a direct remote client hides them until that lands. */
const CLIENT_PENDING = new Set(["members", "governance"]);

/** Which modules exist for this connection. The operator section exists only
 *  while node control is available (ADR A5/A6) — absent, not disabled. */
export const moduleAvailable = (id: string, filter: ModuleFilter): boolean => {
  const mod = moduleById(id);
  if (!mod) return false;
  if (mod.nav.section === "operator") return filter.nodeControl;
  return !(filter.clientMode && CLIENT_PENDING.has(id));
};

/** The modules of one view-mode rail, ordered. */
export const modulesInSection = (section: NavSection, filter: ModuleFilter): AppModule[] =>
  MODULES.filter((m) => m.nav.section === section && moduleAvailable(m.id, filter)).sort(
    (a, b) => a.nav.order - b.nav.order,
  );

/** Which view-mode rail owns a screen id, or null for the shell's own screens
 *  (settings) and unknown ids. */
export const sectionForScreen = (screen: string): NavSection | null =>
  moduleById(screen)?.nav.section ?? null;

/** The default screen a rail lands on (its first available module; an empty
 *  rail — the operator section without node control — falls back to chat). */
export const defaultScreenForSection = (section: NavSection, filter: ModuleFilter): string =>
  modulesInSection(section, filter)[0]?.id ?? "chat";
```

Import line gains the type: `import type { AppModule, ModuleFilter, NavSection } from "./module-def";`

- [ ] **Step 5: `actions.ts` — three normalization sites**

Add `nodeControlAvailable` to the existing `../store/state` (same-file `./state`) import that already carries `isClientMode`, and `ModuleFilter` to the module-def/registry type imports. Add one helper next to `landOn` (~line 1688):

```ts
const filterOf = (s: ConsoleState): ModuleFilter => ({
  nodeControl: nodeControlAvailable(s),
  clientMode: isClientMode(s),
});
```

`applyNavSnapshot` (~1454-1460) — replace the clientMode usages:

```ts
const requestedSection = sectionForScreen(snap.screen);
const filter = filterOf(before);
const screen =
  requestedSection && !moduleAvailable(snap.screen, filter)
    ? defaultScreenForSection(requestedSection, filter)
    : snap.screen;
const viewMode = sectionForScreen(screen) ?? snap.viewMode;
```

and the later persistence guard (~1485) becomes `if (!filter.clientMode) saveViewMode(viewMode);`.

`landOn` (~1690):

```ts
const landOn = (screen: string, extra: Partial<ConsoleState> = {}) => {
  const filter = filterOf(getState());
  const requestedSection = sectionForScreen(screen);
  const target =
    requestedSection && !moduleAvailable(screen, filter)
      ? defaultScreenForSection(requestedSection, filter)
      : screen;
  const section = sectionForScreen(target);
  if (section && !filter.clientMode) saveViewMode(section);
  patch({
    screen: target,
    atHome: false,
    ...(section ? { viewMode: section } : {}),
    ...(target === screen ? extra : {}),
  });
};
```

`setViewMode` (~1733):

```ts
setViewMode: (mode) => {
  // ADR A5/A6: the operator rail is absent, not disabled — refuse to enter
  // it while node control is unavailable (the toggle is hidden then; this
  // guards programmatic and persisted paths).
  if (mode === "operator" && !nodeControlAvailable(getState())) return;
  if (!isClientMode(getState())) saveViewMode(mode);
  update((prev) => {
    const filter = filterOf(prev);
    // Keep the body on the chosen rail: if the current screen belongs to the
    // other rail (or is a shell screen), land on this rail's default surface.
    const screen =
      sectionForScreen(prev.screen) === mode && moduleAvailable(prev.screen, filter)
        ? prev.screen
        : defaultScreenForSection(mode, filter);
    return { viewMode: mode, screen };
  });
},
```

- [ ] **Step 6: `Sidebar.tsx` — conditional toggle**

Header comment (lines 1-5) becomes:

```ts
// The 74px icon rail: brand, the view-mode toggle (only while node control is
// available — ADR A5/A6: the NODE rail is a conditional surface, absent for
// clients and non-owners, not disabled), one entry per module of the active
// rail, and settings. Within a rail, in-view role checks own op-level
// authority.
```

Delete `CLIENT_MODES` (lines 31-34) and the `modes` prop of `ModeToggle` (it maps `LOCAL_MODES` directly). In `Sidebar()`:

```ts
export function Sidebar() {
  const { state, actions } = useDucktape();
  const canControl = nodeControlAvailable(state);
  // Without node control any persisted "operator" mode falls back to the
  // account rail — the NODE surface is absent, so it cannot be selected.
  const mode = canControl ? state.viewMode : "user";
  const rail = modulesInSection(mode, {
    nodeControl: canControl,
    clientMode: isClientMode(state),
  });
```

and the toggle render becomes:

```tsx
{canControl && <ModeToggle mode={mode} onSelect={actions.setViewMode} />}
```

Import `nodeControlAvailable` alongside `isClientMode` from `../store/state`.

- [ ] **Step 7: `TrayPopover.tsx` — rail memo (~line 112)**

```ts
const rail = useMemo(
  () =>
    (snap.client
      ? modulesInSection("user", { nodeControl: false, clientMode: true })
      : [...MODULES].sort((a, b) => a.nav.order - b.nav.order)
    ).filter((m) => m.id !== "status"),
  [snap.client],
);
```

- [ ] **Step 8: Verify — tests + typecheck**

Run: `npx vitest run src/test/nav.test.ts` — Expected: PASS (9 tests).
Run: `npm run typecheck` — Expected: clean. Any residual old-signature caller shows up here; fix it with the same `filterOf`/inline-filter pattern.

- [ ] **Step 9: Commit**

```bash
git add app/src/console/modules app/src/console/store/actions.ts \
  app/src/console/layout/Sidebar.tsx app/src/console/views/tray/TrayPopover.tsx \
  app/src/test/nav.test.ts
git commit -m "feat(app): account-first rail — NODE becomes a conditional surface (ADR A6)

Members/Governance/Explorer move to the account rail; the USER/NODE toggle
renders only while node control is available; a remote client sees no node
chrome.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Spec touch-up + full gates

**Files:**
- Modify: `docs/superpowers/specs/2026-07-14-account-first-ui-design.md` (§3 explorer conditional → resolved; Testing test path → `app/src/test/nav.test.ts`)

**Interfaces:** none.

- [ ] **Step 1: Resolve the spec's explorer conditional**

In §3, replace the "verified during implementation" sentence with: the provider fetches `live.blocks(BLOCKS_KEEP)` unconditionally (`DucktapeProvider.tsx` refresh), so `explorer` stays on the client rail. In Testing, correct the test path to `app/src/test/nav.test.ts`.

- [ ] **Step 2: Full suite + typecheck**

Run (in `app/`): `npm run typecheck && npx vitest run`
Expected: typecheck clean; vitest green except pre-existing simnode-gated skips (they warn and skip without a built `ducktape-simnode`).

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-07-14-account-first-ui-design.md
git commit -m "docs(spec): resolve explorer client-rail conditional + test path

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Fleet QA + PR

**Files:** none (verification + delivery).

- [ ] **Step 1: Fleet QA (UI lane — repo QA doctrine)**

Invoke the repo `qa` skill from the worktree. Verify on a live instance:
1. Owner (managed workspace): USER/NODE toggle present; NODE rail shows exactly Node/Gateway/Modules/Sandbox/Metrics and lands on the node console; USER rail shows Chat/Pages/Files/Browser/Forge/Agents/Members/Governance/Explorer; Explorer renders blocks from the USER rail; Members/Governance admin controls still render for the owner.
2. Remote client: connect a second instance's app to the first instance's node URL via the Remote tab; no toggle, no node surfaces anywhere on the rail; Explorer present and rendering; Members/Governance absent.
3. Persisted-mode fallback: with `localStorage["ducktape.viewMode"]="operator"`, a client boot lands on the account rail (no blank rail).

QA traps that bite here (from the repo ledger): use DOM-shot, not `shot` (renders light); clicks don't focus — use `tauri_focus` + fill; the embedded app is served from `app/dist` under plain cargo, fleet builds once.

- [ ] **Step 2: Push + PR against dev**

```bash
git push -u origin account-first-ui
gh pr create --base dev --title "feat(app): account-first UI — conditional NODE rail (ADR A6)" --body "..."
```

PR body: spec + plan paths, the ADR rule table rows implemented (A6, UI half of A5), QA evidence, and the explicit note that client-mode Members/Governance wait on A3.

---

## Self-review

- Spec coverage: §1 registry re-sort → Task 2 step 4; §2 gate → Task 1; §3 filter + client exclusions + explorer resolution → Task 2 steps 3-4 + Task 3; §4 sidebar → Task 2 step 6; §5 normalization → Task 2 step 5; Testing → Tasks 1/2 tests + Task 4 QA. TrayPopover wasn't in the spec (found during planning) — covered in Task 2 step 7.
- No placeholders; all code shown in full.
- Type consistency: `ModuleFilter` shape `{ nodeControl, clientMode }` used identically in Tasks 1-2 tests and all consumers; `nodeControlAvailable` picks `workspace`/`managed` only.
