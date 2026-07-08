# Settings Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thin the Settings page down to what has no module-view home (identity, custody, preferences, workspace lifecycle), replace module-owned controls with link rows, move the node ops facts to the Node view, and split the 1,242-line SettingsView mono-file.

**Architecture:** React/TypeScript console under `app/src/console`; a single store facade (`useDucktape()` → `{state, actions}`); no router — screens resolve via `state.screen` and `actions.setScreen` (which adopts the owning rail). Settings sections become one file each under `views/settings/`, sharing row/card primitives from a new `parts.tsx`.

**Tech Stack:** React 18, TypeScript, vitest + @testing-library/react (jsdom), bun. Spec: `docs/superpowers/specs/2026-07-07-settings-overhaul-design.md`.

## Global Constraints

- Branch `feat/settings-overhaul`, PR targets `dev` (never `main`).
- Mono-file mandate: no new file over ~600 lines.
- All test/typecheck commands run from `app/`: `bun run test` (vitest run), `bun run typecheck` (tsc).
- The working tree has unrelated uncommitted docs changes (`docs/adr/...quack...`, `docs/pages/...module-model...`) and an untracked spec — NEVER `git add -A`; always add exact paths.
- Views only touch the store facade: `useDucktape()` → `{state, actions}` — never reach around it.
- Copy style: sentence-case labels, mono uppercase section labels (`WORKSPACE`, `PREFERENCES`).

---

### Task 1: Persist the accent preference

The accent is currently in-memory only (`actions.ts:823` is a bare `patch`) and resets on restart. Mirror the `ducktape.viewMode` localStorage pattern.

**Files:**
- Modify: `app/src/console/store/state.ts` (persistence helpers near `DEFAULT_ACCENT` at :252; initial state at :362)
- Modify: `app/src/console/store/actions.ts` (`setAccent` at :823; value-import block from `./state` ending :52)
- Test: `app/src/console/store/accent-persistence.test.ts` (create)

**Interfaces:**
- Produces: `loadAccent(): string`, `saveAccent(accent: string): void` exported from `state.ts`. `createInitialState().accent` hydrates from storage.

- [ ] **Step 1: Write the failing test**

Create `app/src/console/store/accent-persistence.test.ts`:

```tsx
import { afterEach, describe, expect, it } from "vitest";

import { createInitialState, DEFAULT_ACCENT, loadAccent, saveAccent } from "./state";

afterEach(() => {
  localStorage.clear();
});

describe("accent persistence", () => {
  it("round-trips a saved accent", () => {
    saveAccent("#3d63b8");
    expect(loadAccent()).toBe("#3d63b8");
  });

  it("falls back to the default on a missing or malformed value", () => {
    expect(loadAccent()).toBe(DEFAULT_ACCENT);
    localStorage.setItem("ducktape.accent", "javascript:alert(1)");
    expect(loadAccent()).toBe(DEFAULT_ACCENT);
  });

  it("hydrates the initial state accent from storage", () => {
    saveAccent("#3f7d54");
    expect(createInitialState().accent).toBe("#3f7d54");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && bun run test src/console/store/accent-persistence.test.ts`
Expected: FAIL — `loadAccent`/`saveAccent` are not exported.

- [ ] **Step 3: Implement persistence in state.ts**

In `app/src/console/store/state.ts`, directly below `export const DEFAULT_ACCENT = "#a05a3c";` (:252), add (same comment style as the view-mode block):

```ts
// ── Accent persistence ──────────────────────────────────
//
// The chosen accent survives restarts. Values are validated as #rrggbb on
// load so a corrupt/foreign string can never reach inline styles.
const ACCENT_KEY = "ducktape.accent";

export const loadAccent = (): string => {
  try {
    const raw = localStorage.getItem(ACCENT_KEY);
    return raw && /^#[0-9a-f]{6}$/i.test(raw) ? raw : DEFAULT_ACCENT;
  } catch {
    return DEFAULT_ACCENT; // storage unavailable (private mode / quota)
  }
};

export const saveAccent = (accent: string): void => {
  try {
    localStorage.setItem(ACCENT_KEY, accent);
  } catch {
    // persistence is best-effort; a failed write just doesn't survive restart.
  }
};
```

