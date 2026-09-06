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

ConfigureModel records an existing program account's capability tag, allowed
actions, caps and skills, atomically with its dispatch recipe. Its current
identity controller governs later changes. ModelRecord.owner records the
registering origin; it does not track controller transfers. Manual RequestRun
publications retain their authenticated requester's cancellation/reassignment
authority in runs-owned detail. Other content reactions remain program-created
work. Duplicate manual model/channel/anchor requests claim one run.

The compute service receives a committed work payload and returns an oracle
result. A host-owned ephemeral signer authenticates that run's interactive
AgentAction or DelegateRun requests against its session, lease and grant.
It is never an identity key of the program account. The scoped HTTP endpoint
subscribes before admission and waits for the actual target receipt.

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
