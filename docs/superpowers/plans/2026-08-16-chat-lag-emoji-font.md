# Chat Lag Fix: Emoji Fallback Font Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kill the 550ms channel-switch freeze and scroll stalls by bundling an
emoji-capable font, so cosmic-text's emoji fallback resolves instead of
re-scanning the font database (~3.7ms per emoji per fresh row layout,
uncached on miss; measured 60x cheaper on hit).

**Architecture:** The app loads its fonts through `font` directives in
`app/src/ui/app.ice` (design assets in `crates/design/assets/fonts/`). Every
chat row's hover toolbar carries three emoji labels (👍 ✅ 👀); a fresh row
layout pays the fallback miss for each, so a switch mounting 46 fresh rows
pays 46 × 11ms. One more `font` directive + the asset makes every emoji
shaping a cheap resolved fallback. A source lint pins the directive; the rig
A/B (iced/debug beacon) proves the stall drop.

**Tech Stack:** Ice `font` directive, Noto Color Emoji (OFL), iced 0.14
beacon telemetry rig from the diagnosis session.

## Global Constraints

- No version machinery, no compat paths (repo rule).
- Wall-clock is never asserted in tests (frame_probe doctrine); guards are
  source lints.
- Worktree: `.worktree/chat-lag-diagnosis`, branch `perf/chat-lag-diagnosis`,
  PR against `dev`.
- Rig-only probe patches (qa-live Cargo.toml `[patch]`, lag-probe worktree
  eprintlns) must never reach a commit.

---

### Task 1: Bundle the emoji font and load it

**Files:**
- Create: `crates/design/assets/fonts/NotoColorEmoji.ttf` (10.6MB, from
  googlefonts/noto-emoji, already fetched to scratchpad)
- Create: `crates/design/assets/fonts/OFL-NotoColorEmoji.txt`
- Modify: `app/src/ui/app.ice:9` (add directive after GeistMono)
- Modify: `app/src/tests/design.rs:296` region (extend the font lint)
- Modify: `app/src/frame_probe.rs` `headless_renderer()` (load parity)

**Interfaces:**
- Consumes: existing `font` directive support (multiple lines allowed).
- Produces: the emoji face present in the app's font system at boot; lint
  `assert!(app.contains("font \"../../../crates/design/assets/fonts/NotoColorEmoji.ttf\""))`.

- [ ] **Step 1: Extend the design lint (failing first)**

In `app/src/tests/design.rs`, directly under the existing Geist assert:

```rust
    assert!(app.contains("font \"../../../crates/design/assets/fonts/NotoColorEmoji.ttf\""));
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p ducktape-app --lib tests::design -- --nocapture` (worktree)
Expected: FAIL on the new assert.

- [ ] **Step 3: Add the asset + license + directive**

```bash
cp <scratchpad>/NotoColorEmoji.ttf crates/design/assets/fonts/NotoColorEmoji.ttf
# license text from https://github.com/googlefonts/noto-emoji (OFL 1.1)
```

`app/src/ui/app.ice` after the GeistMono line:

```
  font "../../../crates/design/assets/fonts/NotoColorEmoji.ttf"
```

`app/src/frame_probe.rs` `headless_renderer()` after the GeistMono load:

```rust
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/design/assets/fonts/NotoColorEmoji.ttf"
        )));
```

- [ ] **Step 4: Run the design lint + the app suite**

Run: `cargo test -p ducktape-app`
Expected: PASS (design lint green; frame probes still under ceilings —
emoji shaping cost was wall-clock, not allocations).

- [ ] **Step 5: Clippy gate + commit**

Run: `cargo clippy -p ducktape-app --tests --no-deps`

```bash
git add crates/design/assets/fonts/NotoColorEmoji.ttf \
        crates/design/assets/fonts/OFL-NotoColorEmoji.txt \
        app/src/ui/app.ice app/src/tests/design.rs app/src/frame_probe.rs
git commit -m "perf(app): resolve emoji fallback with a bundled color emoji font"
```

### Task 2: Rig A/B verification

**Files:** none committed — rig operations only.

**Interfaces:**
- Consumes: qa-live rig (Xvfb :99, demo node, beacon collector, xdrive),
  before-numbers in scratchpad/lag-findings.md.
- Produces: after-numbers (switch-stall and scroll-stall medians) for the PR
  body.

- [ ] **Step 1: Clean the probe patch out of qa-live** (drop the `[patch]`
  block from qa-live `Cargo.toml`; `git -C .worktree/qa-live checkout Cargo.toml`)
- [ ] **Step 2: Point qa-live at the fix branch**: fetch + checkout the
  Task-1 commit; `cargo build -p ducktape-app --features iced/debug`.
- [ ] **Step 3: Rerun S3 (16 switches) + S2 (scroll storm)** with the
  collector; grep STALL lines.
- [ ] **Step 4: Record before/after** in the PR body draft:
  before = 550-610ms Layout stall per switch, 287ms Interact scroll burst.
  Expected after ≈ tens of ms. If switch stalls stay >150ms, STOP and
  re-attribute before shipping (the fix claim would be wrong).

### Task 3: PR to dev

- [ ] **Step 1:** Push `perf/chat-lag-diagnosis` (spec + plan + fix).
- [ ] **Step 2:** `gh pr create --base dev` — body carries the root-cause
  chain, the parabench numbers (3.7ms miss / 60us hit, repeat-miss
  uncached), rig A/B stall table, and the Generated-with trailer.
- [ ] **Step 3:** Merge per house rule (squash) if gates are green and
  confidence is high; otherwise leave open with risks listed.

### Task 4: File the upstream runtime findings (ducktape-ui)

**Files:** none in this repo — two GitHub issues on byeongsu-hong/ducktape-ui.

- [ ] **Issue A — escape-every-frame:** steady-state `[vs-layout]` lines show
  `escaped=1` on EVERY frame (window() estimate vs mounted range disagree
  permanently), so every frame pays sync + bust + a second layout pass.
  Include probe lines and the lag-probe worktree diff as repro.
- [ ] **Issue B — switch double fresh-mount:** a channel switch lays out 28
  fresh rows in pass1, then the re-aim mounts 18 MORE fresh rows in pass2
  (`memo1=(0,28,0,0) ... memo2=(0,18,0,0)`) — the first-frame window seeding
  aims wrong, doubling the freshly-shaped row bill.

### Task 5: Graduate the collector (spec's Permanence section)

**Files:**
- Create: `ops/beacon-collect/Cargo.toml`, `ops/beacon-collect/src/main.rs`
  (the scratchpad collector, verbatim minus rig-specific comments)
- Modify: `skills/qa/SKILL.md` (short "frame telemetry" note: build the app
  with `--features iced/debug`, run `ops/beacon-collect`, read stalls)

- [ ] **Step 1:** Copy the collector crate into `ops/beacon-collect/`
  (standalone `[workspace]` so the node workspace does not absorb it).
- [ ] **Step 2:** `cargo check` it from its own directory.
- [ ] **Step 3:** Add the QA-skill note + commit both.

### Task 6: Wrap-up

- [ ] Update memory (root cause, rig recipe additions: beacon collector,
  parabench, probe worktree), update scratchpad lag-findings.md to final.
- [ ] Final user report (Korean): root-cause chain, fix, numbers, what
  remains upstream, Mac re-test instructions.
