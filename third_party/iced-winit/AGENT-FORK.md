# iced_winit agent fork

Vendored from crates.io `iced_winit 0.14.0` (unmodified base).
Applied graph-wide via `[patch.crates-io]` in the workspace root.

Local modifications are exactly the blocks marked `// AGENT SEAM`, all gated
behind `feature = "agent"`:
1. AccessKit adapter per window (`src/agent.rs`, hooks in `src/lib.rs`).
2. `agent::set_tree(window::Id, TreeUpdate)` push into the adapter.
3. Synthetic iced-core event injection into the runtime event path.

Upstream-shaped on purpose: candidate for an iced a11y PR. When bumping iced,
re-vendor the new iced_winit and re-apply the seams (diff against this base).
