# Account-first UI (ADR A6): the NODE rail becomes a conditional surface

Date: 2026-07-14. Status: approved (operator picked the "reveal USER/NODE
tabs" shape and approach 1). Implements rule **A6** (and the UI half of
**A5**) of `docs/adr/2026-07-14-account-node-access-model.mdx`.

## Problem

The app presents USER and NODE as permanent peer lenses: the sidebar's
mode toggle is always visible, whoever you are and whatever you're connected
to. The ADR says the app is **account-first** — the account and its data are
the primary lens, and node control is a *conditional* surface that exists
only when the signed-in account can actually control a node (owner + private
RPC reachable). A remote client today still sees a "NODE" rail (read-only
Status/Metrics); under the model it should see no node chrome at all.

## Decision

Keep the USER/NODE toggle as the mechanism, but **reveal the NODE tab only
when node control is available**. When it isn't, there is no toggle — the
rail is just the account surfaces. Network-data surfaces (Members,
Governance, Explorer) move to the USER rail; their admin affordances stay
role-gated in-view exactly as today.

### 1. Registry re-sort (`app/src/console/modules/registry.ts`)

- `members`, `governance`, `explorer` move `section: "operator"` →
  `section: "user"` (rail order: chat, pages, files, browser, forge, agents,
  members, governance, explorer). Viewing members, proposals, votes and
  blocks is chain data any account with standing may read (ADR A1/A3);
  admitting, promoting, removing and upgrade-scheduling remain the in-view,
  role-gated controls they already are.
- The operator section keeps `status` (order 0 — it becomes the section
  default), `gateway`, `modules`, `sandbox`, `metrics`.
- `DEFAULT_OPERATOR_SCREEN` becomes `"status"` (was `"members"`).

### 2. The gate: `nodeControlAvailable` (`app/src/console/store/state.ts`)

```ts
/** ADR A5 (interim): a managed local daemon is ours to control. When the
 *  public/private RPC split (A2) lands, this grows
 *  `|| (ownerKey && privateRpcReachable)` and nothing else moves. */
export const nodeControlAvailable = (
  state: Pick<ConsoleState, "workspace" | "managed">,
): boolean => state.workspace !== null && state.managed;
```

Deliberately **no reachability term**: a stopped local node must still show
the node console — that's where Start lives. Reachability only becomes part
of the predicate for the future remote-owner case (A2/A5), where an
unreachable private RPC means the surface is absent.

### 3. Module availability replaces the client-mode filter (`registry.ts`)

`moduleAvailable(id, clientMode)` (which granted clients read-only
`status`+`metrics`) is replaced by:

```ts
moduleAvailable(id, opts: { nodeControl: boolean; clientMode: boolean })
```

- operator-section modules require `opts.nodeControl`;
- `members` and `governance` are excluded when `opts.clientMode` — the
  client-mode data plane for those projections isn't wired yet (the A3
  "client needs no node status" work); they return to the client rail then;
- everything else in the user section is always available. `explorer` stays
  on the client rail **iff** the block projection populates on a pure client
  connection — verified during implementation; if it doesn't, it joins the
  client exclusion with a note pointing at A3.

`modulesInSection` / `defaultScreenForSection` take the same opts. A remote
client therefore sees **zero node chrome**; connection health stays with the
existing ErrorStrip/retry machinery (no new pill).

### 4. Sidebar (`app/src/console/layout/Sidebar.tsx`)

- `ModeToggle` renders only when `nodeControlAvailable(state)`. `CLIENT_MODES`
  ("Read-only node") is deleted.
- Header comment updated: the NODE rail's *presence* is now authority+
  reachability-gated (A5/A6); within a rail, in-view role checks still own
  op-level authority.

### 5. State/actions normalization (`state.ts`, `actions.ts`)

- `setViewMode("operator")` is a no-op when the gate is off (belt-and-braces;
  the toggle is hidden anyway).
- `landOn` / `applyNavSnapshot` already normalize screens through
  `moduleAvailable` — they pass the new opts, so a nav-history snapshot or
  deep link into an operator screen while the gate is off redirects to the
  user default (`modulesInSection("operator", off)` is empty →
  `defaultScreenForSection` falls back to `"chat"`).
- `loadViewMode()` persistence is unchanged; a persisted `"operator"` is
  harmless because every connect path either satisfies the gate
  (`connectActive` → managed workspace) or resets to `viewMode: "user"`
  (`connectRemote` already does).

### 6. Out of scope

- No wire/backend change. The private RPC split (A2), WG exposure (A4), and
  the client member-dance skip (A3) are separate pending rows in the ADR.
- Members/Governance view splits, Home-as-landing, network-switcher chrome —
  all explicitly rejected during brainstorm in favor of this minimal shape.
- Client-mode Members/Governance enablement (waits on A3 data plumbing).

## Error handling

Gate flips are driven only by connect/leave/forget transitions, all of which
already reset `viewMode`/`screen` or route to full-window layers
(OnboardingGate/Home). The `setViewMode` guard plus `moduleAvailable`
normalization in `landOn`/`applyNavSnapshot` make a stale operator screen
unreachable even from persisted nav history.

## Testing

- New `app/src/test/sim/nav.test.tsx` (short name per repo test layout):
  gate truth table (managed workspace → true; client mode → false; no
  context → false), operator section empty when gate off, client rail
  excludes members/governance, USER rail contains explorer/members/
  governance, default operator screen is `status`.
- Fleet QA pass (UI lane): owner sees the toggle, NODE rail = 5 entries
  landing on the node console; USER rail = 9 entries; a remote client
  connection shows no toggle and no node surfaces; explorer renders block
  data on the USER rail.
