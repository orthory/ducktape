# Desktop chat freeze: root cause and remediation

Date: 2026-08-14
Status: implemented and verified on current `dev`
Target: `dev`

## Executive summary

The freezes were not a network timeout and they were not fixed by delaying
work. Chat history had become part of the cost of almost every app update:

- a composer edit rebuilt an outer timeline node for every retained message;
- a channel click copied the active window and three cached windows several
  times before its async load could start;
- each remote chat operation entered one global live reducer, copied the chat
  lists repeatedly, touched unrelated Pages/Bell/Forge state, and queued one
  global rebuild;
- root and thread history grew without a render-state bound;
- unmounted keyed-lazy subtrees were parked by dependency hash without enough
  reconciliation identity, so a later window could reclaim stale UI state.

These costs explain all three observed symptoms: typing slowed as history grew,
channel switching froze before network I/O began, and a peer posting a burst
starved local input while the event queue drained synchronous reducers.

The remediation makes the rendered chat state bounded, batches ready chat
deltas without a timer, isolates the timeline behind an aggregate revision,
uses numeric server cursors, and makes live-domain dispatch and memo ownership
explicit. There is no compatibility path: the previous index wire and raw
cursor shape are deleted.

## Evidence from the old hot paths

### Typing

`virtual-row` reduced layout and drawing, but the generated view still iterated
every retained message to construct its outer keyed/lazy wrapper. Each quiet
row inherited 48 `ChatScreen` callbacks. A 256-row composer edit and rebuild
performed roughly 11.5k allocations; the slope continued with retained history.

### Channel selection

The generated `choose_channel` update deep-copied the active messages and the
three-entry message cache, rescanned channels for projections, restored another
deep-copied cached window, and only then created the load task. A three-window,
256-row fixture measured roughly 23k allocations before the response path.

### Remote bursts

One applied chat operation produced one `LiveUpdate`. The old lifecycle handler
cloned the active message list at least three times, the open thread twice, and
the channel list repeatedly, then also ran the Pages, Bell, and Forge folds.
Iced drains every queued app message through `update` before its next build, so
a burst made the UI thread spend `O(operations * retained_history)` work before
it could process local input.

No ordinary chat update launched a runaway task. The freeze was synchronous
state reduction and view construction on the UI thread.

## Shipped model changes

### Bounded read model

- The main timeline is a sliding hot window of 256 roots.
- The rich-message room cache is deleted. A switch clears the presentation
  window and starts one indexed tail read instead of copying three retained
  windows through the by-value UI ABI.
- The open thread keeps its root and one 256-reply sliding page.
- Optimistic rows stay at the newest edge and survive canonical settlement.
- Every eviction path repairs author grouping and clears a selected action if
  its target left the window.
- Older rows remain authoritative in the chat index and are loaded by cursor;
  they are not deleted from the network history.

The app's root `messages` vector is now a presentation window, not the archive.

### Query model

Thread paging no longer exposes an index storage key as a string. The public
boundary is:

```text
request:  after_reply_seq: Option<u64>
response: next_reply_seq: Option<u64>
```

Ice represents absence as `0` in its `i64` field and converts only at the Rust
extern boundary. The index alone synthesizes its internal marker key. This
removes key-format coupling and makes ordering, validation, and overflow
behavior explicit.

Root history likewise uses a root-only numeric cursor projection. A long suffix
of thread replies no longer makes the app walk generic message pages looking
for one timeline root.

### Live reducer

- Consecutive chat frames that are already ready are published in ordered
  batches of at most 64. There is no sleep, debounce, or 1200 ms delay.
- The subscription carries a capacity-one UI permit. It does not read the next
  socket publication until the generated app message for the current one has
  finished update and all of its clones have been dropped. Iced's internal
  100-message proxy therefore cannot queue 100 chat batches ahead of input.
- A non-chat frame is an ordering barrier and becomes the next publication.
- One batch crosses one fused chat reducer and advances each affected render
  revision once.
- Exhaustive typed live-kind dispatch prevents Pages/Bell/Forge publications
  from evaluating the chat fold.
- A normal chat batch returns before unrelated domain reducers.

The batch cap bounds one reducer pass; the capacity-one permit supplies the
actual UI acknowledgement/backpressure between passes. Together they preserve
wire order without a timer and prevent a permanently hot room from filling the
framework queue ahead of local input.

### Render boundary

