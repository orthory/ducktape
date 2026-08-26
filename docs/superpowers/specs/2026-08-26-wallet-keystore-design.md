# Wallet keystore — named user keys, one active, app + CLI shared

2026-08-26. Approved direction: approach A (keystore directory + active
pointer), local signing-key switching only, one global active key,
`cast wallet`-style CLI porcelain, wallet-first app entry.

## Why

The app and the CLI answered "where is the user key" differently: the app
reads `~/.ducktape/user.key` (the 2026-07-07 identity-onboarding spec's
canonical cold identity — the key demo-seed mints and demo-gateway binds),
while the CLI ladder resolved `<workspace>/user.key` and `account-init`
silently minted a fresh competing identity there when it found nothing. The
observed failure chain on a dev box: app shell says `cred add codex` →
`cred add` finds no `<workspace>/user.key` → "run account-init" →
account-init mints a stranger key → BindNode → "node is already bound to
another account". Two canonical paths is the defect; this design replaces
both with ONE keystore that also gives users what they actually asked for:
wallet-style key management — several named identities, list them, switch
the active one, enter a workspace as that identity.

## Non-goals

- No on-chain work. Switching wallets never unbinds/rebinds a node. If the
  selected wallet is not the dialed node's bound account, owner-gated verbs
  fail exactly as they do today for a stranger key.
- No per-workspace key mapping. ONE global active wallet (MetaMask model);
  the workspace axis stays what it is.
- No versioning/compat of the key FILE format: the v2 encrypted format from
  the 2026-07-07 spec is unchanged. A wallet file IS a v2 `user.key`, named.

## Keystore layout

```
~/.ducktape/keys/            # or $DUCKTAPE_HOME/keys when the override is set
  <name>.key                 # one v2-encrypted user key per wallet
  active                     # one line: the active wallet's <name>
```

- Wallet names: `[a-z0-9][a-z0-9._-]{0,40}` — filesystem-safe, lowercase.
  The file name IS the wallet name plus `.key`; there is no index file to
  drift from the directory.
- `active` is written atomically (tmp + rename) and contains a name, not a
  path. A dangling `active` (file deleted by hand) is a loud error naming
  `ducktape wallet use`.
- The keystore root honors `DUCKTAPE_HOME` exactly like `workspaces_root()`
  in `bin/node/src/config/mod.rs` (tests, portable setups, huddle lanes).

### One-shot adoption of the legacy location

On first keystore resolution (any wallet verb, or the resolver below): if
`keys/` does not exist and `~/.ducktape/user.key` does, create `keys/`,
rename `user.key` → `keys/default.key`, and write `active` = `default`.
A rename moves a symlinked `user.key` as a symlink, which keeps working.
This is a move, not a dual-read: after adoption `~/.ducktape/user.key` no
longer exists and no code path reads it again. (Zero live networks; the
no-legacy rule applies — the old path is replaced, not tolerated.)

## CLI porcelain — `ducktape wallet`

Modeled on foundry's `cast wallet`, layered over the existing `user key`
plumbing verbs (which keep operating on explicit `--key`/`--out` paths —
plumbing vs porcelain, not a dual path). Secrets cross via stdin only,
same as every `user key` verb.

- `wallet new <name>` — stdin: password. Mints `keys/<name>.key` (refuses
  an existing name), prints the pubkey and the 24 recovery words. If the
  keystore was empty, sets `active` = name.
- `wallet import <name>` — stdin: mnemonic line, then password line.
  Restores into `keys/<name>.key`; same empty-keystore active rule.
- `wallet list [--json]` — enumerates `keys/*.key`: name, pubkey (parsed
  from the v2 header, no password), path, per-file state (the
  `user key status` vocabulary: encrypted/plaintext/unlocatable), and the
  active flag. The JSON form is the app's data source for its wallet list.
- `wallet use <name>` — sets `active`. Errors on an unknown name.

No `wallet remove` (delete the file), no `wallet reveal`
(`user key reveal --key keys/<name>.key` already does it).

## The ONE key resolver

`bin/node/src/config` grows `active_user_key() -> Result<PathBuf>`:

1. `DUCKTAPE_USER_KEY` env, when set — the rig/scripted override, and the
   escape hatch that bypasses wallet selection everywhere.
2. The keystore's active wallet: `keys/<active>.key` (running the adoption
   move first). Empty keystore → error naming `ducktape wallet new <name>`;
   no/dangling active → error naming `ducktape wallet use <name>`.

Callers keep their explicit `--key` flag above the resolver.

Replaced call sites (the `<workspace>/user.key` rung is DELETED, not kept
as a fallback):

- `cred_cli.rs` `VerbCtx::key_path()` — was `workspace.join("user.key")`.
- `userkey_cli.rs` `cmd_user_account_init` — was
  `dir.join("user.key")` + silent mint. Now: resolve the active wallet; on
  an EMPTY keystore only, mint a wallet named after `--name` (sanitized to
  the wallet-name charset), set it active, and print the mnemonic — the
  one-shot operator promise, now landing in the keystore instead of minting
  a stranger beside it. `account_init_mints_the_user_key_when_the_workspace_has_none`
  is rewritten to this contract.

