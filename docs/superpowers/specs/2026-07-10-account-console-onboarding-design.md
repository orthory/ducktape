# Account console + onboarding redesign

Date: 2026-07-10 · Branch: `feat/account-console` · Target: `dev`

## Problem

The backend already models the account↔node split the product wants — the
`identity` module (v2 account format) keeps an **account** umbrella owning many
**member keys** (ed25519 seed / P-256 / WebAuthn passkey) and many **nodes**,
with `BindNode` / `UnbindNode` / `AddMemberKey` / `RemoveMemberKey` /
`SetAccountName` all live and tested (`crates/system/identity`). The desktop
frontend never caught up:

- **Onboarding is five unnumbered screens with no shared frame.** The identity
  gate (password → 24-word grid → confirm) and the workspace gate are two
  disconnected flows; the joiner waiting room hard-codes an orphaned
  "STEP 2 / 3" (`JoinProgress.tsx:180`) that nothing else references.
- **Three "identity" notions collide on one Settings screen.** The card titled
  YOUR IDENTITY shows the *node* key (`IdentityCard.tsx`), while DevicesSection
  separately shows the user key, the account, and bind state. Roles render as
  Genesis/Admitted/Read-Only on Members but genesis/member/guest on the card.
- **Account management has no home.** Member-key add/remove and node unbind
  exist as Tauri signing verbs (`user_sign_add_member`, `user_sign_unbind`)
  with **zero calling UI** — device linking and lost-node eviction are
  CLI-only. The ACCOUNT card in Settings is read-only.
- **Multi-node accounts are invisible.** `AccountView.nodes` lists a person's
  nodes; the only rendering is a text row ("Other devices") and Members-page
  grouping. There is no place to see or manage "my nodes, including my
  validators."
- **A fresh device silently founds a duplicate account.** Auto-bind
  (`auto-bind.ts`) submits `BindNode` on first connect; for a key the network
  has never seen, the module *founds a new account*. A second machine that
  should have joined an existing account instead becomes a stranger — and there
  is no merge op to repair it.

## Vocabulary (locked, user-facing)

- **Account** — the person. One 24-word recovery phrase. Concretely: the
  machine's `~/.ducktape/user.key` is one *member key* of the on-chain account
  record (display name + member keys + bound nodes). The password is
  device-local encryption only.
- **Device** — a machine holding a member key of the account.
- **Node** — a per-workspace daemon with its own disposable key; its standing
  is parked → resident → validator. A node *belongs to* an account (bind); it
  is never "your identity."
- **Workspace** — a network you belong to (unchanged).

The words "user key" and the bare "identity" disappear from the UI.

## Approaches considered

**A. Tabbed mega-Settings** (Account/Node/Workspace tabs inside Settings).
Least navigation change, but it buries account management inside the screen
whose conflation is the problem, and rebuilds a mono-surface.

**B. Shell-level Account console + thin Settings — chosen.** Account becomes a
peer of Settings: a shell-level screen (not a rail module) opened from a
pinned avatar button at the sidebar bottom, directly above the Settings gear.
Settings keeps only preferences + workspace lifecycle. Mirrors the proven
Settings pattern (`resolveScreen`), needs no `NavSection` widening, and makes
the person-vs-operator split physical: Account = the person, Node page = this
daemon, Members = the network's people.

**C. Account as a `user`-rail module.** Makes the console a peer of
chat/pages/files, but it is not a collaboration surface and would occupy
content-rail real estate.

## Design

### 1. Onboarding: one flow, three steps

A shared stepper chrome (`Account → Workspace → Connect`) wraps the three
existing stages so first-run reads as one flow. The stage machinery is kept —
this is a re-skin plus copy overhaul, not a new state machine.