The root and thread timelines are whole-list keyed-lazy islands driven by cheap
aggregate revisions and their channel/thread identity. A composer edit, clock
tick, or unrelated state write hits the memo without cloning or enumerating the
message vector. On a real timeline change, row islands capture only their six
timeline actions instead of all 48 screen routes.

The runtime parking key is now the codegen expression plus reconciliation
scope, and each concrete mount retains only its latest dependency revision.
Another app/window cannot reclaim its subtree, and changing a monotonic
timeline revision cannot accumulate 1024 stale whole-list trees.

### Channel switch

One Rust transition derives the selected channel facts. The loader receives
only the selected channel ID; the root view owns tail selection and no longer
needs either a complete channel-list copy or a head-sequence hint. There is no
message-window cache to clone or reconcile: the click paints an honest loading
state and the single indexed tail read installs at most 256 roots.

## Complexity after the change

Let `W = 256`, `B = 64`, `R` be ready chat deltas, and `V` be the visible or
materialized thread window.

- composer edit: timeline work is `O(1)` on a memo hit;
- active root mutation: `O(W)` with a fixed upper bound;
- ready remote burst: at most `ceil(R / B)` app publications and `O(R * W)`
  root work in the current ordered fold, where `W` is hard-bounded at 256,
  rather than one global `O(unbounded_history)` reducer and rebuild per op;
- channel switch: scalar state transition plus one async window read;
- root/thread paging: one numeric cursor page per request and no reply-only scan
  for roots. Root history deliberately re-reads at most the 64-row prepend seam
  needed to keep scroll anchoring stable; thread pages advance without that
  overlap.

The remaining by-value Ice ABI copies bounded vectors once into and once out of
the Rust fold per publication. Individual deltas still scan the fixed window;
batching removes redundant publications and unrelated folds, not the semantic
need to apply each delta in order.
Moving the entire domain behind an opaque `Arc<ChatState>` could remove that
constant cost, but it is no longer proportional to archive length and is not
required to stop this freeze.

## Correctness invariants

- committed roots remain sequence-sorted and pending rows remain at the tail;
- optimistic confirmation preserves the virtual-row identity;
- reload and live resync preserve the same identity for the same message ID;
- root and reply cursors are numeric and monotonic;
- a bounded-window eviction cannot leave an action menu or edit draft targeting
  an absent row;
- every authored handler that writes a timeline also advances its timeline
  revision;
- live batching neither crosses a non-chat frame nor reorders chat deltas;
- root history and thread history remain queryable after render-window eviction.

## Verification plan and gates

The final delivery gate includes:

1. multi-size allocation slopes at 64, 256, 1024, and 4096 synthetic rows;
2. channel-switch allocation ceilings and history-size slopes;
3. 32- and 256-post remote bursts, publication count, order, render revision,
   and allocation slope;
4. 255/256/257 hot-window boundaries, optimistic settlement, history seams,
   selected-row eviction, and thread root preservation;
5. app unit/frame-probe, chat module, clippy, and desktop build lanes;
6. regenerated chat index wasm plus module parity/root checks;
7. source and generated-code assertions for single-message live dispatch,
   exhaustive typed routing, and timeline memo placement.

Absolute wall time is reported but not used as a flaky CI threshold. Allocation
slopes and publication counts are deterministic acceptance criteria.

The final rebased Linux probe measured:

- composer edit + rebuild: `6,347`, `6,331`, `6,347`, and `6,347`
  allocations at 64, 256, 1,024, and 4,096 synthetic historical rows;
- channel-switch reducer: `79` allocations, with the loading frame fixed at
  `6,676` allocations for 256, 1,024, and 4,096 historical rows;
- one bounded 32-post publication: `144,432` allocations over 256 historical
  rows and `144,509` over 4,096;
- one bounded 256-post input, split into four publications: `145,445` and
  `145,456` allocations respectively.

The app suite passed `408/408` tests after rebasing onto current `dev` and
pinning the official latest ducktape-ui merge. The chat module passed `95/95`,
the three-node dispatch/index round trip passed, both touched crates passed
their scoped clippy gates, the desktop build passed, and the regenerated wasm
artifacts passed the module parity check.

## Deliberately not done

- No debounce or arbitrary wait was added.
- No legacy string cursor, alternate decoder, protocol version, or migration
  branch remains.
- No selector-store framework or general-purpose state abstraction was added.
- The network archive is not truncated; only the desktop presentation window
  is bounded.

If future profiling shows the bounded Ice ABI copy is still material, the next
step is one opaque, channel-owned `ChatState`/snapshot handle. It should replace
the current vectors outright, not coexist as a compatibility layer.
