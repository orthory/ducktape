# Program users and model runs

## 1. Module ownership

identity records numbered accounts, their keys or program control, and the
controller of each program account. A program account has no signing key.
agent binds its pure-data program and interprets attribution changes through
Query, Call, Dispatch, Branch, Report and Finish steps.

attribution owns source objects, revisions, relationships, recipients and
durable change delivery. Chat messages, page blocks, and page comments carry resolved account
mentions. Their source writes commit before attributed programs react.

runs owns model configuration, composed context, model result validation,
run sessions and durable action proposals. dispatch owns admitted calls,
external-work recipes and their outcome ledgers. saga owns provider attempts,
execution leases, oracle results and deadline transitions.

## 2. Ordering and authority

Validators execute ordered operations with the block's agreed origin and
consensus_time. Module messages emitted within one atomic operation either
commit together or roll back together. Rejected members and deferred work
have isolated outcomes.

Attribution changes and dispatch calls are committed queue entries. Later
blocks execute them with their recorded cause. Before executing a program
call, dispatch and the host check the account's current executor, generation
and standing. The target receives Origin::Program(account), and resolves that
account through its normal source-module authorization.

P5: a saga attempt accepts at most one terminal oracle result. Duplicate and
stale results do not create a second transition.

P6: saga callbacks run in the terminal transition's atomic operation.
dispatch records the result and delivers its committed mailbox separately.
Model source actions subsequently run as program calls, each with its own
target outcome.

P7: Crank and reassignment use agreed consensus_time and recorded deadlines.
Worker speed and model output are external inputs; validators agree on the
accepted result rather than reproduce model execution.

## 3. Model workflow

runs::model_program constructs the default program from current Rust types.
Added chat/page mentions call RequestAttributedRun. A runs action_request
attribution reads ActionPlan, claims its future target call, executes that
message as the program account, and completes the proposal against the
dispatch ledger. Controllers can replace the program.

A new PR sink has no history link until its program supplies the actual
Forge output and runs verifies it against dispatch's committed output digest.
The allocated repository and number determine the link; queued or rejected
actions cannot predict one. Existing PR links come from committed Forge state.

ConfigureModel records an existing program account's capability tag and
skills, atomically with its dispatch recipe. Any member may change the record
later. ModelRecord.owner records the registering origin; it does not track
controller transfers. Any member may cancel or reassign a manual RequestRun.
Other content reactions remain program-created work. Duplicate manual
model/channel/anchor requests claim one run.

The compute service receives a committed work payload and returns an oracle
result. A host-owned ephemeral signer authenticates that run's interactive
`RunsMsg::AgentAction` requests against its session and lease. It is never an
identity key of the program account. The scoped HTTP endpoint
subscribes before admission and waits for the actual target receipt, which it
returns to the caller. Each execution attempt binds a fresh public key under
its lease holder's node key. The private key stays on that host; the guest
receives a scoped endpoint token. A retry replaces the binding and retains the
run's action counter. Terminal sagas and changed attempts refuse old keys and
unclaimed proposals. A target call already authorized by the program can
finish and retain its receipt. An attributed run whose binding fails does not
start its provider.

Every agent write is one envelope: `operation`, an optional `target`, an
`input`, and the caller's `request_id`. The MCP server exposes it as
`ducktape_action` and forwards it opaque; `ducktape_actions` lists the catalog
and `ducktape_receipt` reads a receipt back. Runs owns the catalog
(`RunsQuery::Catalog`): each operation's name, target and input schemas, result
schema and the lanes it admits (live through the session signer, final through
the response's `actions`, or both). The `submit` operation is the catalog's
floor: its target names a module and its input is that module's own message,
verbatim, prepared for the run's program account, so whatever a member may
submit to a module a run may. The same envelopes ride the final response, so
adding an operation to the module needs no change to the executor or the tool
binary. `request_id` is idempotent per run: the same bytes under
the same id answer with the existing receipt, different bytes are refused, and
the receipt id is `runs::action_request_id(run_id, request_id)`. Every
proposal is pinned to its operation's `schema_digest`; the program's claim
refuses a proposal whose operation schema changed under it.

`reply` is a catalog operation with no target. Runs resolves its destination
from committed source context: the original chat thread, the Pages comment
thread, a shared reply thread on the mentioned block, or the job discussion.
Live, final and failure replies use the same resolver and execute as the
program account. Pages reply validation reads thread metadata without loading
the discussion bodies. An action-only final response keeps its actions without
inventing another source reply. Explicit destinations are their own
operations: `chat.post_message` (channel_id, optional thread), `pages.comment`
(target or thread_id) and `jobs.comment` (job_id). `tasks.create`,
`tasks.update_status`, `pages.set_checked`, `pages.post` (a new page under the
run's `agent/` prefix), `duckfs.write_text`, `modules.update` (final only),
`agent.call` (live only) and `submit` (any module's own message, verbatim,
under the run's program account) complete the catalog; `react` and `unreact`
mark the source message. A destination never supplies the author.

The job board stores bounded, immutable comments with their authenticated actor
and commit height, and exposes them through its point read and index. Comments
do not claim, finalize or reopen a job. Each comment binds the job creation
revision, so delayed program calls cannot write into a replacement job, even
when an ID is reused at the same height. A default job reply also requires the
run's original claim when proposed; an explicit job destination selects the
current job. Runs forwards only job claims and
finalization as lifecycle operations; comments are proposals the program must
execute like every other conversational write.

## 4. Failure and persistence

Malformed or rejected source reactions cannot roll back the source content
already committed. Dispatch records target rejection, refusal and
unrepresentable outcomes; program continuations can report failure through
another attribution.

Saga requester callbacks share the terminal transition's operation, so their
handlers must not reject malformed callback data and poison that transition.
Dispatch mailbox delivery and program calls instead have explicit isolated
outcomes.

Runs action admission reserves an immutable body and a fixed completion
marker. A linked outbox permits bounded publication and acknowledgement
without enumerating historical receipts. Queries resolve the bound dispatch
outcome even if authority changes after a target succeeds. Native snapshots
and guest host-state snapshots include every receipt record.
