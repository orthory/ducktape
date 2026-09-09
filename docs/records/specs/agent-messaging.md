# Agent messaging across devices

**Status: proposed protocol, not implemented behavior.** Operation names below
are proposed catalog contracts, not commands that an operator can run today.
This specification defines cooperation inside one Ducktape network. It does not
change the live network, grant permissions, or authorize a deployment.

## Purpose and boundaries

A person can connect an existing Claude or Codex session to a collaboration, or
let Ducktape start an agent for a task. Agents exchange explicit messages and
results across devices without knowing each other's IP, local path, or provider
session identifier. People can observe the same collaboration from the app.

Messaging does not transfer a provider conversation, share every tool output,
start work implicitly, or prove that a model understood an input. A task result
is a claim with evidence; it is not an approval to merge or activate an artifact.

Reuse Ducktape's authenticated node connections and agent-service process
boundary. Each service makes an outbound connection to a reachable node; the
node network routes to other services. No inbound port on each laptop is
required. Transport availability remains a prerequisite: joining a network does
not create connectivity where no node can be reached.

## Existing foundations and missing behavior

The current [collaboration contract](../architecture/agent-collaboration-design.md)
assigns model execution to runs, calls and receipts to dispatch, and execution
attempts to saga. `agent.call` already delegates with caller/callee authority
intersection. Run-session signers are bound to the current execution lease.

The existing interactive service transports terminal creation, raw input and
output. Those byte streams are not structured delivery acknowledgements. The
provider specifications currently start cold executions; a new model run is not
the same operation as sending a message into an existing session.

Run-session and delegation records have execution-scoped lifetimes. They MUST
NOT be the only storage for participant addresses or collaboration history.
The existing run-output ring is observational and MUST NOT authorize messages,
claim tasks, or serve as durable delivery evidence.

Implementation ownership is:

- The collaboration module owns participants, conversations, immutable messages,
  recipient delivery records and their bounded retention. This is a proposed
  module boundary, not a second implementation of runs or chat.
- Existing tasks/runs/dispatch/saga own managed work and its effects. Messaging
  references those records rather than introducing a competing task scheduler.
- Chat/app render collaboration events and links. Rendered prose is not parsed
  back into authority or task transitions.
- Agent-service owns provider adapters and local session connections. Local
  socket paths and provider credentials never become network addresses.

## Identity and addressing

| Object | Meaning | Lifetime |
| --- | --- | --- |
| Participant | An owner-authorized collaboration identity, optionally bound to an existing agent account | Independent of a process |
| Conversation | An explicit participant roster and shared topic | Independent of individual runs |
| Task | A specific requested piece of work, referencing the existing job/run identity where managed | Request through terminal result |
| Binding | A participant's connection to a local provider session on one device | One attachment generation |
| Attempt | The execution generation assigned to a task | One execution attempt |

All identifiers are scoped by the authenticated network identity, never by an
RPC URL or display name. Names are discovery labels; mutation requests use
resolved immutable IDs. Ambiguous names return candidate IDs and do not pick the
first match.

A participant has one active input binding per conversation. Registering another
binding requires an explicit replacement with the expected binding generation.
Two devices cannot both claim that binding. Observers may subscribe without
becoming input owners. A participant may join several conversations, but each
attachment explicitly declares which conversations it will receive.

A task-specific message names the task identity and expected attempt. If the task
has been reassigned, the service reports a stale target instead of silently
sending the instruction to the new attempt. General conversation notices do not
name an attempt and can be delivered after a reconnect to the current binding.

An independently launched session attaches using an owner-authorized, scoped
messaging credential. It does not invent a run ID or acquire a node/run signing
key. Managed executions retain the existing run authority and lease checks.

## Messages and actions

Proposed catalog operations are `collaboration.send`,
`collaboration.messages`, and `collaboration.receipt`; binding operations belong
to the service control interface. Existing `agent.call` continues to start
managed delegated work. The MCP or CLI surface forwards typed requests to the
same catalog authorization path; it is not an alternate permission system.

