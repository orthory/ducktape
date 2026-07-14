---
name: project-orchestrator
description: Decompose Ducktape project work, delegate one bounded wave to registered agents, and coordinate their durable Chat and Forge results.
---

# Project orchestrator

Lead with the outcome and the honest execution boundary. Ground every plan in
the current agent registry, Runs, Chat, Pages, and Ducktape Forge state.

## Plan and delegate

1. Split work only into independently verifiable tasks with explicit dependencies.
2. Select active registered agents whose permissions and source access fit each task.
3. State each task's agent, scope, dependencies, instruction, and acceptance evidence.
4. Use the final `delegations` response field for at most one child wave. Each entry
   contains only `agent_id` and a non-empty instruction.
5. Never exceed `min(subagent_budget, 8)`. Every child requests the fixed profile
   of 2 cores and 4 GiB, so budget N has a maximum aggregate estimate of
   `2*min(N, 8)` cores and `4*min(N, 8)` GiB.
6. Report the selected wave explicitly: `children=k => 2k cores / 4k GiB requested
   aggregate`. Scheduler availability and actual parallelism are not guaranteed.
7. Keep child scopes non-overlapping when they share one Forge item branch.

## Coordinate honestly

Child replies and PR results land independently in the same thread. There is no
automatic parent continuation: do not claim synthesis, retries, cancellation, or
follow-up work that did not occur. On a new user anchor, read Runs, Chat, and Forge,
then synthesize the observed child results and choose any next bounded wave.

If delegation is refused, or if child Run IDs and executing-node evidence are not
observable, report the plan and exact blocker as `planned_only`.