In `createInitialState()` (:362) change `accent: DEFAULT_ACCENT,` → `accent: loadAccent(),`.

In `app/src/console/store/actions.ts`: add `saveAccent,` to the value-import list from `./state` (alphabetical, block ends at :52), and change :823 from
`setAccent: (accent) => patch({ accent }),` to:

```ts
    setAccent: (accent) => {
      saveAccent(accent);
      patch({ accent });
    },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd app && bun run test src/console/store/accent-persistence.test.ts`
Expected: PASS (3 tests). Also run `bun run typecheck` — clean.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/store/state.ts app/src/console/store/actions.ts app/src/console/store/accent-persistence.test.ts
git commit -m "fix(settings): persist the accent preference across restarts"
```

---

### Task 2: Split the SettingsView mono-file (pure move — no behavior change)

`SettingsView.tsx` is 1,242 lines. Move each section into its own file; every existing test in `SettingsView.test.tsx` must stay green untouched. Copy code bodies **verbatim** from the line ranges below (current file as of commit bc23840); only the import headers are new.

**Files:**
- Create: `app/src/console/views/settings/parts.tsx` — shared primitives
- Create: `app/src/console/views/settings/IdentityCard.tsx`
- Create: `app/src/console/views/settings/DevicesSection.tsx`
- Create: `app/src/console/views/settings/WorkspaceSection.tsx` (holds `NetworkSection`, renamed export)
- Create: `app/src/console/views/settings/PreferencesSection.tsx`
- Create: `app/src/console/views/settings/DangerZone.tsx`
- Modify: `app/src/console/views/settings/SettingsView.tsx` → composition only
- Test: existing `app/src/console/views/settings/SettingsView.test.tsx` (unchanged this task)

The import headers below are the expected final form; if `tsc` flags one as unused after the verbatim move, prune it — the typecheck gate in Step 8 is the source of truth.

**Interfaces:**
- Produces (named exports):
  - `parts.tsx`: `monoValue`, `smallMono` (CSSProperties), `copyText(text: string): void`, `SectionLabel`, `GroupCard`, `InfoRow`, `ControlRow`, `HoverButton`, `outlineButton`, `darkButton` (CSSProperties)
  - `IdentityCard.tsx`: `IdentityCard()` (keeps `workspaceRole`, `initialsOf` as private helpers)
  - `DevicesSection.tsx`: `DevicesSection()` (keeps `CustodyPanel`, `UserKeyStatus`, `CustodyPanelKind` private)
  - `WorkspaceSection.tsx`: `WorkspaceSection()` (this task: verbatim NetworkSection body incl. `InviteBlob`, `AdmitControl`, `workspaceDataDir`, `quorumText`, its own private copy of `workspaceRole`)
  - `PreferencesSection.tsx`: `PreferencesSection()` (keeps `ACCENTS`, `AccentPicker` private)
  - `DangerZone.tsx`: `DangerZone()` (keeps `DangerRow` private)

- [ ] **Step 1: Create `parts.tsx`**

Header, then move these verbatim from `SettingsView.tsx`: `monoValue` (:32-39), `smallMono` (:41-47), `copyText` (:49-51), `SectionLabel` (:107-128), `GroupCard` (:130-144), `InfoRow` (:146-173), `ControlRow` (:175-214), `HoverButton` (:216-250), `outlineButton` (:252-260), `darkButton` (:262-270). Export every one of them (add `export` to each `const`/`function`).

```tsx
// Shared row/card primitives for the Settings sections. Pure presentation —
// no store access; sections compose these and own their own data.

import { useState, type CSSProperties, type ReactNode } from "react";

import { color, font, radius } from "../../theme/tokens";
```

- [ ] **Step 2: Create `IdentityCard.tsx`**

Move `workspaceRole` (:73-105), `initialsOf` (:61-71), and `IdentityCard` (:306-406) verbatim; export only `IdentityCard`.

```tsx
// YOUR IDENTITY — the person: display name (the canonical editor for the
// origin-gated profiles SetName), role tier badge, and this device's node key.