A message contains:

```text
version
message_id                   {sender credential generation, sender sequence}
network_id, conversation_id
sender_participant_id        checked against authenticated origin/binding
recipient_participant_id
kind                         notice | question | task_request | task_update | result
reply_to                     optional immutable message reference
task                         optional {id, expected_attempt}
body                         UTF-8 text
references                   immutable commit/blob references or scoped duck:// links
expires_at                   agreed network-time deadline for delivery
```

`task_update` requires `task`; `result` requires `task` or `reply_to`. A question
can be conversational without creating a task. A `task_request` is an offer:
recording or delivering it does not claim the task. A managed task's acceptance
uses its existing claim/dispatch authority. A request to an attached personal
session can receive a structured acceptance, but is marked externally executed
and does not claim managed isolation or verifiable execution.

The sender service serializes admission per credential generation with a monotonic
sequence. The network retains a replay floor for that generation after pruning;
retired credentials remain unable to admit messages. A sequence below the floor
with no retained receipt returns `ReceiptPruned`, never a new admission. For one
sender, reusing `message_id` with identical canonical request bytes returns the
same receipt while retained. Different bytes under that ID are rejected. A relay
MUST NOT generate a fresh ID when retrying. The authenticated envelope determines
the sender; prose and client-supplied names cannot override it.

Messages are immutable. Corrections and withdrawals are new events referencing
the original. Links to mutable content include the revision or content hash that
the sender meant. Sending a link does not grant its recipient read access.
Local filesystem paths are not portable artifact references.

## Delivery is distinct from work

Delivery records have one current state per recipient:

```text
Stored -> Queued -> AdapterAccepted
                  -> Held -> AdapterAccepted | Refused | Expired
                  -> DeliveryUnknown -> AdapterAccepted | Expired
Stored or Queued -> Refused | Expired
```

`Stored` means the network accepted the immutable record. `Queued` means the
bound service has durably queued it. `AdapterAccepted` means the provider input
interface accepted it, not that the model read, understood, or acted on it.
`Held` exposes a provider/local approval barrier without overriding it.
`DeliveryUnknown` means the service cannot establish whether input was accepted.

Semantic acceptance, input required, progress, result submission and approval
are separate work events. In particular, `AdapterAccepted` does not imply a task
claim, a model turn completing does not imply the task completed, and a result
submission does not imply its evidence passed verification.

Only the currently authorized binding may advance its delivery record. A stale
service may report historical facts for inspection but cannot overwrite the
current binding's state or claim the current attempt.

The service persists its outbox and receipt mapping before acknowledging queue
ownership. If it crashes after provider input but before recording acceptance,
it first reconciles against provider-visible item IDs/history when supported.
If it cannot reconcile, it retains `DeliveryUnknown`; it MUST NOT silently claim
success or automatically replay potentially acted-upon instructions. Explicit
retry uses the same message ID and tells the receiver it may have seen it before.

The network therefore promises durable, deduplicated message admission, not
exactly-once model execution. Side-effecting actions require their existing
idempotency key, current authority and task/attempt checks independently.

## Ordering, wake-up and reconnect

Conversations have a committed event sequence. Services subscribe for change
notifications and fetch records after their last acknowledged sequence; a
WebSocket notification is a hint, not the authoritative event body. The adapter
processes each binding's eligible messages in sequence. Permission checks happen
again before disclosure/delivery, including after reconnect or revocation.

A message records its causal parent through `reply_to` and its task reference.
A result includes the input revision/event position it addresses. A task update
arriving after that point does not retroactively change what the result proves.
Ordering records an agreed history; it does not make concurrent agents read the
same context or resolve semantic conflicts automatically.

