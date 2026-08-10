# Live Sync Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the node's live head and its sync state on screen in three places — the Explorer header, the titlebar status card, and Settings → node overview.

**Architecture:** No node change. The `status` snapshot topic added in #1033 already pushes the whole `NodeStatus`, including the `operations` projection with `phase` and `sync`. The work is: split that topic's subscription away from `peers` so it can be held on every tab, widen the one shared reader that both the HTTP load and the pushed frame already use, and render.

**Tech Stack:** Rust (`crates/rpc-client`, `app/src/backend`), Ice (`app/src/ui`).

## Global Constraints

- A reading the node has not published is `UNMEASURED` (`-1`), never `0` — `app/src/backend/node.rs`. Renderers turn `< 0` into `—`.
- `operations.sync` is **never cleared** once set (`bin/noded/src/metrics.rs`, `begin_sync` / `record_sync_progress`). Presence of `sync` must never be the test for "is syncing".
- `retries` / `failures` are cumulative counters that never reset. `last_error` IS self-clearing (`record_sync_progress` sets it to `None` on progress).
- Ice: a `parallel` block must be the final statement in a handler (E141). The app has exactly ONE `subscribe` block, in `handlers/lifecycle.ice`.
- Gates: `cargo test -p ducktape-app`, `cargo test -p ducktape-rpc`, `cargo clippy -p <crate> --tests --no-deps`. Only format files you touched.
- Every guard added must be watched go red under a mutation before the task is done.

---

### Task 1: Split the snapshot stream in the rpc client

`node_snapshots` subscribes `peers` + `status` on one socket, so both share one subscription lifetime. `status` must outlive the overview tab; `peers` must not. Splitting also deletes the reason `NodeSnapshot` exists.

**Files:**
- Modify: `crates/rpc-client/src/lib.rs` (`NodeSnapshot`, `snapshot_from_frame`, `node_snapshots`)
- Test: `crates/rpc-client/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Produces: `Client::status_events(&self) -> Result<BoxStream<'static, Result<serde_json::Value>>>` — each item is one `NodeStatus` document.
- Produces: `Client::peers_events(&self) -> Result<BoxStream<'static, Result<serde_json::Value>>>` — each item is one `PeersView` document.
- Removes: `NodeSnapshot`, `node_snapshots`.

- [ ] **Step 1: Rewrite the frame router to take the topic it wants**

Replace `snapshot_from_frame` with a topic-parameterised version. Keep the empty-ack rule — it is the guard from #1033 that stops a console wedging silently against an older daemon.

```rust
/// Route one server frame for a single snapshot topic: the document, a
/// failure, or `None` for a frame this subscription does not consume.
///
/// ADMITTING NONE IS THE CONNECTION FAILING — the same rule
/// `module_event_stream` states below. A console pointed at a daemon that
/// predates this topic gets `subscribed: {}`; swallowing it wedges the pane in
/// total silence, because the node heartbeats every block and the idle timeout
/// never fires.
fn snapshot_from_frame(text: &str, want: &str) -> Option<Result<serde_json::Value>> {
    #[derive(Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum SnapshotFrame {
        Subscribed {
            topics: BTreeMap<String, String>,
        },
        Tail {
            topic: String,
            item: serde_json::Value,
        },
        #[serde(other)]
        Other,
    }
    match serde_json::from_str::<SnapshotFrame>(text).ok()? {
        SnapshotFrame::Subscribed { topics } if topics.is_empty() => {
            Some(Err(Error::new("RPC stream admitted no requested topic")))
        }
        SnapshotFrame::Subscribed { .. } | SnapshotFrame::Other => None,
        // `get`, not `item["…"]`: indexing a `Value` with a &str PANICS on a
        // non-object, and a missing key would otherwise read as an empty
        // sample rather than being ignored.
        SnapshotFrame::Tail { topic, item } if topic == want => {
            Some(Ok(item.get(want)?.clone()))
        }
        SnapshotFrame::Tail { .. } => None,
    }
}
```

- [ ] **Step 2: Replace `node_snapshots` with one private opener plus two thin wrappers**

