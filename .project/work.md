# Binding work rules

- All task work targets the `dev` branch: start worktrees from a checkout on
  `dev` (branches fork from `origin/dev`), keep PRs based on `dev`, and let
  `done` merge them into `dev`. Never base or merge task PRs on `main` —
  `main` advances only by an explicit, user-requested release of `dev`.
