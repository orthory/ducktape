# Desktop Notifications — Implementation Plan

**Design spec:** `~/.claude/plans/notification-system-for-the-foamy-pancake.md` (approved — this plan converts it into tasks, it does not redesign it).
**Worktree:** `<repo>/.claude/worktrees/desktop-notifications` (branch `feat/desktop-notifications`, based on `origin/dev`). All paths below are relative to the worktree root.

## 1. Context

The desktop app has no notification surface: mentions, huddles, finished runs, merged PRs, and admissions are invisible while the console is hidden to the menu bar. This plan builds the approved notifier — an AppHandle-owned Rust task in the Tauri shell (`app/src-tauri`) that consumes the new typed multiplexed `/v1/ws` stream, matches finalized ops (`indexer::OpRow` per `module:<id>` topic), and presents native toasts + a macOS badge — plus the app-only user-mention composer, prefs UI, and structured deep-links. The stream protocol is being built by another agent and is NOT on `dev` yet, so the plan is split: **Phase A** (tasks 1–8) is stream-independent and executes now; **Phase B** (tasks 9–12) is gated on the stream contract landing on `dev`.

## 2. Design concerns

Fact corrections found while confirming wire shapes (none change the architecture; matchers below use the verified shapes):

1. **Wire variant casing is snake_case, not PascalCase.** Every module wire enum carries `#[serde(rename_all = "snake_case")]` (`crates/apps/chat/src/interface.rs`, `crates/apps/forge/src/interface.rs`, `crates/system/governance/src/interface.rs`). The wire tags are `post_message`, `join_huddle`, `leave_huddle`, `sweep_huddle`, `merge_pr`, `propose`, `redeem`, `mention`, `user`, `agent`, `paragraph`, `quote`, `code`. (The stale "PascalCase enum variants" comment at the top of `app/src/domain/transport.ts` is wrong; the app's own encoders in `app/src/domain/chat-client.ts` emit `post_message` etc.) All sample JSON in this plan is the verified shape.
2. **Reply-root lookup:** the design says "one `/v1/query` chat `Message{root}` lookup", but `ChatQuery::Message` takes a `message_id`, not a seq. The seq-addressed equivalent is `{"messages_range": {"channel_id": <ch>, "from_seq": <root>, "limit": 1}}` → reply `{"messages": [MessageView]}`. Plan uses that.
3. **Run results land on `module:runs`, not `module:saga`.** The dispatch module delivers `ResultEvent` as a follow-up `Msg` to the receiver module (`crates/system/dispatch/src/lib.rs` ~727: `ctx.emit_msg(Msg { target: dispatch.receiver, payload: encode_result_event(..) })`), so the op row appears on the receiver's topic with `origin = {kind:"module", id:"dispatch"}`. Subscribe `module:runs` only; no `module:saga` subscription.
4. **Huddle dedupe can't observe roster emptiness from ops alone.** With live-from-tip start the notifier never sees the current roster; the matcher keeps an *observed* per-channel participant set (join adds, leave/sweep removes) and notifies on the observed empty→non-empty transition. A huddle already live at app start notifies once on the next observed join — accepted approximation of "deduped until a Leave/Sweep empties it".
5. **Desktop notification click-to-navigate is best-effort.** `tauri-plugin-notification` v2 has no reliable per-notification click callback on desktop/macOS. The structured `ducktape://navigate` deep-link machinery is still built (the tray popover and any future plugin support use it); clicking a toast activates the app but may not navigate. Badge + tray remain the guaranteed path. Not a redesign — noting so reviewers don't fail the e2e on it.
6. **File inventory addition:** `engine.rs` (frame→notification policy: prefs gating, focus suppression, cursor adoption, unread) is added to the design's `notify/` file list. It is what makes the Phase A/B split real: the engine consumes an *internal* `Frame` enum and is fully testable with injected frames now; Phase B's `stream.rs` only maps wire frames → `Frame`. Also added: `http.rs` (a ~60-line localhost-only HTTP POST helper for the reply-root query — no new HTTP crate, preserving dep hygiene).

## 2b. Pre-flight decisions (controller + user, before execution)

Two plan-internal issues were found in the pre-flight scan and resolved by the user. These OVERRIDE the task text where they conflict:

1. **Cursors are in-memory only; `state.json` persists `unread` alone.** Since app-start subscribes live-from-tip (Task 9) and Task 12 asserts a restart must not re-notify, persisted cursors were never read — dead state. `Engine` keeps `cursors` in memory for the in-session reconnect resume; `NotifyState` = `{ unread }`. Tasks 3 and 9 below reflect this.
2. **No `assert_sink` compile-bound test (Task 4).** `impl Sink for AppSink` is itself the compile-time check, and `cargo check -p ducktape-desktop` is already in Task 4's acceptance. A test with no runtime assertion would be flagged as a defect on every review pass.

## 3. Global Constraints (binding for every task)

- **Shell dep hygiene:** `app/src-tauri` (crate `ducktape-desktop`) stays free of host/module crate deps — the comment at the top of `app/src-tauri/Cargo.toml` is the contract. NO `chat`, `forge`, `noded`, `indexer`, `governance`, `dispatch`, `runs` crate deps. All op decoding is ad-hoc `serde_json::Value`.
- **Rust gate:** `cargo clippy -p ducktape-desktop --tests --no-deps` must be clean after every Rust task. Do NOT run `cargo fmt --all`; format only code you touched. `cargo check -p files --no-default-features` must stay green (should be untouched by this work).
- **Frontend gate:** `cd app && bun run test` (vitest) for touched areas; targeted runs like `bunx vitest run src/console/views/chat/mention.test.ts` are fine per task, full run before PR.
- **Mono-file mandate:** ~600-line soft cap per new file; split by responsibility.
- **Delivery:** work stays on `feat/desktop-notifications`; PR against `dev`; the PR must NOT merge before the stream contract PR is on `dev`. Phase B tasks must not start until the stream lands (rebase onto it first).
- **Phase B files freeze during Phase A:** do NOT touch `app/src/domain/transport.ts` or the block-tick/effect regions of `app/src/console/store/DucktapeProvider.tsx` in Phase A tasks — the stream agent is rewriting them.

### Verified wire facts (copy-check values)

- **OpRow envelope** (camelCase, `crates/kernel/indexer/src/lib.rs:263`):
  `{"height":u64,"seq":u32,"time":u64,"origin":{"kind":"external"|"module"|"system","id":"<string, absent for system>"},"payload":<json>|absent,"payloadHex":"<hex>"|absent}`
  `origin.id` for an external submitter is `indexer::user_handle(bytes)` — printable UTF-8 passes through as a name; a raw ed25519 pubkey (the desktop case) renders as **lowercase hex** (`bin/noded/src/lib.rs:941`).
- **Chat ops** (snake_case variants, snake_case fields):
  - `{"post_message":{"channel_id":s,"message_id":s,"blocks":[Block],"thread":u64|null,"as_agent":s|null}}`
  - `{"join_huddle":{"channel_id":s,"node":[u8;32 as JSON number array]}}`
  - `{"leave_huddle":{"channel_id":s}}`
  - `{"sweep_huddle":{"channel_id":s,"user":[u8...]}}`
  - Block: `{"paragraph":[Span]}` | `{"quote":[Span]}` | `{"code":{"lang":s|null,"text":s}}` | `"divider"`
  - Span: `{"text":s,"marks":[Mark]}`; Mark: `"bold"` | `"italic"` | `{"link":s}` | `{"mention":AuthorRef}`
  - AuthorRef: `{"user":[u8...]}` | `{"agent":{"module":s,"agent_id":s}}` | `{"module":s}` | `"system"`
- **Forge:** `{"merge_pr":{"repo":s ("" = "default"),"number":u64,"prev_target_oid":s,"expected_source_oid":s,"merge_oid":s,"pack_digest":s}}`
- **Governance:** `{"propose":{"proposal_id":s,"action":GovAction,"voting_period":u64}}` where admission actions are `{"add_validator":{"key":[u8...]}}` and `{"add_resident":{"key":[u8...]}}`; `{"redeem":{"issuer":[u8...],"nonce":[u8...],"token_sig":[u8...],"joiner":[u8...],"proof":[u8...]}}`
- **Run result** (topic `module:runs`, `origin = {"kind":"module","id":"dispatch"}`): `{"dispatch_id":s,"recipe_id":s,"outcome":{"Ok":[u8...]}|{"Err":s}}` (serde's default `Result` encoding: literal `"Ok"`/`"Err"` keys).
- **Node HTTP:** `POST /v1/query` body `{"target":"chat","query":<ChatQuery json>}` → the `ChatReply` enum json directly (e.g. `{"messages":[...]}`). `POST /v1/submit` body `{"target":s,"payload":<Msg json>,"origin":s}`.
- **Identity semantics:** on the desktop signed-frame path the chat author IS the op origin = the node pubkey (`status.publicKey`, 64-char hex). The durable user key comes from the identity module: webview state `nodeUsers: Record<nodeHex, {userKey, name|null}>` (`DucktapeProvider.tsx` ~194, built from `identityClient.allUsers` `UserView {user_key:number[], display_name:string|null, nodes:number[][]}`).
  - `selfUserKeyHex` — matches `Mention({"user":bytes})` (bytes→lowercase hex == selfUserKeyHex).
  - `selfNodeKeysHex` — ALL node keys bound to my user (from my `UserView.nodes`), lowercase hex; always contains `status.publicKey`. `is_me(hex) = selfNodeKeysHex.contains(lowercase(hex))` — used for "op.origin ≠ me", "root author == me", and own-JoinHuddle exclusion.
- **Magic strings:** event `"ducktape://navigate"` (existing, `tray.rs:241` / `DucktapeProvider.tsx` ~606); new event `"ducktape://notify-unread"` (payload `{"unread":number}`); commands `notify_configure`, `notify_mark_seen`; localStorage key `"ducktape.notifyPrefs"`; shell state file `<app_data_dir>/notify/state.json`; tray icon id `"ducktape"`; windows `"main"`/`"tray"`/`"huddle"`; capability file `app/src-tauri/capabilities/default.json`, permission `"notification:default"`.
- **Screen ids** (`app/src/console/modules/registry.ts`): `chat`, `pages`, `files`, `forge`, `agent`, `members`, `governance`, `modules`, `status`, `metrics`, `explorer`.
- **NavigateTarget** (structured deep-link payload, camelCase): `{"screen":s,"channelId"?:s,"threadRoot"?:u64,"repo"?:s,"number"?:u64}` — a plain string payload remains valid (legacy tray path) and means `{screen}`.
- **notify_configure payload** (camelCase, defined in Phase A task 7, consumed in Phase B):
  ```json
  {
    "nodeUrl": "http://127.0.0.1:PORT" | null,
    "selfUserKeyHex": "..." | null,
    "selfNodeKeysHex": ["..."],
    "focusedChannel": "general" | null,
    "mainWindowFocused": true,
    "authorNames": { "<hex>": "display name" },
    "prefs": { "enabled": true, "mentions": true, "replies": true, "huddles": true,
               "runs": true, "forge": true, "governance": true, "mutedChannels": [] }
  }
  ```
- **Stream contract (Phase B, verify against the landed spec/code before use):** typed `/v1/ws`, topics `module:<id>`, frames `event{topic,cursor,op}` / `lagged{topic,cursor}` / `heartbeat` / `subscribed`; subscribe with **no `resume` cursor ⇒ live-from-tip** (the notifier's one coordination requirement). Treat the cursor as an opaque `serde_json::Value`.

---

## 4. Task list

### Task 1: `notify/decode.rs` — OpRow + payload decode helpers

**Phase:** A
**Goal:** Pure `serde_json::Value` decode of the OpRow envelope and the payload probes every matcher needs. No I/O, no tauri types.
**Files:**
- create `app/src-tauri/src/notify/mod.rs` — for now just `pub mod decode;` plus a module doc comment (grows in later tasks)
- create `app/src-tauri/src/notify/decode.rs`
- modify `app/src-tauri/src/main.rs` — add `mod notify;` to the module list (no other change)

**Interfaces** (in `decode.rs`):
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OriginKind { External, Module, System }

#[derive(Debug, Clone)]
pub struct Origin { pub kind: OriginKind, pub id: Option<String> }

#[derive(Debug, Clone)]
pub struct OpRow {
    pub height: u64,
    pub seq: u32,
    pub time: u64,
    pub origin: Origin,
    /// the embedded op payload; None when the row carried `payloadHex` or nothing.
    pub payload: Option<serde_json::Value>,
}

/// Decode one OpRow envelope. Returns None on any malformed/missing field
/// (the notifier skips what it cannot read — never panics on wire data).
pub fn decode_op_row(v: &serde_json::Value) -> Option<OpRow>;

/// `payload.get(variant)` for a snake_case-tagged enum op — Some(fields) when
/// this op is that variant.
pub fn variant<'a>(payload: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value>;

/// A JSON number-array of bytes -> lowercase hex ("" for empty array,
/// None when not an array of 0..=255 numbers).
pub fn bytes_hex(v: &serde_json::Value) -> Option<String>;

/// Walk post_message `blocks` (paragraph/quote spans -> marks) and collect the
/// lowercase-hex user keys of every `{"mention":{"user":[..]}}` mark.
pub fn mention_user_hexes(blocks: &serde_json::Value) -> Vec<String>;

/// Flatten blocks to a short plain-text preview (paragraph/quote span text
/// joined, code -> its text, divider skipped), truncated to `max` chars on a
/// char boundary.
pub fn blocks_preview(blocks: &serde_json::Value, max: usize) -> String;
```
Origin decode: `kind` is the camelCase strings `"external"|"module"|"system"`; `id` optional.

**Tests first** (in-file `#[cfg(test)] mod tests`, the crate's existing convention — see `user_identity.rs`):
1. `decode_op_row` on a full envelope: `{"height":42,"seq":1,"time":1720000000,"origin":{"kind":"external","id":"aabb"},"payload":{"post_message":{"channel_id":"general","message_id":"m1","blocks":[],"thread":null,"as_agent":null}}}` → all fields; `variant(payload,"post_message")` is Some.
2. `payloadHex` row (`"payloadHex":"deadbeef"`, no `payload`) → `payload: None`; system origin (`{"kind":"system"}`) → `id: None`.
3. Malformed rows (missing height, origin not an object, non-numeric seq) → None, no panic.
4. `bytes_hex(json!([18,52]))` == `Some("1234")`; `bytes_hex(json!([256]))` == None; `bytes_hex(json!("x"))` == None.
5. `mention_user_hexes` over blocks `[{"paragraph":[{"text":"hi ","marks":[]},{"text":"@jess","marks":[{"mention":{"user":[18,52]}}]}]},{"quote":[{"text":"q","marks":[{"mention":{"agent":{"module":"runs","agent_id":"helper"}}}]}]},"divider"]` → `["1234"]` (agent mention ignored, divider skipped).
6. `blocks_preview` joins paragraph text, truncates multi-byte text without panicking (test with an emoji at the cut).

**Acceptance:** tests green via `cargo test -p ducktape-desktop notify::decode`; clippy gate clean; no new Cargo deps; `main.rs` diff is exactly the `mod notify;` line.
**Depends on:** —

---

### Task 2: `notify/matchers.rs` — per-trigger matchers

**Phase:** A
**Goal:** Pure `OpRow -> Option<Notification>` matchers for all six triggers, with the observed-huddle tracker and an injected reply-root resolver. No I/O, no tauri types.
**Files:**
- create `app/src-tauri/src/notify/matchers.rs`
- modify `app/src-tauri/src/notify/mod.rs` — add `pub mod matchers;`

**Interfaces:**
```rust
use super::decode::{self, OpRow, OriginKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category { Mention, Reply, Huddle, Run, Forge, Governance }

/// Structured deep-link target — serializes to the ducktape://navigate payload.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NavigateTarget {
    pub screen: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub thread_root: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub number: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub category: Category,
    pub title: String,
    pub body: String,
    pub target: NavigateTarget,
    /// the chat channel this belongs to (focus suppression / mute), None for
    /// runs/forge/governance.
    pub channel_id: Option<String>,
}

/// Everything identity-shaped a matcher needs. Built from NotifyConfig.
pub struct MatcherCtx<'a> {
    pub self_user_key_hex: Option<&'a str>,
    /// every node key bound to my user (always includes this node), lowercase.
    pub self_node_keys_hex: &'a [String],
    /// hex -> display name (webview-pushed); fall back to short hex.
    pub author_names: &'a std::collections::BTreeMap<String, String>,
    /// (channel_id, root_seq) -> the root message author's origin hex, or None
    /// when unresolvable. Injected: tests use a closure; Phase B wires /v1/query.
    pub root_author: &'a dyn Fn(&str, u64) -> Option<String>,
}

/// Observed huddle rosters: channel_id -> set of participant user-origin hexes.
#[derive(Debug, Default)]
pub struct HuddleTracker(std::collections::BTreeMap<String, std::collections::BTreeSet<String>>);

#[derive(Debug, Default)]
pub struct MatchState { pub huddles: HuddleTracker }

/// The single entry point the engine calls.
pub fn match_topic(topic: &str, op: &OpRow, ctx: &MatcherCtx, state: &mut MatchState)
    -> Option<Notification>;
```
`match_topic` dispatches on topic: `"module:chat"` → chat/huddle, `"module:runs"` → run, `"module:forge"` → forge, `"module:governance"` → governance; anything else → None. Internal per-topic fns may be private.

**Match rules (exact):**
- Helper `is_me(ctx, hex) = ctx.self_node_keys_hex.iter().any(|k| k.eq_ignore_ascii_case(hex))`. Helper `display_name(ctx, hex)` = `author_names.get(lowercase hex)` else `format!("{}…", &hex[..8.min(len)])`.
- **Mention** (`post_message`): origin is External AND NOT `is_me(origin.id)`; `mention_user_hexes(blocks)` contains `self_user_key_hex` (case-insensitive) → `Notification { Mention, title: "{name} mentioned you in #{channel_id}", body: blocks_preview(blocks, 140), target: {screen:"chat", channel_id, thread_root: thread}, channel_id }`. Mention wins over Reply when both apply (check mention first).
- **Reply** (`post_message` with `"thread": root != null`): origin External, not me, and `(ctx.root_author)(channel_id, root)` returns Some(hex) with `is_me(hex)` → `{ Reply, "{name} replied to your thread in #{channel_id}", preview, target {screen:"chat", channel_id, thread_root: Some(root)} }`.
- **Huddle** (`join_huddle`): let `joiner = origin.id` (external hex). Skip when the payload `node` bytes-hex OR the origin `is_me`. Notify ONLY when the channel's observed roster was empty before inserting the joiner → `{ Huddle, "Huddle started in #{channel_id}", "{name} started a huddle", target {screen:"chat", channel_id} }`. Always insert the joiner (even self) into the tracker. `leave_huddle` removes `origin.id` from the channel set; `sweep_huddle` removes `bytes_hex(user)`; both return None.
- **Run** (topic `module:runs`): origin `{Module, id=="dispatch"}` AND payload is an object with both `"dispatch_id"` and `"outcome"` keys → outcome `{"Ok":_}` ⇒ `{ Run, "Agent run finished", "dispatch {dispatch_id truncated to 12 chars}…", target {screen:"agent"} }`; `{"Err":e}` ⇒ title "Agent run failed", body = the Err string truncated to 140.
- **Forge** (`merge_pr`): `{ Forge, "PR #{number} merged in {repo}", body "" (or the merging author name), target {screen:"forge", repo: Some(repo), number: Some(number)} }` with `repo == ""` rendered/targeted as `"default"`. Broad scope: do NOT exclude own ops (design: "anything in my workspace").
- **Governance** (`propose` with action key `"add_validator"` or `"add_resident"`): `{ Governance, "New admission proposal", "proposal {proposal_id}", target {screen:"members"} }`. (`redeem`): `{ Governance, "New member admitted", "{short joiner hex} joined via invite", target {screen:"members"} }`. Other gov ops → None.

**Tests first** — build ops via a helper `fn op(origin_kind, origin_id, payload: serde_json::Value) -> OpRow`; canned ctx with `self_user_key_hex=Some("1234")`, `self_node_keys_hex=["aa..","bb.."]`, root_author closure over a `HashMap`:
1. post_message with `{"mention":{"user":[18,52]}}` from origin `"cc.."` → Some(Mention), title contains channel id; same op from origin `"aa.."` (me) → None; mention of a DIFFERENT user hex → None.
2. Reply: thread=Some(7), root_author returns `"aa.."` (mine) → Some(Reply) with `thread_root: Some(7)`; root_author returns `"cc.."` → None; root_author returns None → None. Mention+reply in one op → exactly one notification, category Mention.
3. Huddle: join from `"cc.."` on empty tracker → Some; second join `"dd.."` same channel → None (dedupe); `leave_huddle` from `"cc.."` then `"dd.."` (roster empties) then a new join → Some again; `sweep_huddle` emptying → same; my own join (`node` hex in self_node_keys_hex or origin me) → None but still tracked.
4. Run: `{"dispatch_id":"d1","recipe_id":"r","outcome":{"Ok":[1]}}` origin module "dispatch" → Some(Run, "finished"); `{"outcome":{"Err":"boom"}}` → "failed" + body "boom"; same payload with origin module `"runs"` → None; a `watch_channel` RunsMsg on the topic → None.
5. Forge: merge_pr number 7 repo "" → "PR #7 merged in default", target repo "default"; `open_pr` → None.
6. Governance: propose add_resident → Some; propose `{"signal":{"text":"x"}}` → None; redeem → Some.
7. `match_topic("module:pages", ...)` → None.

**Acceptance:** `cargo test -p ducktape-desktop notify::matchers` green; clippy gate clean; matchers contain no `tauri::`, no I/O, no new deps.
**Depends on:** 1

---

### Task 3: `notify/state.rs` + `notify/engine.rs` — persistence and the frame→notification engine

**Phase:** A
**Goal:** The stream-independent core loop: an internal `Frame` enum, a `Sink` seam (real toasts vs test capture), prefs/focus gating, unread counting, cursor adoption (incl. `lagged`, no backfill), and best-effort state persistence.
**Files:**
- create `app/src-tauri/src/notify/state.rs`
- create `app/src-tauri/src/notify/engine.rs`
- modify `app/src-tauri/src/notify/mod.rs` — add `pub mod engine; pub mod state;` and the shared config types:

```rust
// in mod.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotifyPrefs {
    pub enabled: bool, pub mentions: bool, pub replies: bool, pub huddles: bool,
    pub runs: bool, pub forge: bool, pub governance: bool,
    pub muted_channels: Vec<String>,
}
impl Default for NotifyPrefs { /* everything true, muted_channels empty */ }

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotifyConfig {
    pub node_url: Option<String>,
    pub self_user_key_hex: Option<String>,
    pub self_node_keys_hex: Vec<String>,
    pub focused_channel: Option<String>,
    pub main_window_focused: bool,
    pub author_names: std::collections::BTreeMap<String, String>,
    pub prefs: NotifyPrefs,
}
```

**Interfaces:**
```rust
// state.rs — persisted at <app_data_dir>/notify/state.json, pure fns over a path.
// ONLY `unread` is persisted (pre-flight decision 1): cursors are in-memory on the
// Engine, because app start always subscribes live-from-tip and never resumes from disk.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NotifyState {
    pub unread: u32,
}
pub fn load(path: &std::path::Path) -> NotifyState;          // missing/corrupt -> Default
pub fn save(path: &std::path::Path, state: &NotifyState);    // best-effort, creates parent dir, never panics

// engine.rs
/// Internal frame vocabulary — Phase B's stream.rs maps wire frames onto this.
#[derive(Debug, Clone)]
pub enum Frame {
    Event { topic: String, cursor: serde_json::Value, op: serde_json::Value },
    Lagged { topic: String, cursor: serde_json::Value },
    Heartbeat,
}

/// Presentation seam: production = present.rs over AppHandle; tests capture.
pub trait Sink: Send {
    fn present(&self, n: &matchers::Notification);
    fn badge(&self, unread: u32);
}

pub struct Engine<S: Sink> {
    /* sink, match_state, state_path, unread,
       cursors: BTreeMap<String, serde_json::Value>  // IN-MEMORY ONLY, never persisted */
}

impl<S: Sink> Engine<S> {
    pub fn new(sink: S, state_path: std::path::PathBuf) -> Self; // loads persisted unread, pushes initial badge
    /// One frame in. `config` is read per-frame (Phase B swaps it under a lock).
    /// `root_author` as in MatcherCtx.
    pub fn handle(&mut self, frame: Frame, config: &NotifyConfig,
                  root_author: &dyn Fn(&str, u64) -> Option<String>);
    /// notify_mark_seen: zero unread, clear badge, persist.
    pub fn mark_seen(&mut self);
    /// In-memory resume cursors for a TRANSIENT in-session reconnect (Phase B).
    /// Never persisted — app start is always live-from-tip.
    pub fn cursors(&self) -> &std::collections::BTreeMap<String, serde_json::Value>;
    /// Drop all cursors (Phase B: node_url changed -> old cursors are invalid).
    pub fn reset_cursors(&mut self);
}
```
**`handle` semantics (exact):**
- `Event`: always record `cursors[topic] = cursor` **in memory** (no disk write). Then decode via `decode::decode_op_row(&op)`; on Some, run `matchers::match_topic`. A produced notification is DROPPED (no present, no unread) when any of: `!prefs.enabled`; its category toggle is off (`Mention/Reply→mentions/replies`, `Huddle→huddles`, `Run→runs`, `Forge→forge`, `Governance→governance`); `channel_id` is Some and in `prefs.muted_channels`; or (focus suppression) `config.main_window_focused && notification.channel_id.is_some() && notification.channel_id == config.focused_channel`. Otherwise `sink.present(&n)`, `unread += 1`, `sink.badge(unread)`, and `save` (disk writes happen only when `unread` changes).
- `Lagged`: adopt `cursors[topic] = cursor` in memory, notify NOTHING (no backfill), no disk write.
- `Heartbeat`: no-op (liveness watchdog is stream.rs's, Phase B).
- Live-from-tip is a **connection** policy (Phase B never resumes from disk on app start — cursors are not persisted at all); the engine itself never replays anything it wasn't handed — add a test documenting that a fresh engine emits nothing until a frame arrives.

**Tests first** (`#[cfg(test)]`, a `CaptureSink(std::sync::Mutex<Vec<Notification>>)` + captured badge values; state paths under `std::env::temp_dir()` with a unique suffix, removed after):
1. Event frame carrying the Task 2 mention op (self keys in config) → 1 presented, unread 1, badge [1], `engine.cursors()["module:chat"]` == the frame's cursor, and `state.json` holds `{"unread":1}` which `load` round-trips.
2. Same op with `prefs.enabled=false` → nothing presented, unread stays 0, but the in-memory cursor STILL advances.
3. `mentions:false` drops Mention but a huddle op still presents; `muted_channels:["general"]` drops both for that channel.
4. Focus suppression: `main_window_focused=true, focused_channel=Some("general")` → mention in "general" dropped (unread unchanged), mention in "other" presented; `main_window_focused=false` → "general" mention presented.
5. Lagged adopts the cursor (in memory), presents nothing; a following Event presents normally.
6. `mark_seen` zeroes unread, badge [.., 0], persists; `Engine::new` on that path restores unread 0 and pushes badge 0. A path with `{"unread":5}` restores 5 and pushes badge 5.
7. Corrupt state.json → `load` returns Default (no panic).
8. A fresh `Engine` presents nothing and leaves `cursors()` empty until a frame arrives (documents live-from-tip: no disk cursors exist to replay).

**Acceptance:** `cargo test -p ducktape-desktop notify::` green (all three modules); clippy gate clean; no tauri types in engine/state (only `std` + serde). Files each under ~600 lines.
**Depends on:** 2

---

### Task 4: `notify/present.rs` — plugin, toast, badge; Cargo dep + capability

**Phase:** A
**Goal:** The production `Sink`: `tauri-plugin-notification` toasts + macOS dock badge (+ optional tray title), plugin registration, capability permission.
**Files:**
- create `app/src-tauri/src/notify/present.rs`
- modify `app/src-tauri/Cargo.toml` — add `tauri-plugin-notification = "2"` (with a one-line comment in the file's existing style)
- modify `app/src-tauri/src/main.rs` — `builder = builder.plugin(tauri_plugin_notification::init())` (unconditional, before `.setup`), plus `pub mod` exposure if needed
- modify `app/src-tauri/capabilities/default.json` — append `"notification:default"` to `permissions`

**Interfaces:**
```rust
// present.rs
use tauri::{AppHandle, Runtime};

/// The production Sink over an AppHandle.
pub struct AppSink<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> super::engine::Sink for AppSink<R> {
    fn present(&self, n: &super::matchers::Notification);
    fn badge(&self, unread: u32);
}
```
- `present`: `use tauri_plugin_notification::NotificationExt;` → `self.0.notification().builder().title(&n.title).body(&n.body).show()` — ignore the Result (log via `eprintln!` on error). Also `let _ = self.0.emit("ducktape://notify-unread", serde_json::json!({"unread": ...}))` is NOT done here — unread emission goes through `badge`.
- `badge`: on the `"main"` webview window call `set_badge_count(if unread == 0 { None } else { Some(unread as i64) })`, ignoring errors (Linux WebKitGTK may not support it). `#[cfg(target_os = "macos")]`: additionally set the tray title via `app.tray_by_id("ducktape")` → `set_title(Some(unread.to_string()))` when unread > 0, `set_title(None::<&str>)` at 0. Emit `"ducktape://notify-unread"` with `{"unread": n}` via `tauri::Emitter` so the webview can render its own indicator.
- `Sink` is `Send`: `AppHandle` is Send+Sync — fine.

**Tests:** presentation is OS-bound, so verification here is compile-level + gate-level: (a) `cargo clippy -p ducktape-desktop --tests --no-deps` clean; (b) capability JSON is valid — `cargo check -p ducktape-desktop` passes (the tauri build validates capabilities against the now-present plugin). Per pre-flight decision 2, do **NOT** add an `assert_sink::<AppSink>()` compile-bound test — the `impl Sink for AppSink` block is itself the compile-time check, and a test with no runtime assertion is a defect. Presentation behaviour stays covered by the Engine's `CaptureSink` tests (Task 3) and the live drive (Task 12).
**Acceptance:** `cargo check -p ducktape-desktop` + clippy gate green; `capabilities/default.json` contains `"notification:default"`; main.rs registers the plugin unconditionally; description comment in Cargo.toml follows the file's commented-dep style. No other permission changes.
**Depends on:** 3

---

### Task 5: user-mention helpers — `mention.ts` candidates/resolver

**Phase:** A
**Goal:** Extend the pure mention helpers so workspace USERS are mentionable alongside agents, emitting `{"mention":{"user":[bytes]}}` marks. App-only; the consensus collector already handles User mentions.
**Files:**
- modify `app/src/console/views/chat/mention.ts`
- modify `app/src/console/views/chat/mention.test.ts`

**Interfaces** (add; keep every existing export unchanged):
```ts
/** One mentionable workspace user, derived from identity's node->user map. */
export interface UserMentionCandidate {
  kind: "user";
  /** lowercase hex of the durable user key — the mention mark's bytes. */
  userKeyHex: string;
  /** the @token inserted into the composer; matches chat-input's charset [a-z0-9._-]. */
  handle: string;
  /** display label (chosen name or short hex). */
  label: string;
}
export interface AgentMentionCandidate { kind: "agent"; agent: AgentRecord; }
export type MentionCandidate = UserMentionCandidate | AgentMentionCandidate;

/** Distinct users from state.nodeUsers (dedupe by userKey). handle = display
 *  name slugified (lowercase, [^a-z0-9._-]+ -> "-", trimmed of leading/
 *  trailing "-"), falling back to userKeyHex.slice(0, 8) when empty; a handle
 *  colliding with an earlier user's or any agent_id gets "-2", "-3", ... */
export const mentionableUsers = (
  nodeUsers: Record<string, { userKey: string; name: string | null }>,
  agents: AgentRecord[],
): UserMentionCandidate[];

/** Agents-and-users matching `query` (case-insensitive, prefix-ranked like the
 *  existing agent-only filter). Agents keep their existing relative order and
 *  rank rules; users interleave by the same prefix-first rule, ties agents-first. */
export const mentionCandidatesAll = (
  agents: AgentRecord[],
  users: UserMentionCandidate[],
  query: string,
): MentionCandidate[];

/** Existing resolver + user handles -> {user: bytes} AuthorRefs. Bytes from
 *  hex via chat-client's keyBytes. */
export const mentionResolverOf = (
  agents: AgentRecord[],
  users?: UserMentionCandidate[],
): Map<string, AuthorRef>;
```
Notes: `mentionResolverOf` keeps its current 1-arg behavior (agents only) so existing call sites compile until Task 6 threads users through. `insertMention`/`mentionTokenAt`/`hasAgentMention` are untouched. `hasAgentMention` must stay AGENT-only (it gates the runs auto-watch — user mentions must NOT create watches).

**Tests first** (extend `mention.test.ts`, vitest):
1. `mentionableUsers` dedupes two nodes of the same user to one candidate; name `"Jess K"` → handle `"jess-k"`; null name → handle = first 8 hex chars; handle collision with an agent_id `"jess-k"` → `"jess-k-2"`.
2. `mentionResolverOf(agents, users)` maps `"jess-k"` → `{user: keyBytes(userKeyHex)}` and still maps active agents to `{agent:{module:"runs",...}}`; inactive agents excluded as today.
3. `mentionCandidatesAll` with query `"je"` ranks prefix matches first; empty query returns agents then users.
4. Existing tests untouched and green; `hasAgentMention` over blocks containing ONLY a user mention → false (new regression test).
5. Round-trip: `parseMessageInput("hi @jess-k", mentionResolverOf([], users))` (import from chat-input.ts) produces a span `{text:"@jess-k", marks:[{mention:{user:[...]}}]}` — proving the handle charset fits `MENTION_TOKEN` (`/@([a-z0-9._-]+)/`).

**Acceptance:** `cd app && bunx vitest run src/console/views/chat/mention.test.ts src/console/views/chat/chat-input.test.ts` green; no changes to chat-input.ts needed (charset already fits); no store/DucktapeProvider edits.
**Depends on:** —

---

### Task 6: user-mention wiring — MentionMenu, Composer, send path

**Phase:** A
**Goal:** The typeahead lists users alongside agents and the send path resolves user @tokens into `Mention(User)` marks.
**Files:**
- modify `app/src/console/views/chat/MentionMenu.tsx` (+ `MentionMenu.test.tsx`)
- modify `app/src/console/views/chat/Composer.tsx`
- modify `app/src/console/store/actions.ts`

**Interfaces:**
- `MentionMenu` props become `{ candidates: MentionCandidate[]; activeIndex: number; onPick: (handle: string) => void }` — a user row renders `label` + `@handle` (same visual idiom as agent rows; `aria-label` stays "Mention an agent or user"→ update to `"Mention"` or similar); `key` = agent_id / userKeyHex.
- `Composer.tsx`: derive `users = mentionableUsers(store?.state.nodeUsers ?? {}, agents)` (memoized), `menuCandidates = mentionCandidatesAll(agents, users, query)`; `pickMention` takes the candidate's insertable token (`agent.agent_id` or `handle`). Keyboard nav/Escape logic unchanged.
- `actions.ts`: the three `parseMessageInput(body, mentionResolverOf(getState().agents))` call sites (~1127 `sendMessage`, ~1169 `replyInThread`, ~1221 edit path) become `mentionResolverOf(getState().agents, mentionableUsers(getState().nodeUsers, getState().agents))`. `ensureMentionWatch` is untouched (agent-gated by `hasAgentMention`).
- `MessageItem.tsx` needs NO change — it already renders any mention mark via `authorName(mentionMark.mention, names)` (line ~194), and `authorNames` resolves user keys.

**Tests first:**
1. `MentionMenu.test.tsx`: renders a mixed candidate list; clicking (mousedown) a user row calls `onPick("jess-k")`; `aria-selected` follows activeIndex across the mixed list.
2. New/extended `ChatView`-level or Composer-level test if the existing harness supports it (follow `ChatView.test.tsx` patterns): typing `@je` opens the menu containing the user; Enter inserts `@jess-k ` into the draft.
3. Existing `mention`/`chat-input`/`MentionMenu`/`ChatView` suites stay green.

**Acceptance:** `cd app && bun run test` green for the chat view + store suites; a manual grep shows `mentionResolverOf(` in actions.ts passes users at all three send/edit sites; no watch is created for a user-only mention (assert via existing `ensureMentionWatch` behavior test if present, else add one).
**Depends on:** 5

---

### Task 7: notify prefs store + `notify-client.ts`

**Phase:** A
**Goal:** Local notification prefs (localStorage, the accent pattern) in the console store, plus the typed shell-IPC client whose payload shape is fixed now for Phase B.
**Files:**
- create `app/src/domain/notify-client.ts` (+ `app/src/domain/notify-client.test.ts`)
- modify `app/src/console/store/state.ts`
- modify `app/src/console/store/actions.ts`

**Interfaces:**
```ts
// state.ts — next to the accent block (~318)
export interface NotifyPrefs {
  enabled: boolean; mentions: boolean; replies: boolean; huddles: boolean;
  runs: boolean; forge: boolean; governance: boolean; mutedChannels: string[];
}
export const DEFAULT_NOTIFY_PREFS: NotifyPrefs; // all true, mutedChannels []
// key "ducktape.notifyPrefs"; loadNotifyPrefs validates field-by-field
// (unknown/missing -> default) so a corrupt blob can't poison the store.
export const loadNotifyPrefs = (): NotifyPrefs;
export const saveNotifyPrefs = (prefs: NotifyPrefs): void;
// ConsoleState gains: notifyPrefs: NotifyPrefs;  (createInitialState: loadNotifyPrefs())

// actions.ts — ConsoleActions gains:
setNotifyPrefs(prefs: NotifyPrefs): void;          // saveNotifyPrefs + patch, the setAccent pattern (~1046)
toggleChannelMute(channelId: string): void;        // add/remove in mutedChannels via setNotifyPrefs

// notify-client.ts — the shell IPC surface (window.__TAURI__ present only on desktop)
export interface NotifyConfigPayload {
  nodeUrl: string | null;
  selfUserKeyHex: string | null;
  selfNodeKeysHex: string[];
  focusedChannel: string | null;
  mainWindowFocused: boolean;
  authorNames: Record<string, string>;
  prefs: NotifyPrefs;
}
/** invoke("notify_configure", { config }) — swallows "command not found"
 *  (the Rust consumer lands in Phase B) and non-tauri environments. */
export const configure = (config: NotifyConfigPayload): Promise<void>;
/** invoke("notify_mark_seen") — same tolerance. */
export const markSeen = (): Promise<void>;
/** listen("ducktape://notify-unread", ({unread}) => ...) — returns unlisten. */
export const onUnread = (cb: (unread: number) => void): Promise<() => void>;
```
Follow the dynamic-import idiom used elsewhere (`import("@tauri-apps/api/core")` / `("@tauri-apps/api/event")`) so web builds no-op.

**Tests first:**
1. `state.ts` additions: `loadNotifyPrefs` returns defaults on missing/corrupt storage; round-trips a saved value; a partial stored object (e.g. only `{enabled:false}`) fills the rest with defaults (follow existing load* test patterns if present, else add a small suite).
2. `notify-client.test.ts`: `configure` resolves without throwing when invoke rejects with command-not-found (mock the tauri module via vitest `vi.mock`); payload passed through verbatim; `onUnread` unwraps `event.payload.unread`.

**Acceptance:** `cd app && bun run test` green; `NotifyConfigPayload` field names match the Global Constraints payload EXACTLY (camelCase, `selfNodeKeysHex` an array); no DucktapeProvider/transport edits (that's Phase B).
**Depends on:** —

---

### Task 8: prefs UI — Notifications group in Settings

**Phase:** A
**Goal:** The user-facing toggles: master + per-category + mute-current-channel, persisted via Task 7.
**Files:**
- modify `app/src/console/views/settings/PreferencesSection.tsx` (+ its test file if one exists, else create `PreferencesSection.test.tsx`)

**Interfaces:** presentation only, composed from the existing `SectionLabel`/`GroupCard`/`ControlRow` primitives in `views/settings/parts.tsx`. Add below the accent card:
- `SectionLabel` "NOTIFICATIONS", one `GroupCard` with `ControlRow`s: "Enable notifications" (master), then "Mentions & replies"? — NO: keep 1:1 with prefs fields: "Mentions", "Replies", "Huddles", "Agent runs", "Forge", "Governance", each a small toggle switch (build a local `Toggle` button component in-file, styled like the accent buttons — `aria-checked`, `role="switch"`, `aria-label` "Toggle <name> notifications"); category rows disabled (opacity .55, no-op) while master is off.
- "Mute current channel" row: shows `state.activeChannel` (hidden/disabled when null); a button toggling membership via `actions.toggleChannelMute(activeChannel)`; when muted the label reads "Unmute #chan".
- All handlers go through `actions.setNotifyPrefs` / `actions.toggleChannelMute`; render state from `state.notifyPrefs`.

**Tests first** (vitest + testing-library, mirroring existing settings tests — check `views/settings/*.test.tsx` for the harness/provider idiom):
1. Master toggle flips `notifyPrefs.enabled` (assert action called / state patched through a store harness).
2. Category toggle flips only its field; disabled while master off.
3. Mute row toggles `mutedChannels` for the active channel and reflects muted state.

**Acceptance:** `cd app && bun run test` green; visual idiom matches the existing PREFERENCES card (reviewer eyeballs the JSX against `parts.tsx` usage); no new theme tokens.
**Depends on:** 7

---

### Task 9: `notify/stream.rs` + `notify/http.rs` — the WS client of the typed protocol  ⛔ BLOCKED-ON-STREAM

**Phase:** B — do not start until the typed `/v1/ws` contract is merged to `dev` and this branch is rebased onto it. FIRST ACTION: read the landed stream spec (`docs/superpowers/specs/2026-07-10-live-stream-ws-design.md` on dev) and the `noded` stream code, and reconcile every frame/field name below against it — the wire shapes here are the plan's expectation, the landed code is authoritative.
**Goal:** Connect to `ws://<node>/v1/ws`, subscribe `module:chat`, `module:runs`, `module:forge`, `module:governance`, map wire frames → `engine::Frame`, implement the resume policy and the heartbeat watchdog, and provide the reply-root resolver.
**Files:**
- create `app/src-tauri/src/notify/stream.rs`
- create `app/src-tauri/src/notify/http.rs`
- modify `app/src-tauri/Cargo.toml` — add `tokio = { workspace = true }`, `tokio-tungstenite` (current 0.2x, default features, NO TLS features — localhost `ws://` only), `futures-util` (workspace version if present, else minimal)
- modify `app/src-tauri/src/notify/mod.rs` — `pub mod http; pub mod stream;`

**Interfaces:**
```rust
// http.rs — localhost-only HTTP/1.1 POST, no external HTTP crate (dep hygiene).
/// POST `body` as application/json to `{base_url}{path}`, return the response
/// body on 200. Blocking std::net::TcpStream; honors Content-Length; 2s timeouts.
pub fn post_json(base_url: &str, path: &str, body: &serde_json::Value)
    -> Result<serde_json::Value, String>;

/// The matcher's root_author impl: chat messages_range{root,1} -> the root
/// author's origin hex. Returns None on any failure (never blocks retry loops).
/// AuthorRef::User bytes -> lowercase hex; agent/module/system roots -> None.
pub fn root_author(base_url: &str, channel_id: &str, root_seq: u64) -> Option<String>;

// stream.rs
pub struct StreamHandle { /* JoinHandle + shutdown Notify */ }

/// Spawn the notifier loop on tauri::async_runtime. `shared` carries the
/// webview-pushed NotifyConfig + a change Notify (defined in Task 10's mod.rs).
pub fn spawn<S: engine::Sink + 'static>(
    shared: std::sync::Arc<super::Shared>,
    engine: engine::Engine<S>,
) -> StreamHandle;
```
Loop semantics (from the design, reconcile with the landed contract):
- Wait until `shared` has a `node_url`; connect `ws://…/v1/ws` (rewrite `http://` → `ws://`).
- **App-start subscribe: NO `resume` cursor** (live-from-tip — the coordination guarantee). **Transient in-session reconnect: resume from `engine.cursors()`** for topics that have one. **`lagged`: adopt** (engine already does).
- Map wire `event`/`lagged` frames to `Frame::{Event,Lagged}`; heartbeat frames feed a watchdog: no frame for ~2.5× the advertised `intervalMs` (fallback 3000ms × 2.5) → drop the socket and reconnect (with resume). Reconnect backoff: 1s doubling to 30s cap, reset on a successful frame.
- On each `Frame`, snapshot the config under the lock, call `engine.handle(frame, &config, &|ch, root| http::root_author(&url, ch, root))`. `root_author` is a rare blocking localhost call inside the async task — acceptable; do NOT hold the config lock across it (snapshot first).
- `node_url` change or shutdown Notify → drop the connection; a NEW url means app-level reconnect: treat it like app start (no resume — different node, cursors invalid; call `Engine::reset_cursors()`, defined in Task 3).
- Cursors are never read from disk (pre-flight decision 1): the ONLY resume source is the in-memory `engine.cursors()` from the current session.

**Tests first:**
1. `http.rs`: an in-file test spins a `std::net::TcpListener` stub returning a canned HTTP/1.1 200 with Content-Length + the `{"messages":[{...MessageView json with head.author {"user":[18,52]}...}]}` body → `root_author` returns `Some("1234")`; author `{"agent":{...}}` → None; connection refused → None (fast).
2. `stream.rs`: keep the frame-mapping fn pure and unit-test it: wire-frame JSON → `Frame` (event/lagged/heartbeat/unknown-ignored) — exact JSON per the landed contract.
3. The reconnect/resume policy: factor `fn subscribe_frames(topics, resume: Option<&BTreeMap<..>>) -> Vec<Message>` pure and assert app-start emits no resume, reconnect emits stored cursors.
4. Live check (manual, part of acceptance): run a standalone `ducktape-node` from a COPY outside `target/` (see the "tauri dev truncates the node binary" gotcha), point the loop at it, `POST /v1/submit` a `post_message` mention op, observe the capture-sink notification (wire a tiny `#[ignore]`d integration test or a dev binary — implementer's choice, document the command).

**Acceptance:** clippy gate green; unit tests green; the `#[ignore]`d live test (or documented manual drive) demonstrated once against a real node with the landed protocol; no module-crate deps added.
**Depends on:** 3 (engine), stream-on-dev

---

### Task 10: `notify/mod.rs` wiring — task spawn, `notify_configure` / `notify_mark_seen`, tray structured navigate  ⛔ BLOCKED-ON-STREAM

**Phase:** B
**Goal:** Own the notifier lifecycle from `setup()`, expose the two commands, track native main-window focus as the suppression backstop, and let Rust emit structured navigate targets.
**Files:**
- modify `app/src-tauri/src/notify/mod.rs`
- modify `app/src-tauri/src/main.rs`
- modify `app/src-tauri/src/tray.rs`

**Interfaces:**
```rust
// mod.rs
pub struct Shared {
    pub config: std::sync::Mutex<NotifyConfig>,
    /// woken on any config change (stream.rs re-reads; url change reconnects).
    pub changed: tokio::sync::Notify,
}
/// Engine commands crossing from the command handlers to the stream task.
pub enum Cmd { MarkSeen }

/// Build Shared + Engine(AppSink) with state at
/// app.path().app_data_dir()?/notify/state.json, spawn stream::spawn, manage
/// Shared (+ a Cmd sender) in tauri State. Attach a main-window
/// WindowEvent::Focused handler: focused=true -> send Cmd::MarkSeen AND set
/// config.main_window_focused; focused=false -> clear it. Call from setup()
/// AFTER tray::init.
pub fn init<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()>;

#[tauri::command]
pub fn notify_configure<R: tauri::Runtime>(
    state: tauri::State<'_, std::sync::Arc<Shared>>, config: NotifyConfig,
) -> Result<(), String>;   // replace config, notify changed

#[tauri::command]
pub fn notify_mark_seen(state: tauri::State<'_, /* Cmd sender wrapper */>) -> Result<(), String>;
```
- `main.rs`: `notify::init(app.handle())?` in `setup` after `tray::init`/`menu::install`; add `notify::notify_configure, notify::notify_mark_seen` to `generate_handler!`.
- Since the engine lives inside the stream task, `Cmd::MarkSeen` rides a `tokio::sync::mpsc::UnboundedSender<Cmd>` stored in State; the stream loop `select!`s frames, config-change notifies, and Cmds.
- `tray.rs`: extend `OpenConsole` to `{ screen: Option<String>, target: Option<serde_json::Value> }` (camelCase); when `target` is Some emit it verbatim as the `"ducktape://navigate"` payload, else keep today's plain-string screen emit — both shapes stay legal (the Task 11 listener accepts both). Keep `show_main` reuse.

**Tests first:** config replacement + changed-notify observable via a unit test on `Shared` (lock, replace, `notified()` fires); `OpenConsole` deserialization of both `{"screen":"chat"}` and `{"target":{"screen":"chat","channelId":"general"}}`. Lifecycle (spawn/focus events) is compile+manual — covered by Task 12's app drive.
**Acceptance:** clippy gate green; `generate_handler!` lists both commands; `notify::init` called in setup; a `cargo build -p ducktape-desktop` (debug) succeeds; existing tray behavior (popover toggle, close-to-tray) untouched by inspection.
**Depends on:** 4, 9

---

### Task 11: webview integration — config push + structured deep-link (+ forge focus)  ⛔ BLOCKED-ON-STREAM

**Phase:** B (touches DucktapeProvider regions the stream agent owns — rebase first)
**Goal:** The webview pushes `notify_configure` whenever its inputs change, and the `ducktape://navigate` listener accepts structured targets that drive chat/thread/forge/members navigation.
**Files:**
- modify `app/src/console/store/DucktapeProvider.tsx`
- modify `app/src/console/store/state.ts` (one field)
- modify `app/src/console/views/forge/ForgeView.tsx`
- (uses `app/src/domain/notify-client.ts` from Task 7)

**Interfaces / behavior:**
- **Config push effect** (new, alongside effect 5): compute and push `NotifyConfigPayload` via `notifyClient.configure` when any dependency changes: `nodeUrl: state.nodeUrl`; `selfNodeKeyHex = state.status?.publicKey?.toLowerCase()`; `selfUserKeyHex = state.nodeUsers[selfNodeKeyHex]?.userKey ?? null`; `selfNodeKeysHex` = all `nodeHex` keys of `state.nodeUsers` whose `userKey === selfUserKeyHex`, else `[selfNodeKeyHex]`; `focusedChannel: state.screen === "chat" ? state.activeChannel : null`; `mainWindowFocused` from `document.hasFocus()` tracked via window `"focus"`/`"blur"` listeners (a small `useState`); `authorNames: state.authorNames`; `prefs: state.notifyPrefs`. Fingerprint-dedupe pushes with a ref (the huddle-context idiom, ~511). Desktop only (`isTauri()`).
- **Deep-link listener** (extend effect 5, ~606): payload `string | NavigateTarget`. String → today's behavior. Object → `actions.setScreen(target.screen)`; then `if (target.channelId) actions.selectChannel(target.channelId)`; `if (target.threadRoot != null) actions.openThread(target.threadRoot)` (verify `selectChannel`/`enterChannel` patches `activeChannel` synchronously before `openThread` reads it — sequence with `.then` if not); `if (target.repo || target.number != null)` patch `forgeFocus`.
- **Forge handoff:** `state.ts` gains `forgeFocus: { repo: string; number: number } | null` (initial null) — the `explorerFocus` idiom (state.ts:270, consumed ExplorerView.tsx:361). `ForgeView.tsx` consumes it: on presence, set `selectedRepoId = repo`, switch to the items tab, open item `number` (read ForgeView/items internals for the exact setters), then clear via a patch action. Keep the consumption effect small and tolerant (unknown repo/number → just select the repo/tab and clear).
- Also call `notifyClient.markSeen()` on window focus (webview-side complement to the native backstop) and wire `notifyClient.onUnread` into a small store field ONLY if trivially useful — otherwise skip; the badge is native-first (don't grow scope).

**Tests first:**
1. `DucktapeProvider.test.tsx` (follow existing patterns): with a mocked notify-client, mounting with a transport pushes a configure payload carrying `selfUserKeyHex` derived from `nodeUsers[status.publicKey]`; changing activeChannel re-pushes with the new `focusedChannel`; identical state does not re-push (dedupe).
2. Navigate: emitting a structured `{screen:"chat",channelId:"general",threadRoot:7}` through the mocked event layer patches screen, selects the channel, opens the thread; a plain `"members"` string still patches screen (regression).
3. Forge: `{screen:"forge",repo:"default",number:7}` sets `forgeFocus`; a ForgeView test (mirroring `ForgeView.test.tsx` harness) consumes and clears it.

**Acceptance:** `cd app && bun run test` green including provider + forge suites; effects it adds don't touch the block-tick region (post-rebase file); config payload matches Task 7's `NotifyConfigPayload` exactly.
**Depends on:** 7, 10, stream-on-dev (rebased provider)

---

### Task 12: end-to-end verification + PR  ⛔ BLOCKED-ON-STREAM

**Phase:** B
**Goal:** Prove the whole path live, run every gate, and open the PR against `dev`.
**Files:** none beyond incidental fixes (each fix goes through review like any task).

**Steps:**
1. **Gates:** `cargo clippy -p ducktape-desktop --tests --no-deps` clean; `cargo test -p ducktape-desktop` green; `cargo check -p files --no-default-features` green; `cd app && bun run test` green; `bunx tsc --noEmit` if the repo runs it (check package.json scripts).
2. **Injected-frame integration** (already automated in Tasks 3/9): re-run and cite.
3. **Live node e2e:** standalone `ducktape-node` from a copy outside `target/` (gotcha: `tauri dev` truncates `target/debug/ducktape-node`); subscribe; `POST /v1/submit` a `post_message` carrying `{"mention":{"user":[...]}}` for a configured self key from a second origin; assert a presented notification + badge=1; submit `join_huddle` from a foreign node key → huddle notification; `merge_pr`, admission `propose` likewise. Assert live-from-tip: restart the notifier and confirm pre-existing history does NOT re-notify.
4. **App drive** (the `tauri-debug`/`qa` skill, headless Xvfb): unlock identity, create a workspace; use `tauri_ipc` to observe `notify_configure` firing on mount, channel switch, prefs toggle, focus change; toggle Notifications prefs in Settings and confirm the pushed payload changes; type `@` in the composer and pick a user; post from a second origin and confirm the `ducktape://notify-unread` event + (if a libnotify daemon is present) the Linux toast. macOS dock badge + Notification Center toast are verified by the user on a Mac — state this explicitly in the PR.
5. **PR:** open against `dev` titled for the notifier; body lists the verification evidence, the Design-concerns deviations (§2), and the explicit non-goals (no while-quit delivery, no forge review-requests, no backfill). Do NOT merge without the clean-context review; note any skipped check honestly.

**Acceptance:** PR open with all evidence; no gate red; deviations documented.
**Depends on:** all previous

---

## 5. Verification (rollup)

| Check | Where | Gated on stream? |
|---|---|---|
| decode/matcher/engine/state unit tests (mention/reply/huddle-dedupe/run/forge/gov, prefs+focus suppression, lagged adoption, no-replay, persistence round-trip) | Tasks 1–3, `cargo test -p ducktape-desktop notify::` | No |
| Injected-frame integration (fake frame channel + capture sink → presented set + cursors) | Task 3 tests; extended in Task 9 | No (frame enum is internal) |
| `cargo clippy -p ducktape-desktop --tests --no-deps` | every Rust task | No |
| plugin + capability compile (`cargo check -p ducktape-desktop`) | Task 4 | No |
| user-mention + prefs + notify-client vitest suites | Tasks 5–8, `cd app && bun run test` | No |
| wire-frame mapping + subscribe/resume-policy units; live `#[ignore]` node test | Task 9 | **Yes** |
| provider config-push / structured-navigate / forgeFocus suites | Task 11 | **Yes** (rebased provider) |
| live e2e (submit op → toast/badge; live-from-tip on restart) + headless app drive; macOS toast/badge verified by the user on a Mac | Task 12 | **Yes** |
| `cargo check -p files --no-default-features` (untouched, confirm) | Task 12 | No |
