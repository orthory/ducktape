# First Sign-in with a Passkey — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A fresh install signs in as `password → network → create the account with a passkey (or sign in with one) by scanning a QR the app renders`, with the phone's answer relayed through the auth host.

**Architecture:** The auth page keeps its form-POST delivery but accepts a same-origin `/r/<id>` callback that a Worker stores in KV until the app polls it (`authpage::Relay`). The launch window's hub machine gains `password` (device key minted silently) and `account` (welcome screen) steps; ceremonies are backend streams that yield the QR URL first and the outcome last, so the Ice `qr` primitive renders while the wait is in flight. The console shows a banner when the device key has no account on the network.

**Tech Stack:** Rust (iced 0.14 app, Ice UI language via ducktape-ui 1334f31 with the `qr` primitive), `reqwest` 0.12 blocking (already a workspace dep), Cloudflare Workers + KV (`wrangler@4`), node for the page's dependency-free tests.

**Spec:** `docs/superpowers/specs/2026-08-28-first-signin-passkey-design.md`

## Global Constraints

- No wire, identity-module, or CLI change. The relay carries public data; no encryption.
- Callback rule on the page: loopback (`http://127.0.0.1`, `[::1]`, `localhost`) OR same-origin `/r/<id>`; nothing else, ever.
- Relay `id` = 32 random bytes, base64url without padding (43 chars); KV TTL 300 s; POST body ≤ 16 KiB; `GET` deletes on read; 204 while absent.
- Ceremony ceiling stays `CEREMONY_TIMEOUT` (300 s, `app/src/backend/node.rs`); poll every 1.5 s.
- Device key: name = sanitized hostname (`keystore::wallet::sanitize_name`), fallback `device`, `-2`…`-9` on collision; the 24 words are never shown or stored by the app.
- Every Ice extern struct field that carries a phase is a `str` (the `ProvisionStep.state` precedent): ceremony phases are exactly `working | show_qr | done | failed`.
- Ice HANDLER grammar (ducktape-ui 1334f31): `match`, `let`, `return if <cond>`, `parallel`, `run`/`stream`/`task` — but NO `if` blocks and NO calling another handler. A two-way decision is `match <bool>` with `true`/`false` arms; shared steps are inlined (and labelled). Views DO allow `if`. Ice has no string concatenation — text is built by `pure` Rust helpers.
- Repo rules: `tracing` not `println!`; format only touched hunks (`rustfmt --edition 2024`); per-crate gate `cargo clippy -p <crate> --tests --no-deps`; commit trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` + `Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM`; edit files with the Edit tool per hunk (no sed/python edit scripts).
- This host: one cargo at a time, `-j 4`, `CARGO_INCREMENTAL=0`; if rustc/lld segfaults, `cargo clean -p <victim>` and retry.
- Branch `feat/first-signin-passkey` in `.worktree/feat-identity-sshsig` (already checked out from `origin/dev`); PR against `dev`.

---

## File Structure

| File | Responsibility |
|---|---|
| `ops/auth-page/worker.js` (new) | the relay: `POST /r/<id>` stores, `GET /r/<id>` hands out once |
| `ops/auth-page/index.html` | `allowedCallback` replaces `loopbackOnly` |
| `ops/auth-page/wrangler.toml`, `.assetsignore`, `README.md`, `test.mjs` | Worker wiring, contract pin, tests |
| `crates/authpage/Cargo.toml`, `src/lib.rs` | `Relay` (mint id, callback URL, poll) beside `Listener` |
| `app/src/backend/hub.rs` | `create_device_key`, `device_key_name`, `hub_entry_step` → `Password` |
| `app/src/backend/node.rs` | `chain_id_of`, `CeremonyStep`, the three ceremony streams, `qr_ceremony` |
| `app/src/ui/state/types.ice`, `state/onboarding.ice`, `state/roster.ice` | `HubStep.{password,account}`, ceremony + banner state |
| `app/src/ui/extern/backend.ice` | new externs |
| `app/src/ui/components/onboarding.ice` | `PasswordScreen`, `WelcomeScreen` (replace `CreateScreen`/`RevealScreen`) |
| `app/src/ui/handlers/onboarding.ice`, `handlers/roster.ice` | the hub machine, banner, Settings QR |
| `app/src/ui/view.ice`, `screens/settings.ice` | mount changes, banner in the `notice:` slot, QR in the card |
| `app/src/ui/tests/app.ice` | Ice contracts |

---

### Task 1: Relay Worker and the page's callback rule

**Files:**
- Create: `ops/auth-page/worker.js`
- Modify: `ops/auth-page/index.html` (the `loopbackOnly` helper, ~line 62, and its call site in `parseRequest`, ~line 51)
- Modify: `ops/auth-page/wrangler.toml`, `ops/auth-page/.assetsignore`, `ops/auth-page/README.md`
- Test: `ops/auth-page/test.mjs`

**Interfaces:**
- Produces: `POST https://auth.ducktape.byeongsu.dev/r/<id>` (form field `result`) → 200 html; `GET /r/<id>` → 200 `application/json` once, 204 while absent, 404 for a malformed id. `worker.js` exports `default { fetch }` and a named `handle(request, env)` for tests.

- [x] **Step 1: Write the failing page test** — append to `ops/auth-page/test.mjs` before `console.log("auth-page: ok")`:

```js
// the callback is loopback OR this origin's relay path — a crafted link still cannot relay elsewhere
assert.equal(
  parseRequest("#op=get&challenge=AQID&cb=https://auth.ducktape.byeongsu.dev/r/abc", "https://auth.ducktape.byeongsu.dev").cb,
  "https://auth.ducktape.byeongsu.dev/r/abc",
);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=https://auth.ducktape.byeongsu.dev/x", "https://auth.ducktape.byeongsu.dev"), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=https://evil.example/r/abc", "https://auth.ducktape.byeongsu.dev"), /cb must be/);

// the relay worker, with a Map standing in for KV
const workerSrc = readFileSync(new URL("./worker.js", import.meta.url), "utf8");
const { handle } = await import(`data:text/javascript;base64,${Buffer.from(workerSrc).toString("base64")}`);
const store = new Map();
const kv = {
  async put(k, v, opts) { store.set(k, { v, ttl: opts?.expirationTtl }); },
  async get(k) { return store.get(k)?.v ?? null; },
  async delete(k) { store.delete(k); },
};
const env = { CEREMONIES: kv, ASSETS: { fetch: async () => new Response("asset", { status: 200 }) } };
const id = "A".repeat(43);
const waiting = await handle(new Request(`https://auth.example/r/${id}`), env);
assert.equal(waiting.status, 204);
const posted = await handle(new Request(`https://auth.example/r/${id}`, {
  method: "POST",
  headers: { "content-type": "application/x-www-form-urlencoded" },
  body: `result=${encodeURIComponent('{"op":"get"}')}`,
}), env);
assert.equal(posted.status, 200);
assert.equal(store.get(id).ttl, 300);
const taken = await handle(new Request(`https://auth.example/r/${id}`), env);
assert.equal(taken.status, 200);
assert.equal(await taken.text(), '{"op":"get"}');
assert.equal((await handle(new Request(`https://auth.example/r/${id}`), env)).status, 204);
assert.equal((await handle(new Request("https://auth.example/r/short"), env)).status, 404);
const big = await handle(new Request(`https://auth.example/r/${id}`, {
  method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body: "result=" + "x".repeat(17 * 1024),
}), env);
assert.equal(big.status, 413);
assert.equal((await handle(new Request("https://auth.example/"), env)).status, 200); // assets pass through
```

Also change the existing `parseRequest(...)` calls in the file: `parseRequest` now takes `(fragment, origin)` — pass `"https://auth.example"` as the second argument everywhere (a plain `sed`-free edit: there are eight call sites).

- [x] **Step 2: Run it to see it fail**

Run: `node ops/auth-page/test.mjs`
Expected: FAIL — `cb must be` thrown for the relay URL (or `Cannot find module ./worker.js`).

- [x] **Step 3: Page — `allowedCallback`**. In `index.html`, replace the `loopbackOnly` helper and its call:

```js
    const req = { op, challenge: pure.b64u.dec(get("challenge")), cb: pure.allowedCallback(p.get("cb"), origin) };