import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { copyText, HoverButton, outlineButton, smallMono } from "./parts";
```

- [ ] **Step 3: Create `DevicesSection.tsx`**

Move verbatim: the section comment block (:565-574), `UserKeyStatus` (:575), `CustodyPanelKind` (:577-580), `CustodyPanel` (:582-596), and `DevicesSection` (:598-964). Export only `DevicesSection`.

```tsx
import { useCallback, useEffect, useState, type ReactNode } from "react";

import { normalizeKey, shortKey } from "../../../domain/names";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import {
  encryptLegacy,
  identityState,
  lockIdentity,
  revealMnemonic,
  unlockIdentity,
} from "../../../domain/user-identity-client";
import { isDesktop } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import { errMessage, MnemonicGrid, PasswordForm } from "../onboarding/IdentityGateForms";
import { ControlRow, darkButton, GroupCard, HoverButton, InfoRow, monoValue, outlineButton, SectionLabel } from "./parts";
```

- [ ] **Step 4: Create `WorkspaceSection.tsx` (verbatim this task; slimmed in Task 3)**

Move verbatim: `workspaceDataDir` (:53), `quorumText` (:55-59), `InviteBlob` (:408-436), `AdmitControl` (:438-471), `NetworkSection` (:473-563) renamed to `WorkspaceSection` (function name only — the rendered `NETWORK` label and all rows stay byte-identical). Also copy `workspaceRole` (:73-105) as a private helper (it feeds the "Node role" row until Task 3 deletes it).

```tsx
import { useState } from "react";

import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { ControlRow, darkButton, GroupCard, HoverButton, InfoRow, monoValue, outlineButton, SectionLabel } from "./parts";
```

- [ ] **Step 5: Create `PreferencesSection.tsx`**

Move verbatim: `ACCENTS` (:24-30), `AccentPicker` (:272-304), `PreferencesSection` (:966-1000). Export only `PreferencesSection`.

```tsx
import { useDucktape } from "../../store/use-ducktape";
import { color } from "../../theme/tokens";
import { ControlRow, GroupCard, SectionLabel } from "./parts";
import { Toggle } from "./Toggle";
```

- [ ] **Step 6: Create `DangerZone.tsx`**

Move verbatim: `DangerRow` (:1002-1065), `DangerZone` (:1067-1205). Export only `DangerZone`.

```tsx
import type { ReactNode } from "react";

import { normalizeKey } from "../../../domain/names";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton, SectionLabel } from "./parts";
```

- [ ] **Step 7: Rewrite `SettingsView.tsx` as composition (person-first order)**

Replace the whole file with:

```tsx
// The Settings screen: composition only. Sections own their content; shared
// row/card primitives live in parts.tsx. Everything a module view owns
// (membership → Members, daemon + ops facts → Node) lives THERE — Settings
// keeps identity, custody, preferences, and workspace lifecycle.

import { color, font } from "../../theme/tokens";
import { DangerZone } from "./DangerZone";
import { DevicesSection } from "./DevicesSection";
import { IdentityCard } from "./IdentityCard";
import { SectionLabel } from "./parts";
import { PreferencesSection } from "./PreferencesSection";
import { WorkspaceSection } from "./WorkspaceSection";