An online session advertises whether it can accept input while busy, wake from
idle, report input acceptance, and expose response events. Normal delivery is
queued until the provider's supported safe boundary. Urgent task updates may
steer an active turn only when the adapter supports it and the expected task and
turn still match. Unsupported steering remains visibly queued; it does not
interrupt a running tool or fall back to terminal keystroke injection.

Notices do not require replies. Delivery receipts never wake a model. Questions
and task requests may wake an explicitly attached idle session under its existing
policy and budget. Closed sessions are not silently recreated. A managed task may
start a replacement through runs; a personal session needs an explicit reattach.

Disconnection preserves the task's owner. Reassignment is an explicit authorized
transition through existing task/attempt machinery, not a consequence of a missed
heartbeat. A previous attempt cannot publish as the current one after returning.
Its local process may still run or edit files while disconnected; cancellation
or reassignment cannot promise that those external effects stopped. Record a
cancel request separately from confirmed termination. Do not roll back already
committed side effects by relabeling the task as canceled.

## Provider adapters

### Claude

Use the documented per-session inbox socket or another explicitly negotiated
provider input interface. Register the specific local session rather than
scanning and attaching every session owned by the operating-system user. Preserve
inbound accept/hold/refuse policy and keep the messaging token local. Socket write
success alone is insufficient for `AdapterAccepted` unless the selected interface
supplies a corresponding acceptance signal; otherwise report the uncertainty.

Inbound content carries a trusted wrapper naming the verified participant,
conversation, message and task. It remains peer-supplied content, not a direct
human instruction or higher-priority policy. Outbound messages use an explicit
collaboration tool; never forward the session's entire transcript automatically.

### Codex

For an existing session, the installed `codex queue --thread ... --message ...`
interface is the initial adapter candidate. Invoke it with an argument array,
not interpolated shell text. Resolve exact thread IDs locally. Validate its
acceptance semantics in a version-pinned conformance test before declaring a
receipt level; CLI exit success does not prove task execution.

For service-managed sessions, use App Server locally: initialize the connection,
create/resume the authorized thread, use `turn/start` when idle, and use
`turn/steer` with `expectedTurnId` for supported active input. Recheck after a
turn mismatch instead of injecting into whatever turn happens to be active.
Collect declared result/response events separately from arbitrary tool output.

App Server's experimental interfaces are isolated behind a pinned adapter.
Neither provider's socket needs to be exposed over the network. The network
protocol is provider-neutral and independent of the UI tree wire epoch.

## Authority and visibility

Discovery returns only participants the caller may discover. Conversation
membership and send authority are checked on admission and before queued
content reaches a newly bound session. Attachment cannot grant authority the
owner does not possess. A peer cannot bypass its denied operation by asking a
more privileged peer to execute it; managed delegation retains caller/callee
intersection and every target's own authorization.

For personal attached sessions, Ducktape cannot sandbox their existing local
filesystem/tools by transporting a message. Their local policies still apply;
they are not presented as equivalent to managed sandboxed runs.

Conversation visibility is explicit and inherited by its human-facing projection.
Posting restrictions alone do not prove read confidentiality. The first release
must state the underlying network's storage/read visibility and MUST NOT promise
end-to-end encrypted private messages without implementing that separately.
Provider credentials, socket tokens and unrestricted control endpoints are never
message payloads. Replies do not broaden the original audience implicitly.

## Bounds and retention

Initial protocol limits are proposals to validate with the acceptance workload:
16 KiB UTF-8 body, 16 references, 1 KiB per reference, and 32 KiB total encoded
message. Oversized input is rejected before admission, never silently truncated.
The initial send operation has one recipient; group delivery expands into
explicit per-recipient messages and receipts under the same conversation.

Each participant mailbox has at most 256 undelivered messages and 2 MiB of queued
encoded payload; both limits apply at admission, including while disconnected.
Replacing a binding does not reset this accounting. Full queues return a retryable capacity error without
claiming acceptance. Existing execution budgets bound managed model work; attached
sessions declare a wake-up budget. Receipts and presence cannot consume that
budget or cause acknowledgement loops. Per-sender admission quotas prevent one
participant from filling another's queue indefinitely.

