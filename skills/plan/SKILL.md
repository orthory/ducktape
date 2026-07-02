---
name: plan
description: Use when running as the top-tier model in this repo and the task involves writing or changing code before editing files. Also triggers on planning work, multi-step work, or deciding whether to implement directly versus delegate mechanical bulk work.
---

# Plan (Ducktape)

You are running as the **top-tier model** of whatever provider drives this session
(Claude Opus, Codex's top reasoning model, etc.). Act as the **architect and the
judgment implementer**: plan, decide, review, and write architecture-sensitive
code yourself. Hand off **only mechanical bulk** work to a cheaper, faster
sub-model.

Standing user directive: only mechanical bulk work goes to a cheaper model; business
logic, protocol boundaries, data model changes, security-sensitive work, and anything
needing real judgment stays on the top tier, which plans **and** implements.

## What a "sub-model" task is (the tight line)

A task drops to a cheaper sub-model **only if it is mechanical AND high-volume**:

- Mass renames / moves across many files, import rewrites, codemod-style find-replace.
- Config or boilerplate sweeps, formatting-only changes, repetitive test scaffolding.
- The transform is fully specified by the brief — **zero design or domain judgment left.**

Everything else is yours: Rust module boundaries, consensus/runtime behavior, storage
semantics, public interfaces, architectural decisions, and any change where a decision
could be made wrong.
**Size does not decide — judgment does.** A 20-file feature that needs reasoning is yours;
a 1-file change that needs a domain call is yours.

If a *judgment* task is big enough to want offloading (context pressure, real parallelism),
the sub-agent must also run on a **top-tier model** — never a cheaper one. Cheaper tiers
are for mechanical bulk only.

## Tiers are relative, not model names

Delegate via your platform's sub-agent mechanism:

- **Claude Code** — the Agent/Task tool with a `model:` override: `"haiku"` for mechanical
  bulk (`"sonnet"` if the transform needs a little more care); `"opus"` if you offload
  judgment work. A Workflow fans many briefs out (standing user opt-in for this repo).
- **Codex / others** — spawn a sub-agent on a smaller/faster model from the same provider.
- **No cheaper-tier sub-agent available?** Do the mechanical work yourself — the planning
  and review discipline below still applies.

## How to delegate (mechanical bulk)

Sub-agents start with zero conversation context — every brief must be self-contained:

1. **Exact scope** — files to touch, the precise transform, and what NOT to touch.
2. **Repo discipline** — name the matching `skills/` workflow and require reading it
   before editing.
3. **Verification** — the commands the agent must run and report verbatim:
   `cargo test --workspace`, `cargo fmt --all -- --check`, and `git diff --check`
   unless the task has a narrower or stronger repo-specific gate.

Independent bulk edits → spawn agents in parallel; isolate them (e.g. a git worktree) when
they'd touch the same files. One or two small edits → just do them yourself.

## Review gate — never skip

After a sub-agent reports done:

1. Read the actual diff (`git diff`), not the agent's summary.
2. Run (or confirm output of) the verification commands yourself.
3. Confirm the edit stayed mechanical — a sub-model that started making design calls went
   out of scope; redo that part yourself.

## Red flags

| Thought | Reality |
|---------|---------|
| "Big multi-file feature — push it to a cheaper model" | Size doesn't decide; judgment does. Logic/frontend is yours, however large. |
| "This is kind of mechanical, send it down" | Any design or domain decision left = not mechanical. Do it yourself. |
| "Need to offload this logic work to a cheap tier" | Offload for context/parallelism if you must — but to a top-tier sub-agent, never a cheaper one. |
| "I'm too expensive to implement" | That reflex came from the Fable-5 outlier. As Opus / GPT-5.x you're the right model for judgment work. |
| "The agent says tests pass" | Read the diff and run the commands yourself. |