```rust
    /// Subscribe ONE snapshot topic on its own socket.
    ///
    /// One topic per socket is what lets the console hold `status` on every
    /// tab while `peers` lives only on the tab that draws it: the subscription
    /// IS the node's sampling budget, and `peers` composes its document by
    /// encoding the whole metrics registry where `status` is a cell read.
    async fn snapshot_events(
        &self,
        topic: &'static str,
    ) -> Result<futures::stream::BoxStream<'static, Result<serde_json::Value>>> {
        let subscribe = serde_json::to_string(&SubscribeRequest {
            op: "subscribe",
            topics: vec![topic.to_string()],
            resume: BTreeMap::new(),
        })
        .map_err(|error| Error::new(format!("could not encode snapshot subscription: {error}")))?;
        let url = self.stream_url()?;
        let (mut socket, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::connect_async(&url))
            .await
            .map_err(|_| Error::new("RPC stream connection timed out"))?
            .map_err(|error| Error::new(format!("RPC stream connection failed: {error}")))?;
        tokio::time::timeout(TIMEOUT, socket.send(Message::Text(subscribe)))
            .await
            .map_err(|_| Error::new("RPC stream subscription timed out"))?
            .map_err(|error| Error::new(format!("RPC stream subscription failed: {error}")))?;
        let stream = futures::stream::unfold(Some(socket), move |socket| async move {
            let mut socket = socket?;
            loop {
                let message = match tokio::time::timeout(STREAM_IDLE_TIMEOUT, socket.next()).await {
                    Ok(Some(message)) => message,
                    Ok(None) => return Some((Err(Error::new("RPC stream closed")), None)),
                    Err(_) => {
                        return Some((Err(Error::new("RPC stream heartbeat timed out")), None));
                    }
                };
                let Ok(Message::Text(text)) = message else {
                    if matches!(message, Ok(Message::Close(_)) | Err(_)) {
                        return Some((Err(Error::new("RPC stream closed")), None));
                    }
                    continue;
                };
                match snapshot_from_frame(&text, topic) {
                    Some(Ok(document)) => return Some((Ok(document), Some(socket))),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => continue,
                }
            }
        })
        .boxed();
        Ok(stream)
    }

    /// The node's own status projection, pushed. Cheap enough to hold open
    /// wherever the console is: the node answers it from a published cell.
    pub async fn status_events(
        &self,
    ) -> Result<futures::stream::BoxStream<'static, Result<serde_json::Value>>> {
        self.snapshot_events("status").await
    }

    /// The direct-peer sample, pushed. EXPENSIVE — every sample encodes the
    /// node's whole metrics registry — so hold it only while a surface draws
    /// it.
    pub async fn peers_events(
        &self,
    ) -> Result<futures::stream::BoxStream<'static, Result<serde_json::Value>>> {
        self.snapshot_events("peers").await
    }
```

Delete `pub enum NodeSnapshot` and its doc comment.

- [ ] **Step 3: Update the two existing tests to the new shape**

Replace `an_empty_subscribe_ack_fails_the_snapshot_stream` and `snapshot_frames_route_by_topic_and_survive_a_malformed_item` bodies to pass a `want`:

```rust
    #[test]
    fn an_empty_subscribe_ack_fails_the_snapshot_stream() {
        let refused = snapshot_from_frame(r#"{"type":"subscribed","topics":{}}"#, "peers");
        assert!(
            matches!(refused, Some(Err(_))),
            "admitting no topic is the connection failing"
        );
        let partial = snapshot_from_frame(r#"{"type":"subscribed","topics":{"peers":"0"}}"#, "peers");
        assert!(
            partial.is_none(),
            "an admitted subscribe keeps the stream: {partial:?}"
        );
    }

    /// ONE SOCKET, ONE TOPIC. A frame for a topic this subscription did not
    /// ask for must be ignored rather than read at the wrong key — that is the
    /// `files:watch` failure (subscribes cleanly, delivers nothing usable).
    #[test]
    fn snapshot_frames_route_by_topic_and_survive_a_malformed_item() {
        let peers = snapshot_from_frame(
            r#"{"type":"tail","topic":"peers","cursor":"1","item":{"time_ms":1,"peers":{"peers":[]}}}"#,
            "peers",
        );
        assert!(matches!(peers, Some(Ok(_))), "{peers:?}");

        let status = snapshot_from_frame(
            r#"{"type":"tail","topic":"status","cursor":"1","item":{"time_ms":1,"status":{"version":"0.1.0"}}}"#,
            "status",
        );
        assert!(matches!(status, Some(Ok(_))), "{status:?}");

        // a frame for the OTHER topic on this socket is not ours.
        assert!(
            snapshot_from_frame(
                r#"{"type":"tail","topic":"status","item":{"status":{}}}"#,
                "peers"
            )
            .is_none()
        );
        // a missing key is not an empty sample.
        assert!(
            snapshot_from_frame(r#"{"type":"tail","topic":"peers","item":{"time_ms":1}}"#, "peers")
                .is_none()
        );
        // and a non-object `item` must not panic the subscription task.
        assert!(snapshot_from_frame(r#"{"type":"tail","topic":"peers","item":42}"#, "peers").is_none());
        assert!(snapshot_from_frame(r#"{"type":"heartbeat","height":9}"#, "peers").is_none());
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ducktape-rpc`
Expected: PASS, 9 tests.

