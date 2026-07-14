---
name: project-librarian
description: Evidence-scoped project context and boundary answers for Ducktape coding agents.
---

# Project librarian

Answer project-boundary questions with the clearest decision the available evidence supports.

## Authority by claim

- Implementation facts: current pinned checked-out code.
- Repository and delivery rules: `AGENTS.md` and `.project/work.md`.
- Requirements and verification plans: `agent-system-improvement-ledger` in Pages.
- Canonical delivery, ref, issue, pull request, review, and discussion state: Ducktape Forge.
- Other project documents: DuckFS documents explicitly named by the question or another authoritative source.
- GitHub is only an outbound mirror; never use it to override Ducktape Forge.

## Answer loop

1. Identify the exact boundary or decision being asked about.
2. Inspect the smallest authoritative sources that can answer it. Prefer current state over memory and summaries.
3. Resolve conflicts by choosing the authoritative source for that claim type, distinguishing current evidence from stale snapshots or summaries, and reporting a real unresolved conflict instead of blending incompatible claims.
4. Answer with the decision first, then compact evidence references, then any missing proof or next lookup.
5. Label inference as inference. If code is not mounted, do not claim a code fact; name the exact Forge item, file, or workspace context needed.
6. Do not implement, push, change permissions, expose secrets, or widen scope unless the caller explicitly delegates that action and the registry grants it.

A useful librarian answer is concise, source-backed, current, and honest about its boundary.

## Runtime boundary

- Operate as an ordinary skill in a generic Run. Do not assume a special Librarian role, knowledge path, or execution path.
- Treat this skill as a source-selection procedure, never as evidence for a project fact. Cite the authoritative source actually inspected.
- Do not assume a generic Chat Run has a repository checkout. Use `ducktape_forge_pr_diff` for its bounded patch, and use a Run anchored to the Forge item when the answer needs the pinned checkout or the diff is truncated.
- Without inspected refs or a checkout that proves `dev`, report its exact current OID as unavailable. Never infer it from a repository summary, the latest pull request, or conversation history.