- **Step 1 — Account** (`IdentityGate`, 4-state machine kept:
  `absent | plaintext | locked | unlocked`, resume screen kept).
  - `absent` now offers three entries: **Create account** / **Restore from
    recovery phrase** / **Link this device to an existing account**.
  - Create: display name (new, optional) → password → phrase grid → confirm 3
    words. The name is held locally (`ducktape.pendingDisplayName`) and applied
    on-chain after the first successful bind (`SetAccountName`); the Account
    console shows the live name, so a failed application is visible and
    correctable there.
  - Link-device: creates the machine key like Create (no phrase ceremony — the
    phrase lives on the other device), then shows the **link-device wizard**
    (§3) and sets `ducktape.accountLinkPending`, which changes auto-bind
    semantics (§4). The user can proceed to Step 2 immediately; linking and
    joining are independent.
  - The `locked` screen keeps its "Skip for now" escape but states the
    consequence honestly: *"Until you unlock, nodes you start stay unlinked to
    your account."*
- **Step 2 — Workspace** (`OnboardingGate`): same create/join/remote tabs and
  workspace list, rendered under the stepper with account-first copy.
- **Step 3 — Connect** (`JoinProgress`): the joiner waiting room becomes step
  3-of-3 (deleting the hard-coded "STEP 2 / 3"), and gains a **bind row**
  ("Linked to your account ✓ / linking… / unlocked required") so the silent
  auto-bind becomes visible. Founders skip step 3 (connect is immediate).

### 2. Account console (new shell-level screen `"account"`)

Opened from an avatar button pinned at the sidebar bottom (initials of the
account name), above the Settings gear. `resolveScreen` gains an `"account"`
branch beside `"settings"`. When no workspace is connected the console renders
machine-scoped cards only (custody + local nodes) with an honest banner —
account data is chain-scoped and needs a connected workspace.

Sections, one file each under `app/src/console/views/account/`:

- **ProfileCard** — avatar initials, editable display name (moves from
  Settings' IdentityCard; routes through `setAccountName` when bound, profiles
  otherwise, as today), account id + copy, and a note that names are
  per-workspace-network.
- **CustodyCard** — the custody state machine moved whole from
  `DevicesSection`: lock state, unlock / lock, set password, reveal recovery
  phrase (reusing `IdentityGateForms` widgets).
- **DeviceKeysCard** — the account's member keys with scheme labels
  (Seed key / Security key / Passkey) and a this-device marker.
  **Link another device** opens the inviter-side wizard (§3). **Remove key**
  submits `RemoveMemberKey` behind a ConfirmDialog (the module refuses to
  drop the last key; surface that error verbatim).
- **NodesCard** — the multi-node account made visible, two honest groups:
  - *On this network*: `AccountView.nodes` with standing badges — Validator /
    Resident (from `state.members` / `state.residents`) / Offline-unknown —
    a this-device marker, and **Unbind** behind a ConfirmDialog (first consumer
    of `user_sign_unbind`; copy: for lost devices — the node keeps running but
    stops being yours).
  - *On this machine*: the workspace registry (name, network, active marker,
    Switch action → existing `selectWorkspace`).

### 3. Device-linking ceremony (app-only, copy/paste blobs)

Two-sided wizard over the existing Tauri verbs; no network path between the
devices is assumed. Device A = existing member (unlocked, connected). Device
B = new machine.

