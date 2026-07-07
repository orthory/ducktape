# Settings Overhaul — Remove Duplication with Module Views

**Date:** 2026-07-07
**Status:** Approved design
**Branch:** `feat/settings-overhaul` (targets `dev`)

## Problem

`app/src/console/views/settings/SettingsView.tsx` has grown to 1,242 lines (past
the repo's ~600-line mono-file cap) and duplicates functionality that module
views on the operator rail already own:

| Duplicated in Settings | Canonical module-view home |
| --- | --- |
| "Invite a member" (reveal invite blob) | Members — `MembersView.tsx` AdminActions card |
| "Admit a joiner" (pubkey form) | Members — AdmitActions card + one-click pending-join approve |
| "Local node" start/stop toggle | Node — `StatusView.tsx` NodeHeader Start/Stop |
| Ports / Data dir / Quorum / Node role facts | Node — the operator status surface (role already shown; the rest belongs there) |

Settings and the *Modules* view barely intersect — the real duplication is with
the **Members** and **Node** module views. Two smaller defects ride along:

- The accent preference is in-memory only (`actions.ts` `setAccent` is a bare
  `patch`) — it silently resets on every restart.
- The role-presentation helper (`workspaceRole`) is re-implemented in
  Settings, StatusView, and MembersView.

## Direction (chosen from three approaches)

**A. Thin Settings (chosen).** Module views keep their domains; Settings keeps
only what has no module home: personal identity, device/key custody, local
preferences, and workspace lifecycle. Removed controls are replaced by link
rows that navigate to the owning view (`setScreen` already adopts the correct
rail, `actions.ts:781-788`).

Rejected alternatives:

- **B. Consolidate into Settings** (tabbed super-settings, slim the module
  views): inverts the app's architecture — the operator rail *is* the home for
  network/membership operations; Settings would become a parallel module
  system.
- **C. Layout-only reshuffle** (tabs/sub-nav, keep every option): does not
  remove the duplication, which is the point of the overhaul.

## Target Settings IA

Single scrolling page (unchanged shell, `maxWidth: 600`), person-first order:

```
Settings
├─ YOUR IDENTITY   IdentityCard — avatar, display-name edit (canonical home),
│                  role tier badge, node pubkey line, Copy key
├─ DEVICES         custody section, unchanged behavior — this device, bind
│                  state, other devices, user key, lock/unlock, set password,
│                  reveal recovery phrase
├─ PREFERENCES     Accent picker only (persisted — see below)
├─ WORKSPACE       (renamed from NETWORK, slimmed)
│   ├─ Network name           [read]
│   ├─ Network ID             [read]
│   ├─ Switch workspace       [button → OnboardingGate]
│   ├─ Members & invites      [link row → setScreen("members")]
│   └─ Node & daemon          [link row → setScreen("status")]
└─ DANGER ZONE     unchanged — leave / forget / force-forget
```

Removed outright from Settings: Invite a member (+ InviteBlob), Admit a joiner
(+ AdmitControl), the whole `LIVE_JOIN_SUPPORTED` branch, the Local node
toggle, and the Data dir / Ports / Quorum threshold / Node role info rows.

Members' inline self-rename stays: it is a contextual shortcut into the same
origin-gated `profiles` op whose canonical editor is the Settings identity
card — module-side affordance, not settings duplication.

## Module-view changes

- **Node (StatusView):** add the three facts that lose their Settings home —
  Data dir, Ports, Quorum threshold — as rows on the existing access/identity
  card (which already shows the node role). Nothing else moves; Start/Stop is
  already there.
- **Members (MembersView):** no changes — it is already the canonical home for
  invite/admit.
- **OnboardingGate:** update the "invite others from Settings" copy
  (`OnboardingGate.tsx:199`) to point at the Members view.

## Fixes riding along

- **Accent persistence:** `setAccent` writes `localStorage["ducktape.accent"]`;
  state init reads it back (validated as a `#rrggbb` hex, falling back to
  `DEFAULT_ACCENT`), mirroring the existing `ducktape.viewMode` pattern in
  `state.ts`.
- **Shared role helper:** extract one role-presentation helper (tier label +
  colors from `{founder, member}`) into
  `app/src/console/lib/workspace-role.ts` and use it from Settings and
  StatusView (MembersView adoption optional if its variant matches).

## File layout (mono-file mandate)

Split `views/settings/` by section; `SettingsView.tsx` becomes composition
only:

| File | Contents | ~Lines |
| --- | --- | --- |
| `SettingsView.tsx` | page shell + section composition | 80 |
| `parts.tsx` | SectionLabel, GroupCard, InfoRow, ControlRow, HoverButton, button styles, copyText | 260 |
| `IdentityCard.tsx` | YOUR IDENTITY card | 160 |
| `DevicesSection.tsx` | custody section (moved verbatim) | 400 |
| `WorkspaceSection.tsx` | slimmed workspace card + link rows | 130 |
| `PreferencesSection.tsx` | accent picker | 50 |
| `DangerZone.tsx` | danger rows + confirms | 220 |
| `Toggle.tsx` | unchanged (AgentView imports it) | — |

## Data flow / error handling

No new data paths: link rows call `actions.setScreen`, which patches
`{screen, viewMode}`; all removed controls delete code without touching the
store actions they called (Members/Status still use them). The custody
section's error handling moves verbatim. Accent load failure (missing/invalid
stored value) falls back silently to the default.

## Testing

- Update `SettingsView.test.tsx`: drop invite/admit/local-node/network-fact
  cases; add link-row navigation (asserts `setScreen` target + rail adoption),
  slimmed workspace card contents, and accent persistence (write + rehydrate).
- StatusView tests: assert the three new fact rows render from
  `state.workspace`.
- Existing MembersView tests unchanged (guard that invite/admit still live
  there).
- TDD: failing tests first per repo practice; `bun test` in `app/`.

## Out of scope

- Modules-view ↔ Node-view Merkle-root duplication (informational, both
  operator surfaces; separate concern).
- Any tabbed/sub-nav Settings shell (unneeded at five compact sections).
- Workspace delete-by-id in OnboardingGate (different context from active
  workspace danger zone).