```
(`parseRequest(fragment, origin)` gains the `origin` parameter; the page's own call site passes `location.origin`.)

```js
  // The result (a signature the chain will accept) only ever goes to the
  // app/CLI listener on this machine, or to THIS origin's relay (`/r/<id>`,
  // which the app polls) — a crafted link cannot relay it elsewhere.
  allowedCallback(cb, origin) {
    if (cb === null) return null;
    const url = new URL(cb);
    const isLoopback = url.protocol === "http:" && ["127.0.0.1", "[::1]", "localhost"].includes(url.hostname);
    const isRelay = url.origin === origin && /^\/r\/[A-Za-z0-9_-]{43}$/.test(url.pathname);
    if (!isLoopback && !isRelay) throw new Error("cb must be http://127.0.0.1, [::1], localhost, or this origin's /r/<id>");
    return url.href;
  },
```

- [x] **Step 4: Worker** — `ops/auth-page/worker.js`:

```js
// The result relay: a phone that scanned the app's QR runs the ceremony
// here and POSTs the result to /r/<id>; the app polls the same path. KV holds
// it for five minutes, and a GET hands it out exactly once. Everything else
// is the static page.
const ID = /^\/r\/([A-Za-z0-9_-]{43})$/;
const TTL_SECONDS = 300;
const MAX_BODY = 16 * 1024;

export async function handle(request, env) {
  const url = new URL(request.url);
  const m = url.pathname.match(ID);
  if (url.pathname.startsWith("/r/") && !m) return new Response("no such ceremony", { status: 404 });
  if (!m) return env.ASSETS.fetch(request);
  const id = m[1];
  if (request.method === "POST") {
    const body = await request.text();
    if (body.length > MAX_BODY) return new Response("too large", { status: 413 });
    const result = new URLSearchParams(body).get("result");
    if (result === null) return new Response("no result", { status: 400 });
    await env.CEREMONIES.put(id, result, { expirationTtl: TTL_SECONDS });
    return new Response(
      "<!doctype html><meta charset=utf-8><title>ducktape</title><p style=\"font:16px system-ui;margin:2em\">Done — you can return to ducktape.</p>",
      { status: 200, headers: { "content-type": "text/html; charset=utf-8" } },
    );
  }
  if (request.method === "GET") {
    const result = await env.CEREMONIES.get(id);
    if (result === null) return new Response(null, { status: 204 });
    await env.CEREMONIES.delete(id);
    return new Response(result, { status: 200, headers: { "content-type": "application/json" } });
  }
  return new Response("method", { status: 405 });
}

export default { fetch: handle };
```

- [x] **Step 5: Wiring** — `wrangler.toml` becomes:

```toml
# The WebAuthn relying-party origin: the static page plus the result relay
# (`worker.js`, `/r/<id>`, KV-backed, five-minute TTL).
#   CLOUDFLARE_API_TOKEN=… CLOUDFLARE_ACCOUNT_ID=… npx wrangler@4 deploy
# `custom_domain = true` makes wrangler create the DNS record and the
# certificate for the hostname in the same account's zone.
name = "ducktape-auth"
main = "worker.js"
compatibility_date = "2026-08-01"
routes = [{ pattern = "auth.ducktape.byeongsu.dev", custom_domain = true }]

[assets]
directory = "."             # index.html only; .assetsignore drops the rest
binding = "ASSETS"
run_worker_first = ["/r/*"]

[[kv_namespaces]]
binding = "CEREMONIES"
id = "REPLACED-IN-TASK-7"   # `npx wrangler@4 kv namespace create CEREMONIES`
```

`.assetsignore` gains `worker.js`. In `README.md`: the `cb` table row becomes "the listener URL — **loopback**, or **this origin's `/r/<id>`** (the relay the app polls when the ceremony ran on a phone); any other host is refused before the ceremony"; add a section:

```markdown
## Relay — `/r/<id>`