1. **A** (DeviceKeysCard → Link a device) shows a **challenge blob**:
   `ducktape-link-challenge-v1:<b64 json{chain_id, account_id, nonce, name}>`
   (all public data, read from A's connected node).
2. **B** (link wizard, from onboarding Step 1 or its own Account console)
   pastes the challenge, signs possession locally (`user_sign_possession` —
   no connection needed), and shows a **response blob**:
   `ducktape-link-response-v1:<b64 json{key, kind, possession}>`.
3. **A** pastes the response, authorizes (`user_sign_add_member`), and submits
   `AddMemberKey` through its node. Done — B's key is a member.

B then joins the workspace normally (invite); auto-bind resolves B's key to
the account and binds B's node into it. Known limit, surfaced honestly: the
possession signature is nonce-scoped, so any account op landing between steps
1 and 3 invalidates it — the error says "re-run the link from step 1." Blob
format is app-local (both ends are this app), versioned, zero consensus
surface.

### 4. Auto-bind: never found a duplicate account

`auto-bind.ts` gains one guard: when `ducktape.accountLinkPending` is set, it
first queries `accountOfMember(localKey)`; if the key is not yet a member of
any account it **skips** the bind (retrying on subsequent refreshes) instead
of founding a fresh account; once membership appears it binds and clears the
flag. The normal create-account path is unchanged (absent flag ⇒ today's
behavior, including first-bind account founding).

### 5. Settings / Node rearrangement

- **Settings** thins to: Preferences · Workspace (network info + link rows:
  Account, Members & invites, Node & daemon, Switch workspace) · Danger zone.
  `IdentityCard` and `DevicesSection` leave Settings.
- **Node page** gains a **ThisNodeCard** at the top of Overview: node key +
  copy, workspace role badge, and bind state ("Linked to *name*" / "Not
  linked — unlock your account to link"). The node key finally lives on the
  node's page.
- **Store rename**: `state.userKey` → `state.accountId` (the provider comment
  already admits it holds the account id); `nodeUsers`' `userKey` field
  follows. Mechanical, app-wide.
- **Label sweep**: Members page keeps its behavior (it already groups nodes
  under a shared account) with copy aligned to Account/Device/Node vocabulary;
  role badge names unify on the Members-page set (Genesis/Member/Read-Only).

## Explicit non-goals

- **Passkey/WebAuthn enrollment UI** — the scheme renders as a label when
  present; enrollment needs platform WebAuthn work (unproven under WebKitGTK)
  and is its own slice.
- **Account merge** — no consensus op exists; prevention (§4) is the fix.
- **Per-account quorum/vote aggregation** — consensus stays per-node by the
  identity-split ADR's deliberate v1 decision. This is UI truth-telling, not
  a governance change.
- **Web-build onboarding** — the identity gate stays a desktop concern.
- **Multi-node simultaneous daemon control** — NodesCard switches workspaces;
  it does not run several local daemons at once.

## Consensus / compatibility

Zero consensus change. No new Tauri commands anticipated (all signing verbs
exist); frontend-only plus copy. `localStorage` keys added:
`ducktape.pendingDisplayName`, `ducktape.accountLinkPending`.

## File plan (mono-file mandate: split by responsibility, ~600-line cap)

```
app/src/console/views/onboarding/
  OnboardingChrome.tsx      stepper shell shared by all three steps
  IdentityGate.tsx          reworked step-1 host (state machine kept)
  LinkDeviceFlow.tsx        B-side wizard (used by gate + Account console)
  OnboardingGate.tsx        step-2 under the chrome
  JoinProgress.tsx          step-3, renumbered, + bind row
app/src/console/views/account/
  AccountView.tsx           composition root
  ProfileCard.tsx  CustodyCard.tsx  DeviceKeysCard.tsx  NodesCard.tsx
  link-device.ts            challenge/response blob codec + tests
app/src/console/views/status/ThisNodeCard.tsx
app/src/console/views/settings/   thinned (IdentityCard/DevicesSection leave)
app/src/console/store/auto-bind.ts   link-pending guard
```

## Testing

- Unit (vitest): link blob codec round-trip + tamper cases; auto-bind
  link-pending guard (skip → bind once membership appears); stepper stage
  mapping; Account console rendering across custody states and
  connected/disconnected; unbind + remove-key confirm flows with mocked
  clients; pending-display-name application.
- Existing `IdentityGate.test.tsx` / `onboarding.test.tsx` / `NodeFailed`
  tests updated for the chrome + copy.
- Live QA (headless, tauri-debug/fleet): fresh onboarding create-path
  end-to-end; Account console over a real workspace (keys, nodes, custody);
  Settings/Node rearrangement; two-fleet-tile link ceremony if fleet allows,
  else codec-level + single-side wizard checks.