The app's `user_key_path()` (`app/src/backend/rpc.rs:524`) keeps its
`DUCKTAPE_USER_KEY` / `DUCKTAPE_HOME` / `HOME` ladder but its default leg
becomes the keystore's active wallet, resolved via `ducktape wallet list
--json` (the app already shells the CLI for every key op, so keystore
logic lives once, in bin/node).

## App — wallet-first entry

The launch flow today: `on mount` opens the onboarding window
(`app/src/ui/handlers/lifecycle.ice:5-6`), `hub_state()`
(`app/src/backend/hub.rs:229-255`) reads `user_key_state()` off the single
`user_key_path()`, and `hub_entry_step()` (`hub.rs:152-158`) lands on
`HubStep.create` (absent) or `HubStep.unlock`; `key_unlocked` then sets
`hub_step = HubStep.networks` (`handlers/onboarding.ice:59`) — the
workspace list — and `open_network_submit` opens the console window.

The wallet change, at that seam:

- `HubStep` (`app/src/ui/state/types.ice:50-59`) gains `wallets` and
  loses `unlock`: the wallet LIST is the unlock surface. The exhaustive
  `match step` in `components/onboarding.ice:43-128` swaps the arm; the
  standalone `UnlockScreen` dies with it (its `GateNote` refusal plate
  and password-input pattern move into the selected wallet row).
- `hub_state()` shells `wallet list --json` (replacing the bare
  `--version` Gatekeeper prewarm at `hub.rs:238-242` — one subprocess
  does both jobs, and it is also what runs the keystore adoption move on
  a first post-upgrade boot). `HubState` carries the wallet rows
  (name, pubkey, per-file state, active flag) beside `networks`.
  `hub_entry_step()`: empty keystore → `create`, else → `wallets`. When
  `DUCKTAPE_USER_KEY` is set the keystore is bypassed: the list shows ONE
  synthetic row for the env-named key (preselected; unlocking it skips
  `wallet use`), so huddle lanes and rigs get the same single screen with
  zero keystore reads — their recipes retarget the row's password input
  once.
- `WalletsScreen` + `WalletRow` follow `NetworksScreen`/`NetworkRow`
  (`components/onboarding.ice:562-876`) exactly: rows with a selected
  arm that reveals inline content — here the `secure=true` password
  input (the `UnlockScreen:199-213` pattern). The active wallet is
  preselected. Submit → `unlock_user_key` against THAT row's path; on
  success the backend runs `wallet use <name>`, updates the cached
  active-key path, seeds `LOCAL_USER_KEY` as today, and the handler sets
  `hub_step = HubStep.networks`. "Continue read-only" (`login_skip`)
  and "Restore from recovery phrase" (`go_restore`) move to the list's
  footer.
- `user_key_path()` (`app/src/backend/rpc.rs:524-535`) keeps
  `DUCKTAPE_USER_KEY` first; its default leg becomes the cached active
  wallet path set at boot / wallet switch — the signer child
  (`rpc.rs:197-232`) and `user key status` fallback read follow it
  automatically. The signer is already keyed on password identity, so a
  wallet switch retires it like a re-login does.
- Create/Restore screens write through `wallet new <name>` /
  `wallet import <name>` and gain a name input (prefilled `default` /
  `restored`); their success paths (`key_restored`, `reveal_confirm`)
  continue to `HubStep.networks` unchanged — the minted/imported wallet
  is active and its password is in session state.
- Mid-session switch: `switch_network` already reopens the launch
  window at the networks list (`handlers/onboarding.ice:460-484`); the
  networks screen shows the active wallet's name with a "switch wallet"
  affordance → `hub_step = HubStep.wallets`. Unlocking a different row
  re-runs the same success path into `networks`, and the console re-entry
  (`console_opened`'s full reset) re-derives every plane from the new
  identity.

## Ops surfaces

- `ops/demo-seed.sh`: replace the `$DUCK/user.key` provisioning with
  `wallet list --json`-guarded `wallet new demo` (password
  `$DEMO_PASSWORD`). The seed then signs demo-gateway's bind with
  `keys/demo.key` explicitly. The "existing key, we don't hold its
  password, routes skipped" branch DIES: the demo wallet is always the
  seed's own, so the bind always signs. The seed does NOT touch `active`
  — the app's wallet list is where the user picks the demo identity.
- `ops/demo-gateway.mjs`: unchanged (takes the key path as an argument).
- `ops/huddle-lane.sh`: keeps minting per-side keys at explicit paths but
  exports `DUCKTAPE_USER_KEY=$LANE/home-<side>/user.key` so the app
  bypasses wallet selection. (Its `DUCKTAPE_HOME` stays for workspaces.)
- The closing demo-seed banner and `bin/node/src/cli.rs:254`'s
  "account=none" hint change wording: `ducktape user account-init` still
  works and is still the operator one-shot; the key it uses is the active
  wallet.

## Testing

- bin/node unit tests (tempdir `DUCKTAPE_HOME`): wallet new/import/list/
  use round-trip; name validation; empty-keystore auto-active; adoption
  move (user.key → keys/default.key + active written, symlink preserved);
  resolver precedence (`DUCKTAPE_USER_KEY` > active; empty-keystore and
  dangling-active errors); account-init's empty-keystore mint. Env-var
  probes of process-global state run in ONE test, sequentially.
- cred_cli: key_path() resolves the active wallet; `--key` still wins.
- app: ice tests drive the wallet list screen (list renders rows from a
  seeded keystore, selection + unlock + `use` round-trip), plus the
  `DUCKTAPE_USER_KEY` bypass. Waits are event-based, never timed.

## Rollout

One branch, PR against dev: keystore module + wallet verbs + resolver +
call-site replacement + app entry flow + ops scripts + docs, delivered
together so no window exists where app and CLI disagree again.
