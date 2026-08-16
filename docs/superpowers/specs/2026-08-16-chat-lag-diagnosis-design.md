# Chat lag diagnosis — design

Date: 2026-08-16. Approved approach: measure first, fix what the numbers
name, and keep the measuring eye permanently.

## Problem

On a current build (`d2a198099`, macOS, `make dev`) the desktop app feels
heavy everywhere: channel switches hitch, scrolling stutters, typing hitches,
and the app feels loaded even at rest. Prior QA passes missed this because
the assistant's rig measured CPU and screenshots, never frame latency — and
the in-repo probes (`app/src/frame_probe.rs`) gate *allocations*, not
wall-clock, so a frame can pass every gate and still cost 200 ms.

Established facts that narrow the field:

- The dev profile already builds the whole GUI stack (iced, wgpu,
  cosmic-text, ui-lang-*) at `opt-level = 2`; the 08-01 lag campaign measured
  zero win from optimizing the app crate itself. "Debug build is just slow"
  does not explain the report.
- Live chat deltas are batched (`LIVE_CHAT_BATCH_LIMIT`, one reduce+rebuild
  per batch) and the hot window is bounded (`CHAT_HOT_WINDOW_LIMIT`), so the
  older O(history) rebuild theories are gated off by tests.
- ui#699 (merged 2026-08-15) made virtual-window escapes bust `memo_lazy`
  layout caches and re-run layout a second time. Correct — it fixed the
  blank-room bug — but its steady-state cost was never measured. It is the
  newest change on the frame path and the top suspect for scroll cost.
- iced 0.14 rebuilds the whole view on *every* message; there is no dirty
  check. Anything that publishes messages frequently (status ticks, scroll
  feedback, cursor blink) multiplies whatever a frame costs.

## Non-goals

- No speculative optimization: nothing is changed until a measurement names
  it. (The snap-end inversion came from exactly one unmeasured "obvious"
  change.)
- No invented telemetry: iced 0.14 already instruments every stage
  (`iced_debug` spans: Boot/Update/View/Layout/Interact/Draw/Present,
  streamed to `127.0.0.1:9167` when built with `--features iced/debug`).
  We consume that; we do not write our own timing layer.
- No app-code or protocol changes for measurement. The feature flag is
  additive and rig-only.

## Architecture

Three pieces, two of them already exist:

1. **The app, unmodified, built with `iced/debug`** —
   `cargo run -p ducktape-app --features iced/debug`. The built-in beacon
   client connects out to the collector when one is listening and is inert
   otherwise.
2. **A headless beacon collector** (new, ~100 lines): depends on
   `iced_beacon = "=0.14.0"` and consumes its public `run()` stream of
   `Event::SpanFinished { duration, span }`. Aggregates per stage — count,
   p50, p95, max — and, for Update spans, buckets by the message name the
   span already carries. Prints an immediate line for any span over a stall
   threshold (default 100 ms) and a summary table every 10 s. Lives in the
   session scratchpad first; promoted to `ops/` only if it earns permanence.
3. **The existing rig**: Xvfb, the seeded demo workspace (207 realistic
   messages in #Random), and the xdrive XTEST driver.

Scenario matrix, driven by xdrive, one collector log per scenario:

| Scenario | What it isolates |
|---|---|
| idle 60 s | invalidation frequency at rest (Present/sec = redraw rate; Update/sec = who publishes) |
| scroll storm | ui#699 bust + double-layout cost per wheel tick |
| channel-switch loop | switch-path Update/View/Layout split |
| typing burst | per-keystroke frame cost |

Attribution rule: if Layout dominates, add counters to the suspected
ui-lang-runtime paths (BustMemoLayouts walks, second layout passes) via the
proven local-path `[patch]` recipe — rig-only, never a PR of eprintlns. If
Update dominates, the span's message name already names the reducer. If
Draw/Present dominates, the problem is renderer-side and the fix
conversation changes entirely.

## Fixes

Only the top one or two offenders named by the numbers, each as its own PR
(ducktape or ducktape-ui as the offender dictates), each carrying
before/after stage medians from the same scenario. Candidates the numbers
may name (not a work list): scoping ui#699's memo bust to the escaping
column, skipping the second layout pass when the window did not move,
gating an idle publisher.

## Permanence

Whatever proves useful graduates: the collector to `ops/beacon-collect` (or
equivalent), plus a `make` target so a traced dev run is one command, plus a
short note in the QA skill. On the Mac the same feature flag works with
upstream `comet` if a GUI timeline is ever wanted. The stall log closes the
original gap: lag is visible as numbers, to the user and to the assistant,
without anyone having to *feel* it.

## Error handling

- Collector not running → beacon client stays disconnected; the app is
  unaffected. No failure mode reaches the user.
- Rig numbers are debug-profile numbers on a different box than the user's
  Mac. They attribute cost; they do not predict absolute Mac frame times.
  The user-felt verdict on the Mac remains the final gate.

## Testing / success criteria

1. An attribution table exists for all four scenarios (stage × p50/p95/max,
   update-message ranking).
2. Each shipped fix shows its scenario's dominant stage median dropping,
   before/after, on the same rig.
3. The user re-tests on the Mac and the constant heaviness is gone or the
   next offender is named by data, not guesswork.
