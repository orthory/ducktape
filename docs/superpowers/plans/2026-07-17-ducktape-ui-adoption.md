# ducktape-ui Adoption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `app/src-iced` (branch `feat/iced-app`) consumes
[byeongsu-hong/ducktape-ui](https://github.com/byeongsu-hong/ducktape-ui) as its
component layer: toolkit source vendored under `app/src-iced/src/ui` via the
`ducktape-ui` CLI, and every content-surface control (buttons, inputs, fields,
checkboxes, progress, empty states, separators, cards, badges) built from it.

**Architecture:** ducktape-ui is shadcn-style — the CLI copies editable component
source into the app; there is no runtime dependency. The app keeps two token
layers ON PURPOSE: `crate::theme` (rich chrome palette — titlebar, rails, tray,
hover rows) stays the styling source for app chrome; `crate::ui::theme` (semantic
shadcn roles) styles content-surface controls. The bridge is
`theme::ui(mode, accent) -> ui::theme::Theme`. Both palettes share the same
design anchors (ink `0x2c2b27`, paper/dark `0x1b1a17`, radius 7/9/11, type scale
10.5–18) because ducktape-ui was extracted from this app's theme.

**Tech Stack:** iced `=0.14.0` (pinned, `advanced` feature already on),
vendored ducktape-ui at commit `4a8d208e3008a117ce166b1fd978de1833b55177`.

## Global Constraints

- Vendored files under `app/src-iced/src/ui/` are edited only via
  `ducktape-ui add --overwrite` or deliberate local forks — note a local fork
  with a `// local fork:` comment at the top of the file.
- `pub mod ui;` in `lib.rs` stays public: not-yet-consumed components must not
  trip dead-code lints.
- Chrome is NOT migrated: titlebar, network/module rails, tray popup, tab strip,
  mode segments, list-row/nav selection buttons keep their bespoke
  `crate::theme` styles. A "button" migrates only if it is semantically an
  action button (does a thing), not a selection/navigation row.
- No `cargo fmt --all`; format only touched files.
- Gate (per repo law): `cargo clippy -p ducktape-iced --tests --no-deps` with
  zero NEW warnings, and `cargo test -p ducktape-iced` green.
  `CARGO_INCREMENTAL=0 RUSTC_WRAPPER=""` on this box (rustc ICE/segfault traps).
- Geometry tests in `src/test/` are to-be locks: update them deliberately to the
  toolkit contract (button heights 32/36/40, input padding [8,12], label size
  12.0), never just to whatever makes a failure go away.
- `bin/cef_probe.rs` is a diagnostics probe, not app UI — out of scope.
- Wire/backend code untouched: this is a view-layer-only campaign.

## The Migration Contract

Every cluster task applies this table. `t` is obtained at the view boundary:

```rust
use crate::theme;
use crate::ui;  // vendored toolkit

let t = theme::ui(mode, accent);   // mode: theme::Mode, accent: iced::Color
// screens that receive only `mode` (no accent) use theme::ui(mode, theme::ACCENTS[0])
// only when accent is genuinely unavailable at that call site — prefer plumbing.
```

| Today (hand-rolled) | Becomes | Notes |
|---|---|---|
| `button(text(..)).style(filled/primary closure)` | `ui::button::button(label, &t).on_press(msg)` | Default variant = filled primary |
| danger/destructive button | `.variant(ui::button::ButtonVariant::Destructive)` | |
| bordered neutral button | `.variant(ButtonVariant::Outline)` | |
| panel/chip-background button | `.variant(ButtonVariant::Secondary)` | |
| transparent action button | `.variant(ButtonVariant::Ghost)` | actions only — NOT selection rows |
| inline text-link button | `.variant(ButtonVariant::Link)` | |
| icon-only action button | `ui::button::Button::new(icon_el, &t).size(ButtonSize::Icon)` | |
| dense-surface buttons | add `.size(ButtonSize::Small)` (32px) | primary CTAs keep Default (36px) |
| disabled = omitted `on_press` | keep `.on_press(msg)` + `.disabled(cond)` | builder handles `on_press_maybe` |
| `text_input(ph, val).style(..)` | `ui::input::input(ph, val, &t)` | returns `TextInput` — chain `.on_input/.on_submit/.id/.secure/.width` as before |
| invalid input state | `ui::input::input_with_variant(ph, val, InputVariant::Invalid, &t)` | |
| label + control + error column | `ui::field::field(label, control, Some(FieldHint::Error(e)), &t)` | `FieldHint::Description` for help text |
| `checkbox(..)` | `ui::checkbox::checkbox(label, checked, &t).on_toggle(msg)` | |
| `progress_bar(..)` | `ui::progress::progress(percent, ProgressVariant::Default, &t)` | percent is 0.0–100.0 |
| hand-rolled "no items yet" column | `ui::empty_state::empty_state(leading, title, desc, &t)` | |
| hand-rolled hairline/divider on content surface | `ui::separator::horizontal(&t)` / `vertical(&t)` | chrome dividers stay bespoke |
| card-ish container (paper + border + radius) on content surface | `ui::card::card(content, &t)` or `ui::surface::surface(content, SurfaceVariant::Card, &t)` | chrome panels stay bespoke |
| status chip/pill | `ui::badge` (read vendored `badge.rs` for the builder) | |
| segmented mode selector on content surface | `ui::segmented_control::segmented_control(items, selected, on_select, &t)` | shell chrome `mode_segments` stays bespoke |
| `text_editor(..)` styling | `ui::textarea::textarea(..)` (read vendored `textarea.rs`) | |
| `pick_list(..)` | **KEEP** — `native_select` migration is Wave 2 | stateful open/close plumbing, defer |
| tooltips/dialogs/menus/toasts/tabs/scrollbars | **KEEP** — overlay components are Wave 2 | |

Standing instruction: the vendored component source is IN THE TREE
(`app/src-iced/src/ui/*.rs`) — read the exact constructor before using it;
never guess a signature.

---

### Task 1: Vendor toolkit + theme bridge  ✅ (done inline, this session)

**Files:**
- Create: `app/src-iced/ducktape-ui.json`, `app/src-iced/src/ui/*` (22 components + `mod.rs`)
- Modify: `app/src-iced/src/lib.rs` (`pub mod ui;`), `app/src-iced/src/theme.rs` (bridge)

**Interfaces:**
- Produces: `theme::ui(mode: Mode, accent: Color) -> crate::ui::theme::Theme`
  (const fn; `LIGHT`/`DARK` + `.with_accent(accent)`), plus the whole
  `crate::ui::*` component namespace.

- [x] Build CLI from the pinned clone; `ducktape-ui init`;
  `ducktape-ui add button input textarea checkbox field card badge alert progress
  empty separator segmented-control label kbd spinner skeleton avatar native-select surface`
- [x] `pub mod ui;` in `lib.rs`; bridge fn in `theme.rs`
- [x] `cargo check -p ducktape-iced --tests`: only new warning is unused
  `theme::ui` (consumed by Task 2)
- [ ] Commit: `feat(iced): vendor ducktape-ui component layer + theme bridge`

### Task 2: Pilot — screens/user.rs + screens/home.rs

**Files:**
- Modify: `app/src-iced/src/screens/user.rs` (3 buttons, 2 inputs),
  `app/src-iced/src/screens/home.rs` (1 button)
- Test: the screens' existing tests + `src/test/` locks that touch them

**Interfaces:**
- Consumes: Task 1's bridge and components.
- Produces: the proven transform pattern each cluster copies; confirms default
  font (Geist) flows into toolkit text, and geometry-test update mechanics.

- [ ] Migrate per the contract table
- [ ] `cargo clippy -p ducktape-iced --tests --no-deps` — zero new warnings
- [ ] `cargo test -p ducktape-iced user:: home::` (plus affected `test::` locks) — green
- [ ] Commit: `feat(iced): migrate user/home screens to ducktape-ui (pilot)`

### Tasks 3–13: Cluster migration (parallel agents, edit-only)

Eleven disjoint file clusters; each agent applies the contract table to its
files INCLUDING the matching `src/test/<surface>.rs` locks. Agents only edit —
no git, no cargo (shared tree; the orchestrator gates and commits per cluster).

| # | Cluster files (all under `app/src-iced/src/`) |
|---|---|
| C1 | `screens/forge/view.rs`, `screens/forge.rs`, `test/forge.rs` |
| C2 | `screens/agents/view.rs`, `screens/agents.rs`, `screens/agents/run_log.rs`, `test/agents.rs` |
| C3 | `screens/members.rs`, `screens/governance.rs`, `test/members.rs`, `test/governance.rs` |
| C4 | `screens/pages/view.rs`, `screens/pages.rs`, `test/pages.rs` |
| C5 | `screens/workspace.rs`, `test/workspace.rs` |
| C6 | `screens/settings.rs`, `search.rs`, `test/settings.rs` |
| C7 | `huddle_ui.rs` |
| C8 | `screens/operator.rs`, `screens/operator/{gateway,metrics,modules,node,sandbox}.rs`, `test/operator.rs` |
| C9 | `screens/file_browser.rs`, `screens/explorer.rs`, `test/files.rs`, `test/explorer.rs` |
| C10 | `screens/chat.rs`, `screens/chat_composer.rs`, `screens/terminal.rs`, `test/chat.rs`, `test/terminal.rs` |
| C11 | `onboarding.rs`, `browser_chrome.rs`, `network_content.rs`, `notifications.rs`, `test/onboarding.rs`, `test/browser.rs` |

Per cluster (orchestrator): `cargo clippy -p ducktape-iced --tests --no-deps`
zero new warnings → `cargo test -p ducktape-iced <surface>::` green → commit
`feat(iced): migrate <cluster> to ducktape-ui`.

### Task 14: Shell selective migration

**Files:**
- Modify: `app/src-iced/src/shell/view.rs` (dialog action buttons,
  permission-prompt buttons, notification action buttons ONLY)

Chrome functions stay bespoke: `titlebar`, `network_rail`, `module_rail`,
`tray_*`, `mode_segments`, `segment*`, `tab_style`, `rail_circle`,
`icon_button`, `chrome_icon_button`, `transparent_button` (as chrome hover).

- [ ] Migrate action buttons per contract; gates; commit
  `feat(iced): shell dialog actions on ducktape-ui`

### Task 15: Sweep + union gates + delivery

- [ ] Delete vendored components still unused after all clusters (check
  `cargo clippy` dead-code with `mod ui` temporarily private, or grep usage);
  keep `ducktape-ui.json` so `add` restores them on demand
- [ ] `cargo clippy -p ducktape-iced --tests --no-deps` — zero new warnings
- [ ] `CARGO_INCREMENTAL=0 cargo test -p ducktape-iced` — full suite green
  (includes `shell/qa.rs` recipe glob = qa/recipes Simulator lane)
- [ ] `make ui-qa` if the branch Makefile has it (fleet lane optional on this box)
- [ ] Commit plan doc; push; PR against `feat/iced-app`; clean-context review;
  merge only at high confidence (repo law)