export function SettingsView() {
  return (
    <div
      data-screen-label="Settings"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: "#fcfcfc",
        padding: 22,
        overflowY: "auto",
      }}
    >
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
        Settings
      </div>

      <div style={{ maxWidth: 600 }}>
        <SectionLabel marginTop={18}>YOUR IDENTITY</SectionLabel>
        <IdentityCard />

        <DevicesSection />

        <PreferencesSection />

        <WorkspaceSection />

        <DangerZone />

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
```

Note: `WorkspaceSection` still renders its own `NETWORK` label internally this task; change its `marginTop={18}` prop on that internal label to the default (remove the prop) since it is no longer the first section.

- [ ] **Step 8: Run the full existing settings suite unchanged**

Run: `cd app && bun run test src/console/views/settings/SettingsView.test.tsx && bun run typecheck`
Expected: ALL existing tests PASS with zero test edits (the refactor is behavior-preserving). If any fail, the move diverged — fix the move, not the test.

- [ ] **Step 9: Commit**

```bash
git add app/src/console/views/settings/
git commit -m "refactor(settings): split the SettingsView mono-file by section"
```

---

### Task 3: Thin the workspace section — module views own membership and ops facts

Remove from Settings what Members/Node own; link there instead. `actions.setScreen("members" | "status")` adopts the operator rail automatically (`actions.ts:781-788`).

**Files:**
- Modify: `app/src/console/views/settings/WorkspaceSection.tsx` (full replacement below)
- Test: `app/src/console/views/settings/SettingsView.test.tsx` (first test rewritten + one new test)

**Interfaces:**
- Consumes: `parts.tsx` exports (Task 2), `actions.setScreen(screen: string)`.
- Produces: `WorkspaceSection()` — rows: Network name, Network ID, Switch workspace, Members & invites (→ `setScreen("members")`), Node & daemon (→ `setScreen("status")`). Deletes `InviteBlob`, `AdmitControl`, `workspaceRole` (local copy), `workspaceDataDir`, `quorumText` from this file.

- [ ] **Step 1: Rewrite the first test and add the link-row test**

In `SettingsView.test.tsx`, replace the whole `it("renders the workspace settings and preserves the existing actions", ...)` block (:85-116) with:

```tsx
  it("renders the slimmed workspace card and preserves the existing actions", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { spies } = renderSettings();

    // Workspace facts that still live here.
    expect(screen.getByText("WORKSPACE")).toBeInTheDocument();
    expect(screen.getByText("Acme Research")).toBeInTheDocument();
    expect(screen.getByText("acme#abcd1234")).toBeInTheDocument();

    // Everything a module view owns is gone from Settings: ops facts belong
    // to the Node view, invite/admit to Members.
    expect(screen.queryByText("NETWORK")).not.toBeInTheDocument();
    expect(screen.queryByText(/~\/\.ducktape\/workspaces/)).not.toBeInTheDocument();
    expect(screen.queryByText(/p2p 7420/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/quorum threshold/i)).not.toBeInTheDocument();
    expect(screen.queryByText("Node role")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /reveal invite/i }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/joiner pubkey/i)).not.toBeInTheDocument();

    expect(screen.getByText("YOUR IDENTITY")).toBeInTheDocument();
    expect(screen.getByText(/abcdef012345/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /copy key/i }));
    expect(writeText).toHaveBeenCalledWith(workspace.pubkey);

    const name = screen.getByDisplayValue("Rae");
    fireEvent.change(name, { target: { value: "Ari" } });
    expect(spies.setAuthor).toHaveBeenCalledWith("Ari");
    fireEvent.blur(name);
    expect(spies.setDisplayName).toHaveBeenCalledWith("Ari");

    fireEvent.click(screen.getByRole("button", { name: /set accent #3d63b8/i }));
    expect(spies.setAccent).toHaveBeenCalledWith("#3d63b8");

    fireEvent.click(screen.getByRole("button", { name: /workspaces/i }));
    expect(spies.newWorkspace).toHaveBeenCalled();
  });

  it("links into the module views that own membership and the daemon", () => {
    const { spies } = renderSettings();

    fireEvent.click(screen.getByRole("button", { name: /open members/i }));
    expect(spies.setScreen).toHaveBeenCalledWith("members");

    fireEvent.click(screen.getByRole("button", { name: /open node/i }));
    expect(spies.setScreen).toHaveBeenCalledWith("status");
  });
```

- [ ] **Step 2: Run tests to verify the new ones fail**

Run: `cd app && bun run test src/console/views/settings/SettingsView.test.tsx`
Expected: FAIL — `WORKSPACE` not found (label still `NETWORK`), `Open Members`/`Open Node` buttons missing, data-dir/ports/quorum still rendered.

- [ ] **Step 3: Replace `WorkspaceSection.tsx` entirely**

```tsx
// The workspace card: the facts that name the active workspace, the switcher,
// and link rows into the operator surfaces that own everything else — Members
// owns invite/admit, Node owns the daemon and its ops facts (ports, data dir,
// quorum). Settings deliberately does NOT duplicate those controls.

import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import { ControlRow, GroupCard, HoverButton, InfoRow, monoValue, outlineButton, SectionLabel } from "./parts";

export function WorkspaceSection() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;

  return (
    <>
      <SectionLabel>WORKSPACE</SectionLabel>
      <GroupCard>
        <InfoRow
          label="Network name"
          value={
            <span style={{ font: `500 12px ${font.mono}`, color: color.inkSofter }}>
              {workspace?.name ?? "Remote node"}
            </span>
          }
        />
        <InfoRow
          label="Network ID"
          value={
            <span style={monoValue} title={workspace?.chainId}>
              {workspace?.chainId ?? "not available"}
            </span>
          }
        />
        <ControlRow
          title="Switch workspace"
          desc="Create, join, or select another local workspace."
          control={
            <HoverButton
              ariaLabel="Workspaces"
              onClick={actions.newWorkspace}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Workspaces
            </HoverButton>
          }
        />
        <ControlRow
          title="Members & invites"
          desc="Invite, admit, and manage members from the Members view."
          control={
            <HoverButton
              ariaLabel="Open Members"
              onClick={() => actions.setScreen("members")}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Open Members
            </HoverButton>
          }
        />
        <ControlRow
          title="Node & daemon"
          desc="Start or stop the daemon and inspect ports, data dir, and quorum from the Node view."
          last
          control={
            <HoverButton
              ariaLabel="Open Node"
              onClick={() => actions.setScreen("status")}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Open Node
            </HoverButton>
          }
        />
      </GroupCard>
    </>
  );
}
```

This deletes `InviteBlob`, `AdmitControl`, the `LIVE_JOIN_SUPPORTED` import/branch, the local `workspaceRole`, `workspaceDataDir`, `quorumText`, and the `useState` import from this file. Nothing else imports them from here (verify: `grep -rn "InviteBlob\|AdmitControl" app/src` → only MembersView's own copies).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd app && bun run test src/console/views/settings/SettingsView.test.tsx && bun run typecheck`
Expected: PASS, all tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/settings/WorkspaceSection.tsx app/src/console/views/settings/SettingsView.test.tsx
git commit -m "feat(settings): workspace card links to Members/Node instead of duplicating them"
```

---

### Task 4: Remove the Local node toggle from Preferences (Node view owns the daemon)

**Files:**
- Modify: `app/src/console/views/settings/PreferencesSection.tsx` (full replacement below)
- Delete: `app/src/console/views/settings/Toggle.tsx` (its ONLY consumer was this row — verify with `grep -rn "settings/Toggle\|from \"./Toggle\"" app/src` → expect no matches outside settings)
- Test: `app/src/console/views/settings/SettingsView.test.tsx`

**Interfaces:**
- Produces: `PreferencesSection()` with the accent row only.

- [ ] **Step 1: Add the failing assertions**

In the `it("renders the slimmed workspace card...")` test from Task 3, right after the `queryByLabelText(/joiner pubkey/i)` assertion, add:

```tsx
    // The daemon toggle moved to the Node view with the rest of the ops surface.
    expect(screen.queryByText("Local node")).not.toBeInTheDocument();
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && bun run test src/console/views/settings/SettingsView.test.tsx`
Expected: FAIL — "Local node" is still rendered by PreferencesSection.

- [ ] **Step 3: Replace `PreferencesSection.tsx` entirely**

```tsx
// Local console preferences. Daemon start/stop lives on the Node view — the
// operator surface that owns it — not here.

