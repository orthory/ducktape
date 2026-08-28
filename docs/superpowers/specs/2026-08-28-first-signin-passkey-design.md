# First sign-in with a passkey — design

Status: approved 2026-08-28 (design agreed in chat; three decisions below).
Builds on `2026-08-27-identity-rework-design.md` (all six phases merged);
amends its "WebAuthn — registration, and QR login" section in one respect:
the QR is rendered BY THE APP, and the auth host gains a result relay.

## Problem

The launch window (`app/src/ui/handlers/onboarding.ice`) runs
`wallet (name + password) → 24-word reveal → networks → join → console`.
The account is reachable only afterwards, in the console's Settings card
(`app/src/ui/handlers/roster.ice`): `register_passkey` needs an EXISTING
account (`own_account`, `app/src/backend/node.rs`), `login_with_passkey`
needs an unlocked device key, and both open the desktop browser. A new user
meets a recovery phrase before ever seeing "create account", and WebAuthn is
two screens deep. The first sign-in must be: set a password, pick a network,
create the account with a passkey (or sign in with one) — scanning a QR the
app shows.

## Decisions (settled — do not re-ask)

1. **Placement: a launch-window step after the network pick, plus a console
   banner.** `HubStep.account` gates the console; the banner is the way back
   for a session that continued without an account.
2. **QR callback: a relay on the auth host.** `auth.ducktape.byeongsu.dev`
   gains a Worker route `/r/<id>` that holds the ceremony result until the
   app polls it. The node is never involved (a dev node listens on loopback;
   a private node is unreachable from a phone).
3. **Device key: auto-minted, no reveal, password stays.** First launch asks
   for one password; the key is minted under it, named after the host, and
   the 24 words are dropped (zeroized). Recovery is "Sign in with passkey"
   on the new device. Settled decision 3 of the rework (one keystore shared
   with the CLI) is unchanged; the password is the local unlock every signed
   write already threads.
4. **Creation order: the device key creates the account; the passkey is
   registered in the same flow.** Not a chicken-and-egg: `Create`'s origin
   key is merely the first member — every member key is equal, and the
   device key must exist anyway (daily ops are signed by it; a passkey needs
   a touch per signature). The account number is known after `Create`, which
   is what the passkey's `user.id` (= `userHandle`, the QR-login lookup) needs.
   Passkey-origin `Create` would need either a reserved number (a lost race
   strands a passkey on the phone) or a handle index in the identity module —
   rejected as cost without benefit.

## User-visible flows

```
[first launch]  Set a password for this device        (device key minted silently)
[networks]      pick / join by invite                  (unchanged)
[welcome]       no account for this device key on this network:
                  Create account: name → QR 1 (phone makes the passkey)
                                       → QR 2 (phone confirms)      → console
                  Sign in with passkey: QR (phone asserts)           → console
                  Continue without an account                        → console + banner
```

An existing account skips the welcome entirely. Under every QR: "Use this
computer's passkey instead" — the existing desktop-browser ceremony
(`browser_ceremony`, loopback listener), unchanged.

### Create account (with passkey)

1. `Create { name, scheme: Ed25519 }` signed by the device key — the existing
   `create_account`. Zero touches. The account number is now known.
2. QR 1: `op=create`, `user` = number, `name`; `cb` = relay URL. The phone's
   browser opens the auth page, `navigator.credentials.create()` runs on the
   phone's own authenticator (no hybrid transport), the page POSTs the
   result to the relay. The app polls the relay and receives the passkey's
   SEC1 pubkey.
3. The device key consents (`consented_add_key`, as today).
4. QR 2: `op=get` over the `AddKey` frame preimage (`passkey_frame_request`);
   the phone taps; the relay hands the assertion back; `submit_raw_frame`.

Closing at step 2 or 4 leaves a device-key-only account; the banner and the
Settings card offer "Add a passkey" (the same two QRs) later.

### Sign in with passkey

One QR: `login_request(chain_id, device_key, generation)` with `cb` = relay.
The phone's discoverable passkey asserts; `userHandle` names the account;
`login_consent` → `Get { number }` → `login_add_key` → `signed_write`.
Exactly the rework spec's QR login with the QR moved into the app.

### Console banner

Shown when `connected && !account_exists && !account_banner_dismissed`:
"No account on this network — Create · Sign in with passkey · ×". The
buttons reopen the launch window at `HubStep.account` for `connected_rpc`
(the `switch_network` reopen mechanics: open the launch window, close the
console behind it). One welcome screen, not two.

## Launch window — the hub machine

`enum HubStep` (`app/src/ui/state/types.ice`): `create` and `reveal` are
replaced by `password`; `account` is added. `hub_entry_step`: no wallets →
`password`; wallets → `wallets` (today's unlock list, unchanged for devices
that already hold keys). `restore` stays, reached from a small link on the
password screen (a wallet minted by `ducktape wallet new` still has words).

- `password_submit(pw)` → `create_device_key(pw)` (`hub.rs`): name =
  sanitized hostname (`sanitize_name`, suffixed on collision), mint via
  `keystore::wallet::create`, drop the words, activate, seed
  `set_local_user_key`. Lands on `networks` with `password` stored — the same
  session state `unlock_submit` leaves behind.
