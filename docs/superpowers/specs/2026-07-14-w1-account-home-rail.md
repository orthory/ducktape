# W1 Spec — Account home + network rail + store network-scoping

Date: 2026-07-14. Status: **implementing**. Parent epic:
`docs/superpowers/specs/2026-07-14-account-workspace-separation-epic.md` (§W1,
binding). Normative model: `docs/adr/2026-07-14-account-node-access-model.mdx`
(A1–A6). This is the W1 work-item spec; siblings W2/W5/W6 run in parallel.

## What ships

A Discord-style shell frame — **network rail | module nav | content** — on top
of today's single-active node mechanics, plus the store reshape that makes each
joined network a first-class *seat* rather than "the active workspace".

1. **Network rail** (far-left, `NetworkRail.tsx`): a persistent vertical icon
   rail. "me" (account home) chip on top; one chip per joined network below in
   **join order** (registry order, no drag-reorder); a badged **remote** seat
   when a client connection is live; a `+` CTA at the bottom that opens the
   connect panel. Chips are the network initial on a color **deterministically
   hashed from the chain id** (no avatar feature). The active seat highlights.

2. **Account home is the landing surface**. First run (identity set up, zero
   networks) lands on the account home — profile card, custody card, joined
   network list, join/create CTAs — **not** a full-screen onboarding gate. The
   full-screen "Add workspace" gate is deleted; its create/join/remote forms
   are relocated into a dismissible **connect panel** modal opened from the rail
   `+` and the home CTAs. `HomeView` is the account home (already exists from
   #587; refined here).

3. **Store network-scoping** (`store/networks.ts`): a `NetworkSeat` model +
   `networksFrom(state)` selector deriving the rail seats from the existing
   workspace registry plus the live remote connection; `activeSeat(state)`;
   deterministic `seatColor`/`seatInitial`. This is the per-network slice model
   at the granularity single-active needs: **each network is a seat; the active
   seat owns the flat node projection**. W4 promotes this to N live projections
   by swapping the connection layer — no projection field is sharded now
   (single-active premise; the ledger's "reuse the registry mechanics").

4. **`nodeControlAvailable` is a per-network evaluation** — it now evaluates the
   **active seat** (`kind === "local" && managed`), keeping the interim
   `managed` form and the exact `{ workspace, managed }` input shape existing
   callers pass. W2 replaces the predicate body; the seam is one function.

5. **Account-scoped settings = custody + devices only**. These already live on
   the account home (`CustodyCard`, `DevicesCard`), not in network settings.
   W1 keeps them there and leaves theme/general app-level in `SettingsView`
   (`PreferencesSection`). Network settings (`WorkspaceSection`) carries no
   custody/devices — confirmed, unchanged except terminology.

6. **Terminology**: user-facing "workspace" → "network" in every surface W1
   touches (home, rail, connect panel, network settings). Internal identifiers
   (`state.workspace`, `workspace_*` Tauri commands, `Workspace` type, the
   `workspaces.rs` registry) are **not** churned — epic mandate.

7. **Remote/client mode keeps working** (#587): the client connection gets a
   badged rail seat, no control chrome (A6). No change to the connect/dial path.

## Decisions (micro, made here)

- **Chip color**: `hsl(hash(chainId) mod 360, 55%, 45%)`, white glyph —
  theme-invariant on purpose (a colored identity chip has nothing to invert
  against; same reasoning as the video scrim token).
- **`needsOnboarding` is repurposed** as "connect panel open" (a modal), not a
  full-screen route. Landing surfaces that used to set it (boot with no active
  network, forgetting/deleting the last network, retry with no target) now set
  `atHome: true` → the account home is the front door. The panel is always
  dismissible because there is always a base surface behind it.
- **`OnboardingGate.tsx` → `ConnectPanel.tsx`**: same create/join/remote form +
  existing-network quick list + delete affordance, reshaped into a modal. The
  component is renamed (internal identifier, not churn-restricted); the
  full-screen *screen* is gone, satisfying "delete the Add workspace screen".
- **Web build**: no local registry → no rail; unchanged single-node dial.
- **Section seams for W5/W6**: the account home keeps small, clearly-labelled
  card sections (`ProfileCard`, custody, network list, devices) that W5's device
  list and W6's profile fields slot into during the epic merge. Nothing
  speculative is added.

## Non-goals (W1)

- No concurrent live nodes/connections (W3/W4, deferred) — the seat model is the
  door, not the build.
- No owner control-plane change (W2 owns `nodeControlAvailable`'s new body).
- No migration machinery — existing workspaces appear as network seats as-is.
- No `duck://` change — refs stay network-implicit.

## Gates

- App: the touched/added sim suites + `bun run typecheck`.
- No Rust touched (TS-only work item).
- Live desktop/fleet QA skipped — siblings would collide on the epic branch; it
  runs on the epic branch after the item PRs land.
</invoke>