import { useDucktape } from "../../store/use-ducktape";
import { color } from "../../theme/tokens";
import { ControlRow, GroupCard, SectionLabel } from "./parts";

const ACCENTS = [
  color.accent,
  color.accentAlt1,
  color.accentAlt2,
  color.purple,
  color.red,
] as const;

function AccentPicker({
  value,
  onPick,
}: {
  value: string;
  onPick: (accent: string) => void;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
      {ACCENTS.map((accent) => (
        <button
          key={accent}
          type="button"
          aria-label={`Set accent ${accent}`}
          title={accent}
          onClick={() => onPick(accent)}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 22,
            height: 22,
            borderRadius: "50%",
            background: accent,
            boxShadow:
              value === accent
                ? `0 0 0 2px ${color.paper}, 0 0 0 4px ${accent}`
                : `0 0 0 1px ${color.borderStrong}`,
          }}
        />
      ))}
    </div>
  );
}

export function PreferencesSection() {
  const { state, actions } = useDucktape();
  return (
    <>
      <SectionLabel>PREFERENCES</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Accent"
          desc="Used for active navigation, focus, and primary controls."
          last
          control={<AccentPicker value={state.accent} onPick={actions.setAccent} />}
        />
      </GroupCard>
    </>
  );
}
```

Then delete the now-orphaned toggle: `git rm app/src/console/views/settings/Toggle.tsx`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd app && bun run test src/console/views/settings/SettingsView.test.tsx && bun run typecheck`
Expected: PASS; typecheck clean (proves nothing else imported Toggle).

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/settings/PreferencesSection.tsx app/src/console/views/settings/SettingsView.test.tsx
git commit -m "feat(settings): drop the Local node toggle — the Node view owns the daemon"
```

(The `git rm` in Step 3 already staged Toggle.tsx's deletion.)

---

### Task 5: Give the evicted ops facts a home on the Node view

Data dir, ports, and quorum threshold lost their Settings rows; the Node (Status) view is their operator-rail home. Role/daemon control already live there.

**Files:**
- Create: `app/src/console/views/status/NodeFactsCard.tsx`
- Modify: `app/src/console/views/status/StatusView.tsx` (render after `<AccessCard />`, currently :854 inside the `YOUR ACCESS` section)
- Test: `app/src/console/views/status/StatusView.test.tsx` (uses the existing `renderStatus` harness; `workspace` fixture already has ports 7420/8844/9020 and id `acme-research`)

**Interfaces:**
- Consumes: `state.workspace` (`id`, `ports.{listen,http,rpc}`, `member`), `state.members`.
- Produces: `NodeFactsCard()` named export.

- [ ] **Step 1: Write the failing test**

Add to `StatusView.test.tsx` inside the existing top-level `describe`:

```tsx
  it("shows the node ops facts that moved here from Settings", () => {
    renderStatus({ members: [workspace.pubkey, PEER_B, RESIDENT_C] });

    expect(screen.getByText("Data dir")).toBeInTheDocument();
    expect(
      screen.getByText("~/.ducktape/workspaces/acme-research"),
    ).toBeInTheDocument();
    expect(screen.getByText("Ports")).toBeInTheDocument();
    expect(screen.getByText("p2p 7420 · http 8844 · rpc 9020")).toBeInTheDocument();
    expect(screen.getByText("Quorum threshold")).toBeInTheDocument();
    // floor(3 * 2/3) + 1 = 3 of the 3 validators.
    expect(screen.getByText("3 of 3 validators")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && bun run test src/console/views/status/StatusView.test.tsx`
Expected: FAIL — "Data dir" not found.

- [ ] **Step 3: Create `NodeFactsCard.tsx`**

```tsx
// Node-local operational facts — the operator-rail home for what Settings
// used to duplicate: where the workspace lives on disk, which ports the node
// binds, and the quorum the validator set needs. Read-only projections of
// the active workspace and roster.

import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const workspaceDataDir = (id: string): string => `~/.ducktape/workspaces/${id}`;

const quorumText = (count: number): string => {
  if (count <= 0) return "not exposed";
  const threshold = Math.floor((count * 2) / 3) + 1;
  return `${threshold} of ${count} validator${count === 1 ? "" : "s"}`;
};

function FactRow({
  label,
  value,
  last,
}: {
  label: string;
  value: string;
  last?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "11px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <span style={{ font: `500 12px ${font.sans}`, color: color.inkSoft }}>
        {label}
      </span>
      <span
        style={{
          marginLeft: "auto",
          minWidth: 0,
          font: `400 11.5px ${font.mono}`,
          color: color.muted,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={value}
      >
        {value}
      </span>
    </div>
  );
}

export function NodeFactsCard() {
  const { state } = useDucktape();
  const workspace = state.workspace;
  const validatorCount = state.members.length || (workspace?.member ? 1 : 0);
  const portLine = workspace
    ? `p2p ${workspace.ports.listen} · http ${workspace.ports.http} · rpc ${workspace.ports.rpc}`
    : "not available";

  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        overflow: "hidden",
      }}
    >
      <FactRow
        label="Data dir"
        value={workspace ? workspaceDataDir(workspace.id) : "not available"}
      />
      <FactRow label="Ports" value={portLine} />
      <FactRow label="Quorum threshold" value={quorumText(validatorCount)} last />
    </div>
  );
}
```

- [ ] **Step 4: Render it in `StatusView.tsx`**

Add `import { NodeFactsCard } from "./NodeFactsCard";` to the imports, and change the `YOUR ACCESS` block (:851-855) from:

```tsx
      <SectionLabel>YOUR ACCESS</SectionLabel>
      <div style={{ marginTop: 9 }}>
        <AccessCard />
      </div>
```

to:

```tsx
      <SectionLabel>YOUR ACCESS</SectionLabel>
      <div style={{ marginTop: 9 }}>
        <AccessCard />
      </div>
      <div style={{ marginTop: 10 }}>
        <NodeFactsCard />
      </div>
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd app && bun run test src/console/views/status/StatusView.test.tsx && bun run typecheck`
Expected: PASS (existing 3 + new 1); typecheck clean.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/status/NodeFactsCard.tsx app/src/console/views/status/StatusView.tsx app/src/console/views/status/StatusView.test.tsx
git commit -m "feat(status): node facts card — data dir, ports, quorum move from Settings"
```

---

### Task 6: Fix stale cross-references to Settings

**Files:**
- Modify: `app/src/console/views/onboarding/OnboardingGate.tsx:199`

- [ ] **Step 1: Update the copy**

In `OnboardingGate.tsx` (:195-199) change:

```
            Joining a running network is temporarily unavailable. Found a new
            network to get started, and invite others from Settings.
```

to:

```
            Joining a running network is temporarily unavailable. Found a new
            network to get started, and invite others from the Members view.
```

- [ ] **Step 2: Sweep for other stale references**

Run: `grep -rn "from Settings\|in Settings" app/src --include="*.tsx" --include="*.ts"`
Expected: no remaining hits that point at removed Settings functionality (invite/admit/daemon). Fix any found the same way (point at Members or Node).

- [ ] **Step 3: Run the app suite and commit**

Run: `cd app && bun run test`
Expected: full suite PASS.

```bash
git add app/src/console/views/onboarding/OnboardingGate.tsx
git commit -m "fix(onboarding): invites now live on the Members view, not Settings"
```

---

### Task 7: Full verification and PR

- [ ] **Step 1: Full app gate**

Run: `cd app && bun run typecheck && bun run test`
Expected: clean typecheck; the FULL suite passes (settings, status, members, store).

- [ ] **Step 2: Live QA in the real desktop app**

Use the `tauri-debug` skill (headless Xvfb recipe) to drive the running app:
1. Screenshot the Settings screen — verify section order YOUR IDENTITY → DEVICES → PREFERENCES → WORKSPACE → DANGER ZONE, no NETWORK section, no invite/admit rows, no Local node toggle.
2. Click "Open Members" — verify the screen switches to Members on the operator rail (invite/admit cards live there).
3. Click "Open Node" — verify the Node view shows the new facts card (data dir, ports, quorum) under YOUR ACCESS.
4. Pick a non-default accent, restart the app (`dev.sh` restart per skill), verify the accent survived.

- [ ] **Step 3: Push and open the PR against dev**

```bash
git push -u origin feat/settings-overhaul
gh pr create --base dev --title "feat(settings): thin Settings — module views own their domains" --body "$(cat <<'EOF'
## Summary
- Settings keeps only what has no module-view home: identity, device/key custody, preferences, workspace lifecycle
- Invite/admit rows removed (Members view owns them); Local-node toggle removed (Node view owns the daemon); link rows navigate there instead
- Node ops facts (data dir, ports, quorum threshold) move to a new facts card on the Node view
- Accent preference now persists across restarts (localStorage, validated on load)
- 1,242-line SettingsView split into per-section files (parts/Identity/Devices/Workspace/Preferences/DangerZone); orphaned Toggle.tsx deleted

Spec: docs/superpowers/specs/2026-07-07-settings-overhaul-design.md
Plan: docs/superpowers/plans/2026-07-07-settings-overhaul.md

## Test plan
- [ ] `bun run typecheck && bun run test` in app/
- [ ] Live tauri-debug QA: new section order, link-row navigation with rail adoption, facts card on Node view, accent survives restart

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