- `open_network_submit` / `connect_remote_submit` / `enter_console` no longer
  open the console directly: they set `rpc` and run
  `account_probe(rpc) -> account_probed`. `exists` → `task window open
  console` as today. Not found → `hub_step = account`. A read-only session
  (`password` empty) skips the probe: no key, nothing to create with.
- Welcome handlers: `welcome_create_submit(name)`, `welcome_login_submit`,
  `welcome_skip`, `welcome_desktop` (the browser path), `welcome_cancel`
  (stops the ceremony stream, back to the welcome). Success → the console
  opens through the same `console_opened` path.

## Ceremony streams (app backend)

The UI must render the QR WHILE the wait is in flight, so the QR verbs are
streams (`stream replace lane=ceremony … -> ceremony_stepped`) yielding:

```
CeremonyStep { phase: CeremonyPhase, qr: str, detail: str }
  phase: creating | show_qr | waiting | done | failed
```

- `create_account_by_qr(rpc, password, chain_id, name)` — steps 1–4 above;
  yields `show_qr` twice.
- `login_by_qr(rpc, password, chain_id)` — one `show_qr`.
- `add_passkey_by_qr(rpc, password, chain_id, label)` — the Settings/banner
  variant of steps 2–4 (account exists).

All three share `qr_ceremony(request) -> (url, wait)` next to
`browser_ceremony`: mint a `Relay`, build `request_url(AUTH_PAGE, request,
relay.callback_url())`, yield the URL, then `relay.wait()` on
`spawn_blocking` under the existing `CEREMONY_TIMEOUT` (5 min). A cancel
invalidates the lane; the relay entry expires on its own.

QR rendering: the Ice `qr <payload>` primitive (ducktape-ui 1334f31) — no
widget work. Payload = the auth-page URL with its fragment (~300 bytes;
correction `medium`).

## Relay — `ops/auth-page`

- `worker.js` with `run_worker_first = ["/r/*"]`; assets keep serving
  `index.html`. KV binding `CEREMONIES`.
  - `POST /r/<id>` — body `application/x-www-form-urlencoded`, field
    `result`; `put(id, result, { expirationTtl: 300 })`; responds 200
    `text/html` "Done — return to the app" (what the phone shows). Refuses
    a body over 16 KiB and an `id` that is not 43 chars of base64url.
  - `GET /r/<id>` — 200 `application/json` with the stored result, then
    `delete`; 204 while absent. No CORS needed (the app is not a browser).
- `index.html`: `loopbackOnly(cb)` → `allowedCallback(cb)`: loopback as
  today, OR same-origin `/r/<id>`. Nothing else in the page changes; the
  form POST is the same delivery.
- `README.md`: the `cb` row and a "Relay" section (the contract pin).
  `test.mjs`: `allowedCallback` accepts/refuses; `worker.js` exercised with a
  Map-backed KV (`put`/`get`/`delete`), dependency-free.
- Threat model: the relayed body is an assertion or a created credential's
  public key — public data. The app verifies every assertion against the
  account's keys (`login_consent`, `passkey_frame`); a forged or replayed
  post fails there. `id` is 32 random bytes from the app, so a stranger
  cannot poll a ceremony. No encryption.

## `crates/authpage`

- `pub struct Relay { id: String }`, `Relay::new()` (32 random bytes,
  base64url), `callback_url(&self) -> String` (`AUTH_PAGE + "r/" + id`),
  `wait(self) -> Result<Outcome, String>` — poll `GET` every 1.5 s until 200
  (`parse_result`) or the caller's deadline; a 204 keeps waiting; any other
  status is an error. `ureq`/`reqwest` — whichever the crate already has.
- `Listener` and `open_browser` stay for the desktop path and the CLI.
- CLI: untouched (`account login` keeps the browser). A terminal QR is a
  later `--qr`, only if asked.

## Settings

The account card keeps its role. "Register a passkey" and the banner's "Add
a passkey" run `add_passkey_by_qr` with the QR in the card; the desktop
button remains beside it. No "show recovery phrase" — an auto-minted key
has none (the words were dropped at mint), and the phrase-based restore is
for CLI-minted wallets only.

## Out of scope

Removing the password (rejected — the key file would sit unencrypted and
the shared CLI keystore would open unprompted); passkey-origin `Create`;
CLI QR; any identity-module or wire change.

## Testing

- `app/src/ui/tests/app.ice`: fresh device → `password` → `networks`;
  wallets present → `wallets`; network pick with an account → console, without
  → `account`; read-only skips the probe; `ceremony_stepped(show_qr)` renders
  a `qr`; cancel returns to the welcome; banner shows/dismisses and reopens
  the launch window at `account`.
- `app/src/backend`: `create_device_key` names and mints (temp keystore);
  `qr_ceremony` against an in-test loopback relay (the same fake serves the
  `authpage::Relay` unit tests: 204-then-200, timeout, malformed).
- `ops/auth-page/test.mjs`: page callback rule, worker round-trip, TTL args.
- Live: `make dev`, the deployed Worker, a real phone — create, sign in from
  a second workspace, add a passkey from Settings.

## Deploy

`npx wrangler kv namespace create CEREMONIES` (id into `wrangler.toml`),
`npx wrangler deploy` — the same account/OAuth path as the page itself
(`ops/auth-page/README.md`). The relay is live before the app PR merges, so
`make dev` on `dev` can exercise it.