When the app shows a QR instead of opening a browser, the phone runs the
ceremony and the app cannot be reached from it, so `cb` is this origin's
`/r/<id>` (`id` = 32 random bytes, base64url, minted by the app). `worker.js`:
`POST /r/<id>` stores the form's `result` in KV for 300 s and shows "Done";
`GET /r/<id>` answers 200 with the JSON exactly once (deleted on read), 204
while nothing has arrived. The body is an assertion or a public key — public
data the app verifies against the account's keys; a forged post fails there.
```

- [x] **Step 6: Run the tests**

Run: `node ops/auth-page/test.mjs`
Expected: `auth-page: ok`

- [x] **Step 7: Commit**

```bash
git add ops/auth-page
git commit -m "feat(auth-page): a result relay for ceremonies that ran on a phone"
```

---

### Task 2: `authpage::Relay`

**Files:**
- Modify: `crates/authpage/Cargo.toml`, `crates/authpage/src/lib.rs` (after `Listener`, ~line 300)
- Test: same file, `mod tests`

**Interfaces:**
- Produces: `pub struct Relay { base: String, id: String }`; `Relay::new() -> Relay` (at `AUTH_PAGE`); `Relay::at(base: &str) -> Relay`; `relay.callback_url() -> String` (`{base}r/{id}`); `relay.wait(deadline: Duration) -> Result<Outcome, String>`; `pub const RELAY_POLL: Duration = 1500 ms`.

- [x] **Step 1: Failing tests** — in `lib.rs`'s `mod tests`:

```rust
    /// a relay that answers 204 `absent` times, then the JSON once, then 204.
    fn fake_relay(absent: usize, json: &'static str) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let mut served = 0usize;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                assert!(line.starts_with("GET /r/"), "{line}");
                let response = if served < absent {
                    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                } else if served == absent {
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len())
                } else {
                    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                };
                stream.write_all(response.as_bytes()).unwrap();
                served += 1;
            }
        });
        base
    }

    #[test]
    fn a_relay_id_is_43_url_safe_chars_and_names_the_callback() {
        let relay = Relay::at("https://auth.example/");
        assert_eq!(relay.id.len(), 43);
        assert!(relay.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(relay.callback_url(), format!("https://auth.example/r/{}", relay.id));
        assert_ne!(Relay::at("x").id, Relay::at("x").id);
    }

    #[test]
    fn a_relay_waits_through_204s_and_takes_the_first_200() {
        let base = fake_relay(2, r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"KgAAAAAAAAA"}"#);
        let outcome = Relay::at(&base).wait(Duration::from_secs(20)).unwrap();
        assert!(matches!(outcome, Outcome::Get { user_handle: Some(42), .. }));
    }

    #[test]
    fn a_relay_gives_up_at_the_deadline() {
        let base = fake_relay(usize::MAX, "{}");
        let err = Relay::at(&base).wait(Duration::from_millis(10)).unwrap_err();
        assert!(err.contains("did not answer"), "{err}");
    }
```

- [x] **Step 2: Run** `cargo test -p authpage -j 4 relay` — Expected: compile error, `Relay` unknown.

- [x] **Step 3: Implement.** `Cargo.toml` dependencies gain `reqwest = { workspace = true }` (the workspace entry already has `blocking` + `rustls-tls`). In `lib.rs`:

```rust
// ============================================================================
// the relay callback — a ceremony that ran on a phone
// ============================================================================

/// how often [`Relay::wait`] asks the auth host whether the phone answered.
pub const RELAY_POLL: Duration = Duration::from_millis(1500);

/// the auth host's `/r/<id>` slot the page POSTs to when the ceremony ran on
/// a phone that cannot reach this machine; [`Relay::wait`] polls it.
pub struct Relay {
    base: String,
    pub id: String,
}

impl Relay {
    pub fn new() -> Self {
        Self::at(AUTH_PAGE)
    }

    /// at another deployment (`--auth-page`, tests). `base` ends with `/`.
    pub fn at(base: &str) -> Self {
        let mut raw = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
        let id = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw);
        Self { base: base.to_string(), id }
    }

    /// the `cb` to put in the request URL — the page accepts its own origin's `/r/<id>`.
    pub fn callback_url(&self) -> String {
        format!("{}r/{}", self.base, self.id)
    }

    /// block until the phone's result lands (200) or `deadline` passes; a 204
    /// is "not yet". Blocking on purpose — callers run it on a blocking thread
    /// exactly like [`Listener::wait`].
    pub fn wait(self, deadline: Duration) -> Result<Outcome, String> {
        let url = self.callback_url();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("relay client: {e}"))?;
        let started = std::time::Instant::now();
        loop {
            let response = client.get(&url).send().map_err(|e| format!("relay: {e}"))?;
            match response.status().as_u16() {
                200 => {
                    let body = response.text().map_err(|e| format!("relay body: {e}"))?;
                    return parse_result(&body);
                }
                204 => {}
                other => return Err(format!("relay answered {other}")),
            }
            if started.elapsed() >= deadline {
                return Err("the phone did not answer in time".into());
            }
            std::thread::sleep(RELAY_POLL.min(deadline.saturating_sub(started.elapsed())));
        }
    }
}
```

(`Duration` is `std::time::Duration`; add the `use` if the file lacks it. The `rand`/`base64` crates are already dependencies — match their existing call style in the file, e.g. `create_challenge`.)

- [x] **Step 4: Run** `cargo test -p authpage -j 4` — Expected: all pass, including the three new ones.

- [x] **Step 5: Lint + commit**

```bash
cargo clippy -p authpage --tests --no-deps -j 4
git add crates/authpage
git commit -m "feat(authpage): Relay — poll the auth host for a phone's answer"
```

---

### Task 3: Launch window — `password` step, device key minted silently

**Files:**
- Modify: `app/src/ui/state/types.ice:50-59` (`enum HubStep`), `app/src/ui/state/onboarding.ice` (drop `reveal_words`), `app/src/backend/hub.rs` (`hub_entry_step`, new `create_device_key`, `device_key_name`; tests ~line 660), `app/src/ui/extern/backend.ice:167` (extern), `app/src/ui/components/onboarding.ice` (`HubColumn` match + `CreateScreen`/`RevealScreen` → `PasswordScreen`), `app/src/ui/handlers/onboarding.ice` (`create_submit`/`key_created`/`reveal_confirm` → `password_submit`/`device_key_created`), `app/src/ui/view.ice:9-45` (props/events), `app/src/tests/design.rs` if it names a removed variant
- Test: `app/src/ui/tests/app.ice` (`create_screen_read_only_escape_contract` → `password_screen_read_only_escape_contract`; `launch_wallets_contract` events), `hub.rs` tests

**Interfaces:**
- Produces: `HubStep.password`, `HubStep.account` (the latter unused until Task 4); extern `create_device_key(password:str) -> str ! AppError` (returns pubkey hex); Ice events `password_submit(str)`, `device_key_created(str)`; component `PasswordScreen(busy:bool, error:str)` emitting `password_submit(str)`, `go_restore`, `login_skip` with ids `#device-password`, `#device-password-confirm`, `#password-submit`, `#password-skip`.

- [x] **Step 1: Failing Ice test** — replace `create_screen_read_only_escape_contract` in `app/src/ui/tests/app.ice`:

```
test password_screen_read_only_escape_contract
  preset ui_launch
  viewport 480 680
  mount
    PasswordScreen #pw
      with
        busy=false
        error="the keystore listing is unreadable"
      events
        password_submit -> password_submit _
        go_restore -> go_restore
        login_skip -> login_skip
  target screen = #pw/root
  target skip = #pw/root/password-skip
  target field = #pw/root/device-password
  expect exists field
  expect text "the keystore listing is unreadable" within screen
  // read-only signs as NOBODY: the label must not keep naming a wallet.
  click skip
  expect hub_wallet_selected == ""
  expect hub_step == HubStep.networks
```

In `launch_wallets_contract`'s `events` list replace `create_submit -> create_submit _ _` with `password_submit -> password_submit _` and delete `reveal_confirm -> reveal_confirm`; delete `reveal=""` from its `with`.

- [x] **Step 2: Failing Rust test** — in `hub.rs` `mod tests` replace the two `hub_entry_step` asserts:

```rust
    #[test]
    fn an_empty_keystore_starts_at_the_password_step() {
        assert!(matches!(hub_entry_step(vec![]), crate::HubStep::Password));
    }

    #[test]
    fn a_device_key_is_named_after_the_host_and_never_empty() {
        let name = device_key_name();
        assert!(!name.is_empty());
        assert!(keystore::wallet::valid_name(&name).is_ok(), "{name}");
    }
```

- [x] **Step 3: Run** `cargo test -p ducktape-app -j 4 hub::tests` — Expected: compile error (`Password` variant, `device_key_name` missing).

- [x] **Step 4: Ice types + state.** `types.ice`:

```
enum HubStep
  loading
  password
  wallets
  restore
  networks
  join
  provisioning
  live
  account
```

`state/onboarding.ice`: delete the `reveal_words = ""` line.

- [x] **Step 5: Backend.** In `hub.rs`:

```rust
pub fn hub_entry_step(wallets: Vec<WalletInfo>) -> crate::HubStep {
    if wallets.is_empty() {
        crate::HubStep::Password
    } else {
        crate::HubStep::Wallets
    }
}

/// The name an auto-minted device key gets: this host's name, in the
/// keystore's grammar; `device` when the host has none to give.
pub fn device_key_name() -> String {
    let host = std::fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_default();
    let name = keystore::wallet::sanitize_name(host.trim());
    if name.is_empty() {
        return "device".to_string();
    }
    name
}

/// Mint this device's key under `password` with no phrase shown — recovery
/// is a passkey login from another device. The words the keystore returns
/// are dropped here; the pubkey seeds the identity cache. Returns pubkey-hex.
pub async fn create_device_key(password: String) -> Result<String, AppError> {
    async {
        require_password(&password)?;
        let duck = duck_home()?;
        let base = device_key_name();
        let candidates = std::iter::once(base.clone())
            .chain((2..10).map(|n| format!("{base}-{n}")));
        let taken = |name: &str| keystore::wallet::key_file(&duck, name).exists();
        let name = candidates
            .into_iter()
            .find(|name| !taken(name))
            .ok_or_else(|| "this host already holds nine device keys — pick one in the wallet list".to_string())?;
        let minting = {
            let (name, password) = (name.clone(), Zeroizing::new(password));
            let duck = duck.clone();
            in_the_keystore(move || keystore::wallet::create(&duck, &name, &password))
        };
        let (words, pubkey) = minting.await?;
        drop(Zeroizing::new(words));
        activate_wallet(&name).await?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}
```

