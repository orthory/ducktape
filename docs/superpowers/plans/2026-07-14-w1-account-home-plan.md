# W1 Plan — Account home + network rail + store network-scoping

Spec: `docs/superpowers/specs/2026-07-14-w1-account-home-rail.md`. Commit in
reviewable chunks; run `bun run typecheck` + touched sim suites after each.

## Steps

1. **Store seat model** — `app/src/console/store/networks.ts`: `NetworkSeat`,
   `networksFrom(state)`, `activeSeat(state)`, `seatColor(chainId)`,
   `seatInitial(name)`, `nodeControlForSeat(seat)`. Refactor
   `nodeControlAvailable` (in `state.ts`) to evaluate the active seat, same
   input shape. Pure — add `app/src/test/sim/networks.test.ts`.

2. **Network rail** — `app/src/console/layout/NetworkRail.tsx`: me chip →
   `goHome`; seat chips (join order) → `selectWorkspace`/dismiss home; remote
   badge; `+` → `newWorkspace`. Reuse `initialsOf`. `app/src/test/sim/rail.test.tsx`.

3. **Connect panel** — rename `OnboardingGate.tsx` → `ConnectPanel.tsx`, reshape
   to a dismissible modal, terminology → "network". Delete the full-screen gate.

4. **Shell frame** — `DucktapeConsole.tsx`: `ConsoleBody` → full-screen
   `JoinProgress`/`NodeFailed` first, else `AppFrame` = rail + base
   (`hasNodeContext ? ConsoleShell : HomeView`) + connect-panel modal. Move the
   account avatar out of `Sidebar` (now the me chip lives on the rail);
   `Sidebar` becomes pure module nav + node toggle + settings + theme.

5. **Boot/actions** — `DucktapeProvider` boot: no-active-workspace → `atHome`
   (not the gate). `actions.ts`: forget/delete-last + retry-no-target →
   `atHome`; `newWorkspace` opens the modal.

6. **Terminology sweep** — home/rail/network-settings/connect-panel strings
   workspace → network. Update `WorkspacesTable` labels; keep the component name.

7. **Tests** — update `home-routing.test.tsx` (first run → home),
   `onboarding.test.tsx` + `workspace-management.test.tsx` (import ConnectPanel,
   new strings). Keep `nav.test.ts` green (predicate shape unchanged).

## Risks / seams

- `state.workspace` blast radius is large → **not** renamed; seat model layers
  on top. Narrow seam for W2 on `nodeControlAvailable`.
- Huddle-dock offset in `ConsoleShell` is measured from the shell box (right of
  the rail) → unchanged.