Delivery defaults to a 24-hour deadline and may request at most seven days.
Use agreed network time for authoritative expiry, not a laptop's wall clock.
Expiry prevents further delivery; it does not undo accepted work. Retention and
pruning preserve deduplication records through the valid retry window; expired
request bytes cannot be admitted as a new message after their body is pruned.
Undelivered records remain until delivery, explicit refusal or expiry. Conversation
history follows an explicit retention policy. A consumer behind the
retained sequence floor receives an explicit history-gap response and resyncs;
it never advances its cursor while silently losing actionable messages.

Token deltas, typing and presence use bounded ephemeral streams. Task requests,
answers, decisions and result references use durable records. Large artifacts
are fetched by verified reference under the recipient's own authority.

## Acceptance evidence

Tests must use actual provider adapters for acceptance semantics, alongside
fault-controlled network/service tests. A socket echo or mock delivery alone is
not proof of model participation.

1. Attach a Claude session on device A and a Codex session on device B. Neither
   device can directly reach the other's provider socket. Exchange a question and
   explicit reply through the same network conversation.
2. Request a review of an immutable Forge commit. Observe separate message
   delivery, work acceptance, input-required question, answer and result reference.
   The result identifies the commit and input revision it actually reviewed.
3. Disconnect A before the question arrives. Reconnect and receive it in sequence
   without starting a second task. Leave the provider closed and verify queued
   state without silently spawning a replacement.
4. Hold/refuse provider input and observe the same state in the app. Exercise idle
   wake-up, a busy turn, turn-ID mismatch, and unsupported steering.
5. Retry identical and conflicting message IDs. Crash before input, after input
   but before receipt, and after receipt. Verify deduplication or explicit
   `DeliveryUnknown`; no fabricated read/complete status or duplicate deployment.
6. Reassign a task while its previous device is disconnected. Reject that old
   attempt's late publication; preserve its evidence as an old attempt. Test
   cancellation racing with an already committed action.
7. Revoke a participant or replace its binding while a message is queued. The old
   credential and stale generation cannot receive new content or acknowledge the
   new binding. Test cross-network IDs and ambiguous display names.
8. Exceed body, queue and wake-up budgets; verify explicit errors. Test expired
   retries after pruning and a replay cursor below the history floor.
9. Verify that only explicitly sent messages/results leave a personal session;
   unrelated conversation history, local paths and credentials are not copied.
10. Observe the entire task from the Ducktape app without treating PTY output or
    natural-language claims as authoritative state transitions.

Implementation proceeds through message admission/receipts, actual local adapters,
cross-device reconnect/fencing, then task and app integration. Wiring a build or
`modules.update` workflow is a consumer of this protocol, not part of its first
acceptance gate. It still requires independent build/approval/activation evidence.

## External protocol references

- [Claude cross-session messaging](https://code.claude.com/docs/en/cross-session-messaging):
  local inboxes, inbound controls and provider-operated cross-machine delivery.
- [Codex App Server](https://learn.chatgpt.com/docs/app-server):
  thread/turn lifecycle, steering, notifications and transport status.
- [Codex changelog](https://learn.chatgpt.com/docs/changelog):
  `codex queue` for existing local/remote sessions; installed CLI help is also an
  adapter-version input, not a guarantee about every deployed Codex version.
- [A2A specification](https://a2a-protocol.org/v1.0.0/specification/):
  message/context/task/artifact separation. Its optional send idempotency is not
  a substitute for this specification's explicit retry contract. External A2A
  interoperability can be an adapter; full A2A implementation is not required.
- [Agent Client Protocol](https://agentclientprotocol.com/protocol/v1/overview):
  a client-to-agent session boundary, not a durable cross-device mailbox.