(`require_password` lives in `rpc.rs` — import it as `unlock_wallet` does. If `keystore::wallet::key_file` is not `pub` from the app's view, use `keystore::wallet::list(&duck)` and check names instead.)

`extern/backend.ice` line 167 area: add `create_device_key(password:str) -> str ! AppError`; keep `create_user_key` (the CLI-shaped mint is still used by restore paths? — if `grep -rn create_user_key app/src/ui` finds no caller after this task, delete the extern AND the Rust fn).

- [x] **Step 6: Component.** In `components/onboarding.ice`, delete `CreateScreen` (line ~390) and `RevealScreen` (~566) and add, keeping `CreateScreen`'s exact box/input styling for the two fields:

```
component PasswordScreen(busy:bool, error:str)
  emits
    password_submit(str)
    go_restore
    login_skip
  state
    pw = ""
    pw2 = ""
  col #root w=428.0 gap=0.0
    HubBrand
      with
        title="Welcome to ducktape"
        caption="Set a password for this device. Your account lives on the network you join next — a passkey on your phone is how you get it back."
    box w=fill pt=26.0
      text "PASSWORD"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=8.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        input "" #device-password <-> pw
          with
            label="New password"
            hint="at least 8 characters"
            secure=true
            disabled=busy
            w=fill
            p=0.0
            text-size=13.0
            line-h=1.2
            font=code
            @control
          active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
          disabled value=hint
    box w=fill pt=10.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=line
          border-w=1.0
          r=10.0
        input "" #device-password-confirm <-> pw2
          with
            label="Confirm password"
            hint="type it again"
            secure=true
            disabled=busy
            submit=submit_password
            w=fill
            p=0.0
            text-size=13.0
            line-h=1.2
            font=code
            @control
          active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
          disabled value=hint
    if !empty(password_problem(pw, pw2))
      box w=fill pt=8.0
        text password_problem(pw, pw2)
          with
            size=12.0
            @text-danger
    if !empty(error)
      box w=fill pt=8.0
        text error
          with
            size=12.0
            @text-danger
    box w=fill pt=18.0
      button "Continue" #password-submit -> submit_password
        with
          disabled=(busy || !empty(password_problem(pw, pw2)) || empty(pw))
          w=fill
          h=40.0
          @primary_action
    box w=fill pt=14.0
      row w=fill gap=14.0 align=center
        button "Restore from recovery phrase" -> emit(go_restore)
          with
            disabled=busy
            @link
        space w=fill
        button "Continue read-only" #password-skip -> emit(login_skip)
          with
            disabled=busy
            @link
  on submit_password
    emit(password_submit, pw)
```

(Copy the style utilities the deleted `CreateScreen` used for its danger text, primary button and links — read them off the deleted block before removing it; the names above are the intent, the deleted block is the truth.) In `HubColumn`: `match step` arms `HubStep.create` and `HubStep.reveal` become one `HubStep.password` arm mounting `PasswordScreen #password with busy error forward password_submit go_restore login_skip`; delete the `reveal` prop from `HubColumn`'s signature and the `reveal_confirm`/`create_submit` emits, add `password_submit(str)`.

- [x] **Step 7: Handlers.** In `handlers/onboarding.ice` replace `create_submit`, `key_created`, `reveal_confirm` with:

```
// PASSWORD — the device key is minted here, silently: no name, no phrase.
// Recovery is a passkey login from another device, so the words the
// keystore hands back are dropped before anything can show them.
on password_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every create_device_key(password) -> device_key_created _ | login_failed _

on device_key_created(_pubkey)
  mutation_phase = MutationPhase.idle
  hub_step = HubStep.networks
  run replace lane=hub_state hub_state() -> hub_refreshed _
```

(`hub_refreshed` fills `hub_wallets`/`hub_wallet_selected` so "signing as …" names the new key.) `go_login` keeps `hub_entry_step(hub_wallets)`. In `view.ice`: drop `reveal=reveal_words`, replace `create_submit -> create_submit _ _` with `password_submit -> password_submit _`, delete `reveal_confirm -> reveal_confirm`. Update `app/src/tests/design.rs` only if it names `Create`/`Reveal`.

- [x] **Step 8: Run** `cargo test -p ducktape-app -j 4` — Expected: PASS (Ice contracts + hub tests). If an Ice error names a style utility, read the deleted `CreateScreen` from `git show HEAD:app/src/ui/components/onboarding.ice` and use its exact names.

- [x] **Step 9: Commit**

```bash
git add app
git commit -m "feat(app): first launch asks for a password — the device key is minted silently"
```

---

### Task 4: `chain_id_of` and the account step's probe (no ceremonies yet)

**Files:**
- Modify: `app/src/backend/node.rs` (near `load_account`, ~line 1255), `app/src/ui/extern/backend.ice` (~line 267), `app/src/ui/state/onboarding.ice`, `app/src/ui/handlers/onboarding.ice` (`open_network_submit`, `connect_remote_submit`, `enter_console`), `app/src/ui/components/onboarding.ice` (`WelcomeScreen`, `HubColumn` arm), `app/src/ui/view.ice`
- Test: `app/src/ui/tests/app.ice`

**Interfaces:**
- Produces: extern `chain_id_of(rpc:str) -> str ! AppError`; state `hub_chain_id = ""`, `welcome_name_draft = ""`, `ceremony_phase = ""`, `ceremony_qr = ""`, `ceremony_detail = ""`; events `account_probed(AccountData)`, `account_probe_failed(HydrationError)`, `chain_named(str)`, `welcome_skip`, `welcome_cancel`; component `WelcomeScreen(network:str, name_draft:str (bind), phase:str, qr:str, detail:str, busy:bool, error:str)` emitting `welcome_create_submit(str)`, `welcome_login_submit`, `welcome_desktop`, `welcome_cancel`, `welcome_skip`, with ids `#welcome-name`, `#welcome-create`, `#welcome-login`, `#welcome-skip`, `#welcome-cancel`, `#welcome-desktop`, `#welcome-qr`. Task 5 wires the create/login/desktop events; this task leaves them as no-op handlers that only set `onboarding_error = ""`.

- [x] **Step 1: Failing Ice tests** — append to `app.ice`:

```
// THE WELCOME SCREEN is where a device key with no account on the picked
// network lands. Skipping opens the console (a window task — not asserted
// here); a QR in state renders as a real qr node; cancel clears it.
test welcome_screen_contract
  preset ui_launch
  viewport 480 680
  state
    hub_step = HubStep.account
    ceremony_phase = "show_qr"
    ceremony_qr = "https://auth.ducktape.byeongsu.dev/#op=get&challenge=AQID"
  mount
    WelcomeScreen name_draft<->welcome_name_draft #welcome
      with
        network="demo"
        phase=ceremony_phase
        qr=ceremony_qr
        detail=ceremony_detail
        busy=false
        error=""
      events
        welcome_create_submit -> welcome_create_submit _
        welcome_login_submit -> welcome_login_submit
        welcome_desktop -> welcome_desktop
        welcome_cancel -> welcome_cancel
        welcome_skip -> welcome_skip
  target qr = #welcome/root/welcome-qr
  target cancel = #welcome/root/welcome-cancel
  expect exists qr
  click cancel
  expect ceremony_qr == ""
  expect ceremony_phase == ""
  expect hub_step == HubStep.account

// A network pick probes the account BEFORE the console opens: no account →
// the welcome step; an account → straight through (window task, not asserted).
test network_pick_lands_on_the_welcome_step_without_an_account
  preset ui_launch
  state
    password = "hunter22"
    rpc = "http://127.0.0.1:1"
  dispatch account_probed(account_data_none(7))
  expect hub_step == HubStep.account
```

(`account_data_none(generation:i64) -> AccountData` is a new `pure` extern for tests — Ice cannot construct extern structs; it wraps `AccountData::none`.)

- [x] **Step 2: Run** `cargo test -p ducktape-app -j 4 ice` — Expected: FAIL, `WelcomeScreen` unknown.

- [x] **Step 3: Backend.** In `node.rs` after `load_account`:

```rust
/// The chain a network names, read once off `/v1/status` — the welcome step
/// runs before the console's status stream exists, and every key consent is
/// chain-scoped.
pub async fn chain_id_of(rpc: String) -> Result<String, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status().await?;
        let chain_id = status["chain_id"].as_str().unwrap_or_default().to_string();
        named_chain(chain_id)
    }
    .await
    .map_err(app_error)
}

/// test seam: Ice cannot construct an extern struct.
pub fn account_data_none(generation: i64) -> AccountData {
    AccountData::none(generation)
}
```

`extern/backend.ice`: `chain_id_of(rpc:str) -> str ! AppError` and `pure account_data_none(generation:i64) -> AccountData`.

- [x] **Step 4: State** — `state/onboarding.ice` gains:

```
  hub_chain_id = ""
  welcome_name_draft = ""
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
```

- [x] **Step 5: Component** — `WelcomeScreen` in `components/onboarding.ice` (styles as in `PasswordScreen`):

```
component WelcomeScreen(network:str, bind name_draft:str, phase:str, qr:str, detail:str, busy:bool, error:str)
  emits
    welcome_create_submit(str)
    welcome_login_submit
    welcome_desktop
    welcome_cancel
    welcome_skip
  col #root w=428.0 gap=0.0
    HubBrand
      with
        title="No account on this network yet"
        caption=network
    if phase == "show_qr"
      col w=fill gap=12.0 align=center
        box w=fill pt=20.0 align-x=center
          qr qr #welcome-qr size=(240.0) correction=medium
        text "Scan with your phone"
          with
            size=13.0
            @text-meta
        text detail
          with
            size=12.0
            @text-meta
        row w=fill gap=14.0 align=center
          button "Use this computer's passkey instead" #welcome-desktop -> emit(welcome_desktop)
            with
              @link
          space w=fill
          button "Cancel" #welcome-cancel -> emit(welcome_cancel)
            with
              @link
    if phase == "working"
      col w=fill gap=12.0 align=center
        box w=fill pt=20.0
          text detail
            with
              size=13.0
              @text-meta
        button "Cancel" #welcome-cancel -> emit(welcome_cancel)
          with
            @link
    if empty(phase)
      // NOTE: the block below is shown two spaces too deep (it was lifted out
      // of a match arm) — dedent every line of it by two when writing the file.
        col w=fill gap=0.0
          box w=fill pt=26.0
            text "ACCOUNT NAME"
              with
                size=10.0
                wrap=none
                font=code_semibold
                @text-label
          box w=fill pt=8.0
            box
              with
                w=fill
                px=14.0
                py=12.0
                bg=surface
                border=primary
                border-w=1.5
                r=10.0
              input "" #welcome-name <-> name_draft
                with
                  label="Account name"
                  hint="shown to others; not unique"
                  disabled=busy
                  submit=submit_create
                  w=fill
                  p=0.0
                  text-size=13.0
                  line-h=1.2
                  font=code
                  @control
                active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
                disabled value=hint
          if !empty(error)
            box w=fill pt=8.0
              text error
                with
                  size=12.0
                  @text-danger
          box w=fill pt=18.0
            button "Create account with a passkey" #welcome-create -> submit_create
              with
                disabled=(busy || empty(trim(name_draft)))
                w=fill
                h=40.0
                @primary_action
          box w=fill pt=10.0
            button "Sign in with a passkey" #welcome-login -> emit(welcome_login_submit)
              with
                disabled=busy
                w=fill
                h=40.0
                @secondary_action
          box w=fill pt=14.0
            row w=fill align=center
              space w=fill
              button "Continue without an account" #welcome-skip -> emit(welcome_skip)
                with
                  disabled=busy
                  @link
  on submit_create
    emit(welcome_create_submit, trim(name_draft))
```

`HubColumn` gains props `network:str, name_draft:str (bind), phase:str, qr:str, detail:str` and the arm:

```
        HubStep.account
          WelcomeScreen name_draft<->name_draft #welcome
            with
              network
              phase
              qr
              detail
              busy
              error
            forward
              welcome_create_submit
              welcome_login_submit
              welcome_desktop
              welcome_cancel
              welcome_skip
```

and the five emits. `view.ice` passes `network=network_label(account_name, rpc)` (or `rpc` when that helper needs a connected endpoint — check `network_label`'s signature), `name_draft<->welcome_name_draft`, `phase=ceremony_phase`, `qr=ceremony_qr`, `detail=ceremony_detail`, and routes the five events to same-named handlers.

- [x] **Step 6: Handlers** — in `handlers/onboarding.ice`:

```
// A NETWORK PICK PROBES THE ACCOUNT FIRST. The console opens only for a
// device key that has one (or a read-only session, which has no key to ask
// about); a key with none lands on the welcome step for that network.
// Read-only has nothing to probe: no key, no account, no welcome. The probe
// block is INLINED in the three pickers (a handler cannot call a handler).
on open_network_submit
  return if mutation_phase != MutationPhase.idle || empty(selected_network_endpoint(hub_networks, hub_selected))
  rpc = selected_network_endpoint(hub_networks, hub_selected)
  onboarding_error = ""
  let read_only = empty(password)
  match read_only
    true
      task window open console -> console_opened _
    false
      mutation_phase = MutationPhase.onboarding
      parallel
        run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
        run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | account_probe_failed _

on connect_remote_submit(endpoint)
  return if mutation_phase != MutationPhase.idle || empty(trim(endpoint))
  rpc = canonical_endpoint(endpoint)
  onboarding_error = ""
  let read_only = empty(password)
  match read_only
    true
      task window open console -> console_opened _
    false
      mutation_phase = MutationPhase.onboarding
      parallel
        run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
        run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | account_probe_failed _

on enter_console
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  let read_only = empty(password)
  match read_only
    true
      task window open console -> console_opened _
    false
      mutation_phase = MutationPhase.onboarding
      parallel
        run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
        run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | account_probe_failed _

on chain_named(id)
  hub_chain_id = id

on account_probed(next)
  mutation_phase = MutationPhase.idle
  let found = next.exists
  match found
    true
      task window open console -> console_opened _
    false
      ceremony_phase = ""
      ceremony_qr = ""
      ceremony_detail = ""
      hub_step = HubStep.account

// A node that cannot answer the probe is a node the console cannot use
// either: say so where the user is, keep the pick.
on account_probe_failed(cause)
  mutation_phase = MutationPhase.idle
  onboarding_error = cause.message

on welcome_skip
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  task window open console -> console_opened _

on welcome_cancel
  invalidate lane=ceremony
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""

// wired in the ceremonies task
on welcome_create_submit(_name)
  onboarding_error = ""

on welcome_login_submit
  onboarding_error = ""

on welcome_desktop
  onboarding_error = ""
```

Declare `lane=ceremony`, `lane=account_probe`, `lane=chain_probe` wherever the file's other lanes are declared. `account_generation` is roster state — bump it in `account_probed` only if the console's `load_account` later ignores stale generations (read `account_loaded`'s guard: `return if next.generation != account_generation`) — the probe uses the CURRENT generation, so no bump is needed.

- [x] **Step 7: Run** `cargo test -p ducktape-app -j 4` — Expected: PASS.

- [x] **Step 8: Commit**

```bash
git add app
git commit -m "feat(app): a network pick probes the account; no account lands on the welcome step"
```

---

### Task 5: Ceremony streams — create with a passkey, sign in, add a passkey

**Files:**
- Modify: `app/src/backend/node.rs` (after `login_with_passkey`, ~line 1640), `app/src/ui/extern/backend.ice`, `app/src/ui/handlers/onboarding.ice`
- Test: `app/src/backend/node.rs` `mod tests` (or `app/src/tests/`), `app/src/ui/tests/app.ice`

**Interfaces:**
- Consumes: `authpage::Relay` (Task 2), `HubStep.account`/state (Task 4), existing `create_account`, `own_account`, `consented_add_key`, `passkey_frame_request`, `passkey_frame`, `login_request`, `login_consent`, `login_add_key`, `signed_write`, `submit_raw_frame`, `key_generation`, `local_user_key`, `CEREMONY_TIMEOUT`.
- Produces: extern struct `CeremonyStep(phase:str, qr:str, detail:str)`; externs `stream create_account_by_qr(rpc:str, password:str, chain_id:str, name:str) -> CeremonyStep`, `stream login_by_qr(rpc:str, password:str, chain_id:str) -> CeremonyStep`, `stream add_passkey_by_qr(rpc:str, password:str, chain_id:str, label:str) -> CeremonyStep`; Rust `pub(crate) async fn qr_ceremony(relay_base: &str, request: authpage::Request, tx: &mut Sender<CeremonyStep>) -> Result<authpage::Outcome, String>`.

- [x] **Step 1: Failing Rust test** — in `node.rs` tests (the fake relay from Task 2 is `authpage`-private; re-declare a minimal one here, serving `GET` 204 once then the `get` JSON):

```rust
    #[tokio::test]
    async fn a_qr_ceremony_shows_the_url_then_yields_the_outcome() {
        use iced::futures::StreamExt;
        let base = fake_relay_once(r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"KgAAAAAAAAA"}"#);
        let (mut tx, mut rx) = iced::futures::channel::mpsc::channel::<CeremonyStep>(8);
        let request = authpage::Request::Get { challenge: [7u8; 32] };
        let outcome = qr_ceremony(&base, request, &mut tx).await.unwrap();
        assert!(matches!(outcome, authpage::Outcome::Get { user_handle: Some(42), .. }));
        let shown = rx.next().await.unwrap();
        assert_eq!(shown.phase, "show_qr");
        assert!(shown.qr.starts_with("https://auth.ducktape.byeongsu.dev/#op=get&challenge="), "{}", shown.qr);
        assert!(shown.qr.contains(&format!("cb={}", urlencoding_free_check(&base))), "{}", shown.qr);
    }
```

(`urlencoding_free_check` is not a thing — assert instead that `shown.qr.contains("cb=")` and that decoding the `cb` param with `authpage`'s own percent-decoding, if exposed, yields `{base}r/…`; otherwise assert `shown.qr.contains("%2Fr%2F")`.)

- [x] **Step 2: Run** `cargo test -p ducktape-app -j 4 a_qr_ceremony` — Expected: compile error.

- [x] **Step 3: Implement.** In `node.rs`:

```rust
/// One reading of a ceremony the launch window (or the Settings card) is
/// showing: `show_qr` carries the URL to render, `working` a line of what
/// the app is doing between touches, `done`/`failed` close the stream.
#[derive(Clone, Debug, PartialEq)]
pub struct CeremonyStep {
    pub phase: String,
    pub qr: String,
    pub detail: String,
}

impl CeremonyStep {
    fn working(detail: &str) -> Self {
        Self { phase: "working".into(), qr: String::new(), detail: detail.into() }
    }
    fn show_qr(url: String, detail: &str) -> Self {
        Self { phase: "show_qr".into(), qr: url, detail: detail.into() }
    }
    fn done() -> Self {
        Self { phase: "done".into(), qr: String::new(), detail: String::new() }
    }
    fn failed(message: String) -> Self {
        Self { phase: "failed".into(), qr: String::new(), detail: message }
    }
}

type StepSender = iced::futures::channel::mpsc::Sender<CeremonyStep>;

async fn step(tx: &mut StepSender, step: CeremonyStep) -> Result<(), String> {
    use iced::futures::SinkExt;
    tx.send(step).await.map_err(|_| "the ceremony was cancelled".to_string())
}

/// One browser ceremony run ON A PHONE: mint a relay slot, hand the URL to
/// the UI as a QR, then wait for the phone's answer under the same ceiling
/// the desktop path uses.
pub(crate) async fn qr_ceremony(
    relay_base: &str,
    request: authpage::Request,
    tx: &mut StepSender,
) -> Result<authpage::Outcome, String> {
    let relay = authpage::Relay::at(relay_base);
    let url = authpage::request_url(authpage::AUTH_PAGE, &request, &relay.callback_url());
    let detail = match request {
        authpage::Request::Create { .. } => "Your phone will create the passkey.",
        authpage::Request::Get { .. } => "Your phone will confirm with the passkey.",
        authpage::Request::Eth { .. } => "Your phone will sign with the wallet.",
    };
    step(tx, CeremonyStep::show_qr(url, detail)).await?;
    let waiting = tokio::task::spawn_blocking(move || relay.wait(CEREMONY_TIMEOUT));
    waiting.await.map_err(|_| "the ceremony did not finish".to_string())?
}

/// Run `body` as a step stream: every `Err` becomes a `failed` step, `Ok` a
/// `done` one, and dropping the receiver (a cancel) ends the task.
fn ceremony_stream<F, Fut>(body: F) -> iced::futures::stream::BoxStream<'static, CeremonyStep>
where
    F: FnOnce(StepSender) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    use iced::futures::{SinkExt, StreamExt};
    let (tx, rx) = iced::futures::channel::mpsc::channel::<CeremonyStep>(8);
    tokio::spawn(async move {
        let mut done_tx = tx.clone();
        let outcome = body(tx).await;
        let last = match outcome {
            Ok(()) => CeremonyStep::done(),
            Err(message) => CeremonyStep::failed(message),
        };
        let _ = done_tx.send(last).await;
    });
    rx.boxed()
}

/// Create the account with this device's key (no touch), then register a
/// passkey from the phone: QR 1 creates it, this device consents, QR 2 has
/// the passkey sign its own admission.
pub fn create_account_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
    name: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        require_password(&password)?;
        step(&mut tx, CeremonyStep::working("Creating the account…")).await?;
        create_account(rpc.clone(), password.clone(), name).await.map_err(|e| e.message)?;
        add_passkey_steps(&mut tx, &rpc, password, &chain_id, None).await
    })
}

/// Register a passkey on the account this device's key already belongs to.
pub fn add_passkey_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
    label: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        let label = optional_label(label)?;
        require_password(&password)?;
        add_passkey_steps(&mut tx, &rpc, password, &chain_id, label).await
    })
}

/// QR 1 (create) → consent → QR 2 (the passkey signs its AddKey) → submit.
async fn add_passkey_steps(
    tx: &mut StepSender,
    rpc: &str,
    password: String,
    chain_id: &str,
    label: Option<String>,
) -> Result<(), String> {
    let client = rpc_client(rpc)?;
    let account = own_account(&client).await?;
    let registered = qr_ceremony(
        authpage::AUTH_PAGE,
        authpage::Request::Create {
            challenge: authpage::create_challenge(),
            user: account.number,
            name: account.name,
        },
        tx,
    )
    .await?;
    let authpage::Outcome::Create { public_key, .. } = registered else {
        return Err("expected a passkey registration".to_string());
    };
    step(tx, CeremonyStep::working("Consenting to the new key…")).await?;
    let msg = consented_add_key(&client, password, chain_id, identity::KeyScheme::Secp256r1, &public_key, label).await?;
    let (request, preimage) = authpage::passkey_frame_request(&public_key, next_sequence(), &identity_msg(&msg));
    let signed = qr_ceremony(authpage::AUTH_PAGE, request, tx).await?;
    step(tx, CeremonyStep::working("Submitting…")).await?;
    submit_raw_frame(&client, "identity", authpage::passkey_frame(preimage, &signed)?).await?;
    Ok(())
}

/// Admit THIS device by a passkey's consent given on the phone: one QR.
pub fn login_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        require_password(&password)?;
        let Some(device_key) = local_user_key().await else {
            return Err("this device has no user key".to_string());
        };
        let client = rpc_client(&rpc)?;
        let generation = key_generation(&client, &device_key).await?;
        let consent = qr_ceremony(authpage::AUTH_PAGE, authpage::login_request(&chain_id, &device_key, generation), &mut tx).await?;
        let (number, proof) = authpage::login_consent(&consent)?;
        step(&mut tx, CeremonyStep::working("Joining the account…")).await?;
        let account = account_reply(client.query("identity", &identity::IdentityQuery::Get { number }).await?)?
            .ok_or_else(|| format!("the passkey names account {number}, unknown to this node"))?;
        let msg = authpage::login_add_key(&chain_id, &device_key, generation, &account, None, proof)?;
        signed_write(&client, "identity", identity::encode_msg(&msg), password).await?;
        Ok(())
    })
}
```

(`create_account` returns `Result<bool, AppError>` — use whatever field carries the message on `AppError`; if it is `AppError::message`, the `.map_err` above is right, else adapt. `own_account` returns `AccountView { number, name, .. }` — matches `register_passkey`'s use.)

`extern/backend.ice`:

```
  CeremonyStep(phase:str, qr:str, detail:str)
  stream create_account_by_qr(rpc:str, password:str, chain_id:str, name:str) -> CeremonyStep
  stream login_by_qr(rpc:str, password:str, chain_id:str) -> CeremonyStep
  stream add_passkey_by_qr(rpc:str, password:str, chain_id:str, label:str) -> CeremonyStep
```

- [x] **Step 4: Handlers** — replace the three placeholders in `handlers/onboarding.ice`:

```
// THE CEREMONIES. Each is a stream on ONE lane: the first step is the QR to
// show, `working` lines fill the gaps between touches, and `done`/`failed`
// close it. `welcome_cancel` invalidates the lane, which drops the stream's
// receiver — the backend task ends on its next send.
on welcome_create_submit(name)
  return if mutation_phase != MutationPhase.idle || empty(name) || empty(hub_chain_id)
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  stream replace lane=ceremony create_account_by_qr(rpc, password, hub_chain_id, name) -> ceremony_stepped _

on welcome_login_submit
  return if mutation_phase != MutationPhase.idle || empty(hub_chain_id)
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  stream replace lane=ceremony login_by_qr(rpc, password, hub_chain_id) -> ceremony_stepped _

// The desktop path: the existing browser ceremonies, from the welcome.
// Create was already committed by the time a QR shows, so this is the
// Settings card's register / login pair with the welcome's own landing.
on welcome_desktop
  return if ceremony_phase != "show_qr"
  invalidate lane=ceremony
  ceremony_phase = "working"
  ceremony_qr = ""
  ceremony_detail = "Continue in the browser…"
  let creating = !empty(trim(welcome_name_draft))
  match creating
    true
      run replace lane=ceremony register_passkey(rpc, password, hub_chain_id, "") -> welcome_desktop_done _ | welcome_failed _
    false
      run replace lane=ceremony login_with_passkey(rpc, password, hub_chain_id, "") -> welcome_desktop_done _ | welcome_failed _

// Same landing as a `done` step (inlined: a handler cannot call a handler).
on welcome_desktop_done(_ok)
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  task window open console -> console_opened _

on ceremony_stepped(next)
  let phase = next.phase
  ceremony_phase = phase
  ceremony_qr = next.qr
  ceremony_detail = next.detail
  match phase
    "done"
      mutation_phase = MutationPhase.idle
      ceremony_phase = ""
      ceremony_qr = ""
      task window open console -> console_opened _
    "failed"
      mutation_phase = MutationPhase.idle
      ceremony_phase = ""
      ceremony_qr = ""
      onboarding_error = next.detail
    _
      onboarding_error = ""

on welcome_failed(cause)
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  onboarding_error = cause.message
```

`welcome_desktop` picks the desktop ceremony by the one fact the welcome has — a non-empty name draft means the user was creating (the account already exists by the time a QR shows, so `register_passkey` is the right continuation); otherwise it is a login. `welcome_cancel` (Task 4) already invalidates `lane=ceremony`.

- [x] **Step 5: Ice test** — append:

```
// A ceremony's steps drive the welcome: show_qr renders, failed lands the
// message on the screen and frees the machine.
test ceremony_steps_drive_the_welcome
  preset ui_launch
  state
    hub_step = HubStep.account
    mutation_phase = MutationPhase.onboarding
  dispatch ceremony_stepped(ceremony_step("show_qr", "https://auth.ducktape.byeongsu.dev/#op=get", "Your phone will confirm."))
  expect ceremony_phase == "show_qr"
  expect ceremony_qr == "https://auth.ducktape.byeongsu.dev/#op=get"
  dispatch ceremony_stepped(ceremony_step("failed", "", "the phone did not answer in time"))
  expect ceremony_qr == ""
  expect onboarding_error == "the phone did not answer in time"
  expect mutation_phase == MutationPhase.idle
```

(`pure ceremony_step(phase:str, qr:str, detail:str) -> CeremonyStep` — the test constructor, declared in `extern/backend.ice` and implemented as a one-line `pub fn ceremony_step(phase: String, qr: String, detail: String) -> CeremonyStep` in `node.rs`.)

- [x] **Step 6: Run** `cargo test -p ducktape-app -j 4` then `cargo clippy -p ducktape-app --tests --no-deps -j 4` — Expected: PASS, clean.

- [x] **Step 7: Commit**

```bash
git add app
git commit -m "feat(app): create the account with a passkey, or sign in with one, by QR"
```

---

### Task 6: Console banner and the Settings card's QR

**Files:**
- Modify: `app/src/ui/state/roster.ice` (or wherever `account_exists` is declared — `grep -rn 'account_exists = ' app/src/ui/state`), `app/src/ui/view.ice` (`notice:` slot after `WorkspaceTabs`, ~line 124; `console_opened` reset in `handlers/onboarding.ice`), `app/src/ui/handlers/roster.ice` (`account_passkey_submit`, new handlers), `app/src/ui/handlers/onboarding.ice` (`open_account_welcome`, `welcome_reopened`), `app/src/ui/screens/settings.ice` (~line 565)
- Test: `app/src/ui/tests/app.ice`

**Interfaces:**
- Produces: state `account_banner_dismissed = false`, `account_ceremony_phase = ""`, `account_ceremony_qr = ""`, `account_ceremony_detail = ""`; events `dismiss_account_banner`, `open_account_welcome`, `welcome_reopened(window-id)`, `account_ceremony_stepped(CeremonyStep)`, `account_ceremony_cancel`, `account_passkey_desktop`.

- [x] **Step 1: Failing Ice tests** — append:

```
// THE BANNER: a connected console whose device key has no account says so,
// once; dismiss hides it for the session; reopening the launch window at the
// welcome is a window task (not asserted).
test console_banner_names_the_missing_account
  preset ui_settings
  state
    connected = true
    account_exists = false
    account_banner_dismissed = false
  target banner = #console/account-banner
  expect exists banner
  dispatch dismiss_account_banner
  expect account_banner_dismissed == true
  expect not exists banner

// The Settings card renders the ceremony's QR in place.
test settings_card_shows_the_passkey_qr
  preset ui_settings
  state
    connected = true
    account_exists = true
    account_ceremony_phase = "show_qr"
    account_ceremony_qr = "https://auth.ducktape.byeongsu.dev/#op=create"
  target qr = #settings/settings-body/account-ceremony-qr
  expect exists qr
  dispatch account_ceremony_cancel
  expect account_ceremony_qr == ""
```

(Adjust the two `target` paths to the ids the mounted tree actually has — `preset ui_settings` mounts the console; read the existing `settings_keyboard_scroll_contract` for the path prefix.)

- [x] **Step 2: Run** `cargo test -p ducktape-app -j 4 ice` — Expected: FAIL, `account_banner_dismissed` unknown.

- [x] **Step 3: State** — next to `account_exists`:

```
  account_banner_dismissed = false
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
```

In `console_opened` (handlers/onboarding.ice) add `account_banner_dismissed = false` and the three ceremony resets beside the other per-network resets.

- [x] **Step 4: Banner** — in `view.ice`'s `notice:` slot, before the `if has_error` box:

```
            if connected && !account_exists && !account_banner_dismissed && !empty(password)
              box #account-banner
                with
                  w=fill
                  pl=12.0
                  pr=12.0
                  pb=8.0
                box
                  with
                    w=fill
                    p=8.0
                    bg=surface
                    border=line
                    border-w=1.0
                    r=8.0
                  row w=fill gap=10.0 align=center
                    text "No account on this network yet."
                      with
                        size=12.5
                        wrap=none
                    space w=fill
                    button "Create or sign in" -> open_account_welcome
                      with
                        h=26.0
                        p=5.0
                        @secondary_action
                    button "×" -> dismiss_account_banner
                      with
                        label="Dismiss"
                        h=26.0
                        p=5.0
                        @link
```

(Copy the `bg`/`border` tokens the `has_error` box below it uses for its non-danger sibling, if one exists; the names above follow the settings card.) Handlers in `handlers/onboarding.ice`:

```
on dismiss_account_banner
  account_banner_dismissed = true

// The banner's way back: the launch window at the welcome step for THIS
// network. Same reopen mechanics as `switch_network`, different landing.
on open_account_welcome
  return if mutation_phase != MutationPhase.idle
  rpc = connected_rpc
  hub_chain_id = network_chain_id
  task window open onboarding -> welcome_reopened _

on welcome_reopened(id)
  onboarding_win = some(id)
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  onboarding_error = ""
  hub_step = HubStep.account
  parallel
    task window close target=window_target(console_win)
    task window close target=window_target(huddle_win)
```

(`switch_network` resets shell state before reopening — mirror the same invalidations here by extracting nothing: copy its `invalidate`/reset lines verbatim above the `task window open`, labelled `// same teardown as switch_network`.)

- [x] **Step 5: Settings QR** — in `handlers/roster.ice` replace `account_passkey_submit`:

```
// Register a passkey FROM THE PHONE: the card shows the QR the stream hands
// back; the desktop browser path stays one button over.
on account_passkey_submit
  return if !connected || !account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  stream replace lane=ceremony add_passkey_by_qr(connected_rpc, password, network_chain_id, trim(account_key_label_draft)) -> account_ceremony_stepped _

on account_passkey_desktop
  return if !connected || !account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  run every register_passkey(connected_rpc, password, network_chain_id, trim(account_key_label_draft)) -> account_changed _ | account_op_failed _

// `done` is `account_changed`'s body inlined (a handler cannot call a
// handler): the account picture moved, re-read it under a fresh generation.
on account_ceremony_stepped(next)
  let phase = next.phase
  account_ceremony_phase = phase
  account_ceremony_qr = next.qr
  account_ceremony_detail = next.detail
  match phase
    "done"
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_busy = false
      account_key_label_draft = ""
      account_generation = account_generation + 1
      run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    "failed"
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_busy = false
      error = next.detail
    _
      error = ""

on account_ceremony_cancel
  invalidate lane=ceremony
  account_busy = false
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
```

In `screens/settings.ice` at the "Register a passkey" button (~line 565): the button emits `account_passkey_submit` (now the QR path); add beside it `button "…in this browser" -> emit(account_passkey_desktop)` with the same `disabled`; below the row:

```
                    if account_ceremony_phase == "show_qr"
                      col w=fill gap=8.0 align=center
                        qr account_ceremony_qr #account-ceremony-qr size=(200.0) correction=medium
                        text account_ceremony_detail
                          with
                            size=12.0
                            @text-meta
                        button "Cancel" -> emit(account_ceremony_cancel)
                          with
                            h=26.0
                            p=5.0
                            @link
                    if account_ceremony_phase == "working"
                      text account_ceremony_detail
                        with
                          size=12.0
                          @text-meta
```

Add `account_passkey_desktop()` and `account_ceremony_cancel()` to `SettingsScreen`'s emits and the props `account_ceremony_phase`, `account_ceremony_qr`, `account_ceremony_detail`; route them in `view.ice` where the other `account_*` events are routed.

- [x] **Step 6: Run** `cargo test -p ducktape-app -j 4` and `cargo clippy -p ducktape-app --tests --no-deps -j 4` — Expected: PASS, clean.

- [x] **Step 7: Commit**

```bash
git add app
git commit -m "feat(app): a console without an account says so; Settings registers a passkey by QR"
```

---

### Task 7: Deploy the relay, run every gate, PR

**Files:**
- Modify: `ops/auth-page/wrangler.toml` (the KV id)

- [x] **Step 1: KV namespace + deploy** (the OAuth recipe in `ops/auth-page/README.md`; headless: `--browser=false` and curl the printed callback within 120 s):

```bash
npx wrangler@4 kv namespace create CEREMONIES --config ops/auth-page/wrangler.toml   # prints the id
# put the id into wrangler.toml's [[kv_namespaces]] entry
npx wrangler@4 deploy --config ops/auth-page/wrangler.toml
```

- [x] **Step 2: Live relay check**

```bash
ID=$(head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n')
curl -s -o /dev/null -w '%{http_code}\n' https://auth.ducktape.byeongsu.dev/r/$ID           # 204
curl -s -o /dev/null -w '%{http_code}\n' -d 'result={"op":"get"}' https://auth.ducktape.byeongsu.dev/r/$ID   # 200
curl -s https://auth.ducktape.byeongsu.dev/r/$ID; echo                                     # {"op":"get"}
curl -s -o /dev/null -w '%{http_code}\n' https://auth.ducktape.byeongsu.dev/r/$ID           # 204
```

- [x] **Step 3: Gates**

```bash
node ops/auth-page/test.mjs
cargo test -p authpage -j 4
cargo test -p ducktape-app -j 4
cargo clippy -p authpage -p ducktape-app --tests --no-deps -j 4
```

Expected: all green. Then `make dev` on this branch and, with a phone: create an account from the welcome (two scans), from a second workspace sign in (one scan), from Settings add a passkey. Note any refusal verbatim in the PR.

- [x] **Step 4: Commit the KV id, push, PR**

```bash
git add ops/auth-page/wrangler.toml
git commit -m "ops(auth-page): the CEREMONIES namespace"
git push -u origin feat/first-signin-passkey
gh pr create --base dev --title "feat(app): first sign-in with a passkey — QR ceremonies over a relay" --body-file <body>
```

PR body: the spec path, what a first launch now looks like, the relay contract, what was verified live and what was not, the footer `🤖 Generated with [Claude Code](https://claude.com/claude-code)` + the session URL.

---

## Self-review

- **Spec coverage:** decisions 1–4 → Tasks 3/4/6 (placement + banner), 1/2 (relay), 3 (device key), 5 (creation order). Flows → 4/5. Hub machine → 3/4. Ceremony streams → 5. Relay → 1/2. Settings → 6. Out of scope honoured (no CLI/module/wire change). Testing → each task's step 1 + Task 7. Deploy → Task 7.
- **Placeholders:** none; the two "if Ice forbids X, inline it" notes name the exact fallback.
- **Type consistency:** `CeremonyStep(phase, qr, detail)` everywhere; `Relay::at(base)` / `callback_url()` / `wait(Duration)` in Tasks 2 and 5; `HubStep.password`/`.account` in 3, 4, 6; `create_device_key(password) -> str` in 3; `chain_id_of(rpc) -> str` in 4, consumed as `hub_chain_id` in 5 and 6.