- [ ] **Step 5: Mutation check**

Delete the `topic == want` guard (make the arm accept any topic). Run `cargo test -p ducktape-rpc`.
Expected: FAIL on "a frame for the OTHER topic on this socket is not ours".
Restore.

- [ ] **Step 6: Commit**

```bash
git add crates/rpc-client/src/lib.rs
git commit -m "refactor(rpc): one snapshot topic per socket, so their lifetimes can differ"
```

---

### Task 2: Carry phase and sync in the shared reader

`node_facts` is the ONE reader both the HTTP load and the pushed frame use (#1033). Widening it there covers both paths.

**Files:**
- Modify: `app/src/backend/node.rs` (`NodeFacts`, `impl Default`, `node_facts`)
- Test: `app/src/backend/tests.rs`

**Interfaces:**
- Produces: `NodeFacts` gains `phase: String`, `phase_since: i64`, `sync_target: i64`, `sync_applied: i64`, `sync_retries: i64`, `sync_failures: i64`, `sync_last_error: String`.
- Produces: `pub(crate) fn sync_in_progress(phase: &str) -> bool`.

- [ ] **Step 1: Write the failing test**

Add to `app/src/backend/tests.rs`, beside `a_status_without_operations_produces_unmeasured_readings`:

```rust
/// SYNC IS READ OFF `phase`, NEVER OFF THE PRESENCE OF `sync`.
///
/// `operations.sync` is set by `begin_sync` and never cleared — no writer in
/// `bin/noded/src/metrics.rs` puts it back to `None`. A node that finished
/// syncing hours ago still carries the last run's heights. Reading presence as
/// "is syncing" paints a progress bar that never goes away.
#[test]
fn a_finished_sync_is_not_a_sync_in_progress() {
    let caught_up = serde_json::json!({
        "operations": {
            "phase": "serving",
            "phase_since": 1_700_000_000,
            "sync": { "target_height": 900, "applied_height": 900, "retries": 3, "failures": 1 },
        },
    });
    let facts = node_facts(&caught_up, 0);
    assert_eq!(facts.phase, "serving");
    assert!(
        !sync_in_progress(&facts.phase),
        "a stale sync block must not read as a sync in progress"
    );
    // the numbers still ride, because Settings prints them as cumulative
    assert_eq!(facts.sync_retries, 3);
    assert_eq!(facts.sync_failures, 1);

    let catching_up = serde_json::json!({
        "operations": {
            "phase": "syncing",
            "sync": { "target_height": 900, "applied_height": 412 },
        },
    });
    let facts = node_facts(&catching_up, 0);
    assert!(sync_in_progress(&facts.phase));
    assert_eq!(facts.sync_applied, 412);
    assert_eq!(facts.sync_target, 900);

    // a node that has never synced publishes no `sync` at all: heights are
    // UNMEASURED, counters are genuinely zero, and the error is absent.
    let fresh = serde_json::json!({ "operations": { "phase": "validating" } });
    let facts = node_facts(&fresh, 0);
    assert_eq!(facts.sync_target, UNMEASURED);
    assert_eq!(facts.sync_applied, UNMEASURED);
    assert_eq!(facts.sync_retries, 0);
    assert_eq!(facts.sync_last_error, "");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p ducktape-app a_finished_sync_is_not_a_sync_in_progress`
Expected: FAIL to compile — `phase` not a field of `NodeFacts`.

- [ ] **Step 3: Widen the struct, the Default and the reader**

In `app/src/backend/node.rs`, add to `pub struct NodeFacts` (after `height`):

```rust
    /// The node's own lifecycle phase — `starting`, `recovering`, `joining`,
    /// `syncing`, `validating`, `serving`, `draining`, `halted`. THE ONLY
    /// TRUSTWORTHY DISCRIMINANT for whether a sync is happening: the `sync`
    /// block beside it is written by `begin_sync` and never cleared.
    pub phase: String,
    /// Unix seconds the phase last changed; `UNMEASURED` when unpublished.
    pub phase_since: i64,
    /// The sync run's heights, `UNMEASURED` when the node has published none.
    pub sync_target: i64,
    pub sync_applied: i64,
    /// CUMULATIVE since boot and never reset, so these are a total, not a
    /// state. Absence really is zero — a count of nothing.
    pub sync_retries: i64,
    pub sync_failures: i64,
    /// The last sync error, SELF-CLEARING: `record_sync_progress` sets it back
    /// to `None` the moment the node makes progress. Present therefore means
    /// "the most recent attempt failed and nothing has advanced since", which
    /// is a fact about now.
    pub sync_last_error: String,
```

Extend `impl Default for NodeFacts` with `phase: String::new(), phase_since: UNMEASURED, sync_target: UNMEASURED, sync_applied: UNMEASURED, sync_retries: 0, sync_failures: 0, sync_last_error: String::new(),`.

Extend `node_facts` (after `height`):

```rust
        phase: operations["phase"].as_str().unwrap_or_default().to_string(),
        phase_since: operations["phase_since"].as_i64().unwrap_or(UNMEASURED),
        sync_target: sync["target_height"].as_i64().unwrap_or(UNMEASURED),
        sync_applied: sync["applied_height"].as_i64().unwrap_or(UNMEASURED),
        sync_retries: sync["retries"].as_i64().unwrap_or(0),
        sync_failures: sync["failures"].as_i64().unwrap_or(0),
        sync_last_error: sync["last_error"].as_str().unwrap_or_default().to_string(),
```

with `let sync = &operations["sync"];` beside the existing `let consensus = ...`.

Add beside it:

```rust
/// Whether the node is catching up RIGHT NOW.
///
/// The phase, and only the phase. `operations.sync` is never cleared, so its
/// presence says a sync once happened, not that one is happening.
pub(crate) fn sync_in_progress(phase: &str) -> bool {
    phase == "syncing"
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p ducktape-app a_finished_sync_is_not_a_sync_in_progress`
Expected: PASS.

- [ ] **Step 5: Mutation check**

Change `sync_in_progress` to `!phase.is_empty() && phase != "serving"`. Run the test.
Expected: still passes — so ALSO change it to test presence: `pub(crate) fn sync_in_progress(_phase: &str) -> bool { true }`.
Expected: FAIL on "a stale sync block must not read as a sync in progress". Restore.

- [ ] **Step 6: Commit**

```bash
git add app/src/backend/node.rs app/src/backend/tests.rs
git commit -m "feat(app): carry the node's phase and sync run in the shared status reader"
```

---

### Task 3: Two streams with two different gates

**Files:**
- Modify: `app/src/backend/node.rs` (`node_overview` → `node_status_live` + `node_peers_live`; delete `NodeOverview`, `overview_from`)
- Modify: `app/src/ui/extern/backend.ice`
- Modify: `app/src/ui/handlers/lifecycle.ice` (the subscribe block)
- Modify: `app/src/ui/handlers/node.ice` (`node_overview_sample` → two handlers)
- Modify: `app/src/ui/state.ice` (new fields)
- Test: `app/src/tests.rs`

**Interfaces:**
- Produces: `pub fn node_status_live(rpc: String) -> BoxStream<'static, NodeFacts>`
- Produces: `pub fn node_peers_live(rpc: String) -> BoxStream<'static, PeersData>`
- Consumes: `Client::status_events` / `Client::peers_events` from Task 1; `node_facts` from Task 2.

- [ ] **Step 1: Replace the merged stream with two**

In `app/src/backend/node.rs`, delete `pub struct NodeOverview` and `fn overview_from`, and replace `node_overview` with two functions built on the same unfold shape already there. Each yields its own already-parsed payload, so the `answered` flags disappear — they existed only because one stream carried two things.

```rust
/// THE NODE'S OWN STATUS, PUSHED, ON EVERY TAB.
///
/// Cheap to hold: the node answers `status` from a cell it publishes at each
/// boundary, and #1042's debounce means one read per heartbeat. That is what
/// lets the titlebar carry a sync reading wherever the reader is standing.
pub fn node_status_live(rpc: String) -> iced::futures::stream::BoxStream<'static, NodeFacts> {
    snapshot_stream(rpc, |client| Box::pin(async move { client.status_events().await }), |document| {
        // the generation is the HTTP loads' stale-reply guard; a PUSH answers
        // no request, so `-1` is the app's own "not a reply" reading.
        node_facts(&document, -1)
    })
}

/// THE DIRECT-PEER SAMPLE, PUSHED, ONLY WHERE IT IS DRAWN.
///
/// Every sample encodes the node's whole metrics registry, so the Ice `when`
/// gate on this subscription IS the budget — leaving the tab stops the encode
/// at the source.
pub fn node_peers_live(rpc: String) -> iced::futures::stream::BoxStream<'static, PeersData> {
    snapshot_stream(rpc, |client| Box::pin(async move { client.peers_events().await }), |document| {
        PeersData {
            generation: -1,
            peers: peer_rows(&document),
        }
    })
}
```

with one private helper carrying the reconnect-with-backoff loop that `node_overview` already has (same `retry_delay`, same "a dropped socket is not a reason to blank the pane" comment).

- [ ] **Step 2: Declare both in the extern file**

In `app/src/ui/extern/backend.ice`, replace the two `NodeOverview` lines with:

```
  stream node_status_live(rpc:str) -> NodeFacts
  stream node_peers_live(rpc:str) -> PeersData
```

and extend the `NodeFacts(...)` line with `, phase:str, phase_since:i64, sync_target:i64, sync_applied:i64, sync_retries:i64, sync_failures:i64, sync_last_error:str`.

- [ ] **Step 3: Two subscriptions with two gates**

In `app/src/ui/handlers/lifecycle.ice`, replace the single `run node_overview(...)` line with:

```
  // STATUS RIDES EVERYWHERE. The node answers it from a published cell, so a
  // console holding it on every tab costs one cell read per heartbeat — and
  // sync is a fact about the node, not about the tab you are standing on.
  run node_status_live(connected_rpc) when connected -> node_status_pushed _
  // PEERS DOES NOT. Each sample encodes the whole metrics registry, so this
  // gate is the budget: leaving the tab stops the encode at the source.
  run node_peers_live(connected_rpc) when (connected && shell_tab == "settings" && node_tab == "overview") -> node_peers_pushed _
```

- [ ] **Step 4: Split the handler**

In `app/src/ui/handlers/node.ice`, replace `on node_overview_sample(next)` with:

```
// A PUSHED status document. No generation guard, and that is not an omission:
// a generation retires a stale REPLY to a request this app made, and a push
// answers no request. The freshest sample wins, which is the node's own order.
on node_status_pushed(next)
  node_version = next.version
  node_root_hash = next.root_hash
  node_last_finalized = next.last_finalized_at
  node_checkpoint = next.checkpoint_height
  node_height = next.height
  node_view_label = optional_number(next.view)
  node_quorum_label = optional_number(next.quorum)
  node_reachable_label = optional_number(next.reachable_validators)
  node_phase = next.phase
  node_phase_since = next.phase_since
  node_sync_target = next.sync_target
  node_sync_applied = next.sync_applied
  node_sync_retries = next.sync_retries
  node_sync_failures = next.sync_failures
  node_sync_last_error = next.sync_last_error

on node_peers_pushed(next)
  node_peers = next.peers
```

Extend `on node_facts_loaded(next)` with the same seven new assignments, so the HTTP load and the push stay in step.

- [ ] **Step 5: Add the state**

In `app/src/ui/state.ice`, beside `node_checkpoint`:

```
  // The node's lifecycle phase and its sync run. `-1` is the sentinel for a
  // height the node has not published; the two counters are cumulative totals
  // where absence genuinely is zero.
  node_phase = ""
  node_phase_since:i64 = -1
  node_sync_target:i64 = -1
  node_sync_applied:i64 = -1
  node_sync_retries:i64 = 0
  node_sync_failures:i64 = 0
  node_sync_last_error = ""
```

- [ ] **Step 6: Run the suite and fix fallout**

Run: `cargo test -p ducktape-app`
Expected: the two #1033 tests referencing `NodeOverview` fail to compile. Rewrite `an_overview_frame_moves_only_the_half_it_answered` as `a_pushed_status_moves_the_facts_and_leaves_the_table` (dispatch `NodeStatusPushed` with a `NodeFacts`, assert the nine facts fields move and `node_peers` does not), and update `the_node_overview_stream_only_runs_on_the_tab_that_draws_it` to pin BOTH new `run` lines.

- [ ] **Step 7: Commit**

```bash
git add app/src/backend/node.rs app/src/ui
git commit -m "feat(app): status rides every tab, peers stays on the tab that draws it"
```

---

### Task 4: Render it in the three places

**Files:**
- Modify: `app/src/backend/node.rs` (one renderer)
- Modify: `app/src/ui/extern/backend.ice`
- Modify: `app/src/ui/screens/storage.ice` (`ExplorerScreen`)
- Modify: `app/src/ui/components/shell.ice` (`StatusCard`)
- Modify: `app/src/ui/screens/settings.ice` (node overview arm)
- Modify: `app/src/ui/view.ice` (three call sites)
- Test: `app/src/backend/tests.rs`

**Interfaces:**
- Produces: `pub fn sync_label(phase: String, applied: i64, target: i64) -> String`

- [ ] **Step 1: Write the failing test**

```rust
/// ONE STRING FOR ALL THREE SURFACES, so they cannot disagree about what the
/// node is doing.
#[test]
fn the_sync_label_shows_progress_only_while_catching_up() {
    assert_eq!(sync_label("syncing".into(), 412, 900), "Syncing 412 / 900");
    // caught up: the phase alone, however stale the numbers beside it are.
    assert_eq!(sync_label("serving".into(), 900, 900), "Serving");
    assert_eq!(sync_label("validating".into(), -1, -1), "Validating");
    // syncing with nothing published yet is still honest about the phase.
    assert_eq!(sync_label("syncing".into(), -1, -1), "Syncing");
    // a node that published no phase says nothing rather than guessing.
    assert_eq!(sync_label(String::new(), -1, -1), "");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p ducktape-app the_sync_label_shows_progress_only_while_catching_up`
Expected: FAIL — `sync_label` not found.

- [ ] **Step 3: Implement**

```rust
/// The one sentence all three surfaces print for what the node is doing.
///
/// Progress rides ONLY while `sync_in_progress`, because `operations.sync` is
/// never cleared — printing it whenever it exists leaves a finished run's
/// numbers on screen forever.
pub fn sync_label(phase: String, applied: i64, target: i64) -> String {
    if phase.is_empty() {
        return String::new();
    }
    let name = title_case_phase(&phase);
    let measured = applied >= 0 && target >= 0;
    if !sync_in_progress(&phase) || !measured {
        return name;
    }
    format!("{name} {} / {}", grouped_digits(applied), grouped_digits(target))
}

fn title_case_phase(phase: &str) -> String {
    let mut chars = phase.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p ducktape-app the_sync_label_shows_progress_only_while_catching_up`
Expected: PASS.

- [ ] **Step 5: Wire the three surfaces**

Declare `sync sync_label(phase:str, applied:i64, target:i64) -> str` in `app/src/ui/extern/backend.ice`.

1. `ExplorerScreen` (`app/src/ui/screens/storage.ice`): add `block_height:i64, sync:str` to the component signature and print `height_label(block_height)` beside `sync` in the header row that currently holds the title and the refresh button. `view.ice` passes `block_height=block_height` and `sync=sync_label(node_phase, node_sync_applied, node_sync_target)`.
2. `StatusCard` (`app/src/ui/components/shell.ice`): add `sync:str` and print it under the existing height row. `view.ice` passes the same expression through `WorkspaceTabs`.
3. Settings node overview (`app/src/ui/screens/settings.ice`): add a `StatCard` with `label="SYNC"`, `value=sync` and `note=relative_time(node_phase_since)`, plus a `KeyValueRow` for cumulative retries/failures (`label="Sync retries"`, `value=reading_pair(...)`, `note="cumulative"`), and a `KeyValueRow` for `node_sync_last_error` rendered only `if !empty(node_sync_last_error)`.

- [ ] **Step 6: Run the suite**

Run: `cargo test -p ducktape-app`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add app/src
git commit -m "feat(app): the head and the sync reading reach the three surfaces that need them"
```

---

### Task 5: Guards that fail

**Files:**
- Modify: `app/src/tests.rs`

- [ ] **Step 1: Pin both subscription gates as an exact set**

```rust
/// STATUS EVERYWHERE, PEERS ONLY WHERE IT IS DRAWN — and pinned as sets,
/// because a `contains` is satisfied by a commented-out line and equally by a
/// SECOND, wrongly-gated subscription beside the right one.
#[test]
fn the_node_streams_carry_the_gates_their_costs_require() {
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    let status: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("run node_status_live("))
        .collect();
    assert_eq!(
        status,
        ["run node_status_live(connected_rpc) when connected -> node_status_pushed _"],
        "status is a cell read and a fact about the node, so it rides every tab"
    );
    let peers: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("run node_peers_live("))
        .collect();
    assert_eq!(
        peers,
        ["run node_peers_live(connected_rpc) when (connected && shell_tab == \"settings\" && node_tab == \"overview\") -> node_peers_pushed _"],
        "every peers sample encodes the whole metrics registry; this gate is the budget"
    );
    assert_no_polling(&lifecycle);
}
```

- [ ] **Step 2: Pin that the explorer receives the live head**

```rust
/// The explorer used to be handed a 100-block snapshot and a refresh button,
/// with no live head at all — the number the reader is watching for.
#[test]
fn the_explorer_is_handed_the_live_head() {
    let (app, _) = Ducktape::__boot();
    assert_eq!(backend::height_label(app.block_height), "h —");
    let view = inlined(include_str!("ui/view.ice"));
    assert!(
        view.contains("block_height=block_height"),
        "the explorer must draw the live register, not its snapshot's newest row"
    );
}
```

- [ ] **Step 3: Run both, then mutate each**

Run: `cargo test -p ducktape-app`
Then: widen the peers gate to bare `connected` → expect FAIL on "this gate is the budget". Restore.
Then: narrow the status gate to the settings tab → expect FAIL on "rides every tab". Restore.

- [ ] **Step 4: Full gates**

```bash
cargo test -p ducktape-app
cargo test -p ducktape-rpc
cargo clippy -p ducktape-app --tests --no-deps
cargo clippy -p ducktape-rpc --tests --no-deps
cargo fmt -- --check   # only the files this plan touched must be clean
```

- [ ] **Step 5: Commit and open the PR against `dev`**

---

## Self-Review

**Spec coverage:** ① gate split → Task 1 + Task 3. ② shared reader widened → Task 2. ③ display rule (phase-discriminated, self-clearing vs cumulative) → Task 2 (`sync_in_progress`) + Task 4 (`sync_label`, the Settings "cumulative" note, the conditional `last_error` row). ④ explorer live head → Task 4 step 5.1 + Task 5 step 2. ⑤ guards → Task 2 step 5, Task 4 step 1, Task 5.

**Type consistency:** `sync_in_progress(&str) -> bool` is defined in Task 2 and consumed by `sync_label` in Task 4. `node_facts` (Task 2) is consumed by `node_status_live` (Task 3). `peer_rows` already exists. The seven new `NodeFacts` fields are named identically in the struct, the `Default`, the reader, the extern line, both handlers and the state block.

**Known gap, deliberate:** no node-side change is needed — the `status` topic already carries `operations`. If a future reader expects a `sync` projection endpoint, there is none and none is added.
