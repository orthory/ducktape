# iced_test bridge — Preset boot lane + sem Selector interop

**Status:** approved direction (user pruned the option set: `.ice`
record/replay REJECTED — do not rebuild; Emulator-as-bridge-backend deferred).

## Goal

Connect the upstream `iced_test` 0.14 stack (Simulator / Emulator / Presets)
with the iced-agent stack so the two QA lanes share one addressing scheme and
one state-authoring effort:

- **iced_test ≈ simnode lane** — in-process, headless, no side-effect
  realism, plain `cargo test`, no Xvfb.
- **iced-agent ≈ fleet lane** — the live app, real windows, real node/CEF,
  driven externally over the bridge.

Two deliverables, one per repo:

## W2 — sem Selector interop (`byeongsu-hong/iced-agent-browser`)

`plugin/src/selector.rs`: implement `iced_selector::Selector` over the `sem()`
semantic layer, so Rust tests address widgets exactly like the agent does
(role + name), not by brittle visible text.

- Verified mechanics: the `sem` wrapper's `operate` emits
  `operation.custom(None, bounds, &mut SemProbe)`; `iced_selector`'s `Find`
  operation forwards `custom()` as `Candidate::Custom { state: &dyn Any, bounds,
  visible_bounds, .. }` — the selector downcasts `state` to `SemProbe`, matches
  `Enter { role, name, .. }` (ignores `Exit`), and yields a `Target`.
- API: `by::role(Role::Button, "Save")` (exact name, case-insensitive) and
  `by::any(Role::Button)` (first of role). Output = `iced_selector::Target`
  so `Simulator::click`/`find` and `Emulator` instructions accept it directly.
- `iced_test` becomes a dev-dependency of the plugin with a unit test: build a
  small sem-tagged view, `simulator.click(by::role(Role::Button, "Go"))`,
  assert the message fired. This is the only test that exercises `sem` through
  a real widget tree (today's tests drive the collector with hand-fed probes).
- Non-goal: no changes to the fork; the selector is plugin-side only.

## W1 — Presets + iced_test smoke lane (ducktape, `feat/iced-app`)

- `shell::run()` gains `.presets([...])` on the `iced::daemon` builder
  (daemon supports presets natively in 0.14).
- Preset v1: **`ui-demo`** — a backend-less `Shell` (backend `None`, canned
  `Resource::Ready` data for the chrome-visible screens) that boots straight
  past onboarding into the main chrome. Purpose: UI-logic and navigation tests
  with zero node/network. Honest scope: a "real onboarded backend" preset is
  NOT buildable UI-side (identity/consensus state lives in the node) — out of
  scope until the backend grows a fixture seam.
- Live-boot seam: `DUCKTAPE_PRESET=<name>` (dev builds only, same
  `all(feature = "agent", debug_assertions)` gate) makes `Shell::boot` call the
  named preset's boot instead of the default — the agent e2e can then drive
  the chrome without walking onboarding first.
- Smoke lane: `app/src-iced/tests/ui_smoke.rs` with `iced_test` as a
  dev-dependency — an `Emulator` boots the `ui-demo` preset, clicks a nav
  button (via W2's `by::role` once published, or text selector meanwhile), and
  expects a text marker on the target screen. Proves the lane runs under plain
  `cargo test` in CI.

## Risks / notes

- `iced_test`/`iced_selector` pull no `iced_winit` — no interaction with the
  fork patch. All pinned `=0.14.0`.
- Upstream marks the instruction DSL experimental; we consume only
  Simulator/Emulator/Preset APIs, not `.ice`.
- The `ui-demo` preset is dev-only fixture data; it must never leak into
  release binaries (same cfg gate as the agent stack) nor contain key
  material.

## Verification

- W2: `cargo test -p iced-agent-plugin` includes the Simulator round-trip;
  clippy clean.
- W1: `cargo test -p ducktape-iced` includes the Emulator smoke; live check:
  `DUCKTAPE_PRESET=ui-demo` boot under Xvfb → `iced-agent tree` shows main
  chrome without onboarding nodes; release build ignores the env var.
