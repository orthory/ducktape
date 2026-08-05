# Onboarding Window: login → network picker → console

Status: IMPLEMENTED — this document matches the shipped shape.

## Goal

Discord/Steam-style launch flow. The app boots into a small dedicated
onboarding window: sign in (password now, passkey-shaped seat when a custody
plane exists), then a list of known networks — local workspaces on disk plus
saved remotes — and picking one opens the full console window and closes the
onboarding window. This REPLACES the current in-window pre-console phase
column (`OnboardingPhase` in `view.ice`); no dual-path, per repo doctrine.

## Current reality (recon)

- `app Ducktape` is a single-window ice app (`app/src/ui/app.ice`); the whole
  shell is generated from ice. `view.ice` branches on `phase`: `"console"`
  mounts the shell, anything else mounts the pre-workspace column.
- Onboarding today: welcome → join(invite blob) → provisioning(`ducktape node
  join` + `/v1/status` poll) → live → console. The app is a strict CLIENT —
  there is no create-network route (deliberate, PR #891).
- No login exists. No network list exists: one workspace, `rpc`/`connected_rpc`
  strings, `forget_workspace_submit` in settings.
- ui-lang fully supports the window story: `daemon` roots, named `window`
  templates, `task window open/close` with `window-id` in state, per-window
  view routing via the `window` binding, `window closed` subscriptions
  (ducktape-ui SPEC.md ~1425, ~4440).

## Design

### Windows

Convert the root to `daemon Ducktape` with two named window templates:

- `window onboarding` — small fixed column (~420×640), centered,
  non-resizable, macOS hidden-title settings.
- `window console` — today's 1280×800 / min 820×540 window with the existing
  macOS platform block.

State holds `onboarding_win:window-id?` and `console_win:window-id?`. The root
view branches on which window is being rendered. `on mount` always opens the
onboarding window; the console window opens only on a network pick. Closing
the last window exits (`window closed` subscription → `exit` when both ids are
gone).

Known trade (framework ceiling, verified in ducktape-ui codegen): a `daemon`
root uses `Bridge::without_native_adapter()` — the deterministic
accessibility tree and the ice test harness keep working, but the NATIVE
screen-reader adapter is application-only until iced exposes window-scoped
a11y operations. Recorded here and in the PR body; reversible upstream.

### Onboarding window steps (one `hub_step` discriminant)

1. `login` — keyed on `user key status`:
   - `absent` → create identity: password (+confirm, min 8 = CLI's
     MIN_PASSWORD_LEN) → `ducktape user key init` (password over stdin, the
     CLI's only secret channel) → show the 24 recovery words once with an
     explicit "I saved them" gate. A "Restore from recovery phrase" sub-step
     drives `ducktape user key restore` (24 words + new password).
   - `encrypted` → unlock: one password field, verified by `ducktape user key
     unlock` (decrypt probe). On success the password lands in the SAME
     `password` state field every signing extern already threads — the
     signing model does not change, only where the password is captured.
     A quiet "continue read-only" escape stays available (reads never need
     the password).
   - This fixes the recon gaps directly: today first run is a dead end (the
     app cannot mint `user.key`) and the password is a bare Settings field
     with no ceremony.
   - Passkey: consensus-side WebauthnP256 verification exists, client-side
     custody does not (Touch ID plane deleted; passkey-session-keys spec is
     an unapproved DRAFT). No dead buttons — password ships now, the passkey
     seat is the named follow-up.
2. `networks` — the picker. Rows: every known network (local workspaces on
   disk + saved remotes), name + endpoint/chain id + last-used, last-used
   preselected, Enter/click opens. Row actions: open, forget. Footer: "Join a
   network" → the existing invite-blob join + provisioning flow, folded into
   this window as an `add` sub-step.
3. Pick → open console window against that network's rpc, close onboarding
   window, run the existing `connect(rpc)` boot.

### Cache teardown this forces

`registered_endpoint()`, `active_workspace_name()` and `local_user_key()` are
`OnceLock`/`OnceCell` process-lifetime caches. A picker that switches
networks in-process and a login that mints a key in-process both invalidate
them — they become plain per-call reads (they are one small file/CLI read
each; the caches bought nothing a boot needs).

`Leave workspace` in the console reverses the handoff: open onboarding window
at `networks`, close console window.

### Backend (app/src/backend/hub.rs — new module, mono-file cap respected)

- `hub_state()` — key state (absent/encrypted/plaintext/unlocatable, the
  Settings probe reused) + the network list in one boot read.
- `list_networks()` — enumerate `~/.ducktape/workspaces/*/` (a dir with
  `node.toml`); identity/`-n` selector = `chain_id` read from `network.toml`
  via the existing hand parser, dir name as fallback; display name =
  `chain_id` split on `#`; endpoint from `node.toml http_listen`; merged
  with `saved_remotes` from `app-prefs.json` (endpoint + name persisted when
  the console connects to an endpoint owning no local dir); minus
  `forgotten_workspaces`; sorted by a per-network `last_used` stamp in prefs.
- `probe_network(endpoint)` — one bounded `/v1/status` ping per row; a dead
  row honestly shows `not running · ducktape node run -n <id>` (the
  provisioning step-4 precedent) instead of a connect that fails later.
- `create_user_key(password)` / `restore_user_key(words, password)` /
  `unlock_user_key(password)` — `ducktape user key init|restore|unlock` over
  piped stdin (non-tty = plain newline-delimited fields, verified in
  `userkey_cli.rs`); create returns the 24 words for the one-time reveal.
- `forget_network(id)` / `touch_network(id)` (last-used stamp).
- Cache teardown from the section above rides along.

Fixed by construction: the boot split-brain where phase reads the workspaces
dir but the endpoint reads demo-only `registry.json` — a picked row carries
its own endpoint, and the registry ladder survives only as the initial
preselection hint.

## Out of scope

- Real FIDO2/webauthn passkeys (no platform custody plane today; Touch ID
  custody was deleted with src-tauri).
- Creating networks from the app (deliberately absent).
- Multi-console (one console window at a time).
