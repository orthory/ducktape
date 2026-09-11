# Agent messaging across devices

This specification defines cooperation inside one Ducktape network. It does not
grant permissions or authorize a deployment.

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

- The chat module (`crates/modules/apps/chat`) owns the conversation: the
  channel, its membership, and every message body. A participant is a chat
  `Party`; a message is a chat message named by its `message_id`.
- The collaboration module (`crates/modules/apps/collaboration`) owns what chat
  cannot say: per-recipient delivery records, device bindings, mailbox
  accounting and the per-channel event stream. It stores no body, topic or
  roster.
- Existing tasks/runs/dispatch/saga own managed work and its effects. Messaging
  references those records rather than introducing a competing task scheduler.
- The app renders chat and collaboration events and links. Rendered prose is
  not parsed back into authority or task transitions.
- Agent-service owns provider adapters and local session connections. Local
  socket paths and provider credentials never become network addresses.

## Identity and addressing

| Object | Meaning | Lifetime |
| --- | --- | --- |
| Participant | A chat party: an identity account or a bare signing key. Spelled on every CLI and daemon surface as a handle, `acct:<number>` or `key:<hex>` | Independent of a process |
| Channel | A chat channel: its membership is the roster and its post policy is who may speak | Independent of individual runs |
| Task | A specific requested piece of work, referencing the existing job/run identity where managed | Request through terminal result |
| Binding | A participant's connection to a local provider session on one device, on one channel | One attachment credential |
| Attempt | The execution generation assigned to a task | One execution attempt |

All identifiers are scoped by the authenticated network identity, never by an
RPC URL or display name. Every collaboration op carries the chain id inside its
signed payload and is refused on any other network.

A participant has one active input binding per channel. Binding again requires
the expected current credential, so two devices cannot both claim that binding.
Binding a service key seats that key as a channel member, which is what lets
it post and read there. A participant may sit on several channels, but each
attachment explicitly declares which channel it will receive.

A task-specific message names the task identity and expected attempt. If the task
has been reassigned, the service reports a stale target instead of silently
sending the instruction to the new attempt. General conversation notices do not
name an attempt and can be delivered after a reconnect to the current binding.

An independently launched session attaches using an owner-authorized, scoped
messaging credential. It does not invent a run ID or acquire a node/run signing
key. Managed executions retain the existing run authority and lease checks.

## Messages and actions

A message is a chat message. Sending one is two ops: chat's `PostMessage`,
which is the message, then collaboration's `Deliver`, which asks that ONE
recipient's bound device carry it to a provider session. The run catalog
exposes the second as `collaboration.deliver` and the receipt as
`collaboration.acknowledge` (`crates/modules/apps/runs/src/catalog.rs`);
binding belongs to the operator's `ducktape collab attach`. Existing
`agent.call` continues to start managed delegated work. The MCP or CLI surface
forwards typed requests to the same catalog authorization path; it is not an
alternate permission system.

A delivery request (`DeliverRequest` in
`crates/modules/apps/collaboration/src/interface.rs`) contains:

```text
channel_id                   the chat channel the message sits in
message_id                   the chat message id; chat's uniqueness is the dedup
recipient                    one chat party
kind                         notice | question | task_request | task_update | result
task                         optional {id, expected_attempt}
references                   immutable commit/blob references or scoped duck:// links
expires_at                   agreed network-time deadline for delivery
```

The sender is the authenticated origin, and the module admits the request only
when chat says that origin posted the message. `task_update` requires `task`;
`result` requires `task` or a message posted in a thread. A question can be
conversational without creating a task. A `task_request` is an offer:
recording or delivering it does not claim the task. A managed task's acceptance
uses its existing claim/dispatch authority. A request to an attached personal
session can receive a structured acceptance, but is marked externally executed
and does not claim managed isolation or verifiable execution.

Asking again for the same message and recipient with identical metadata
answers the existing record; different metadata under that pair is refused. A
relay MUST NOT mint a fresh chat message when retrying. The authenticated
envelope determines the sender; prose and client-supplied names cannot
override it.

Chat messages are immutable in their attribution history; a delivery record
names the message and never copies its body. Links to mutable content include
the revision or content hash that the sender meant. Sending a link does not
grant its recipient read access. Local filesystem paths are not portable
artifact references.

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

Each channel has a committed collaboration event sequence: every delivery
request, delivery advance and binding change. The node's pump reads it after
its last cursor as the binding's own scoped key and hands the daemon what the
network still permits; a delivery record is keyed by the chat sequence of the
message it names. The adapter processes each binding's eligible messages in
sequence. Permission checks happen again before disclosure/delivery, including
after reconnect or a binding's replacement.

A message records its causal parent through its chat thread and its task
reference.
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

Body size is chat's bound. A delivery request carries at most 16 references of
1 KiB each (`MAX_REFERENCES`, `MAX_REFERENCE_BYTES`). Oversized input is
rejected before admission, never silently truncated. One request names one
recipient; group delivery expands into explicit per-recipient requests and
records under the same channel.

Each participant mailbox has at most 256 undelivered messages and 2 MiB of
queued encoded delivery records; both limits apply at admission, including
while disconnected. Replacing a binding does not reset this accounting. Full
queues are refused without claiming acceptance. Existing execution budgets
bound managed model work; attached sessions declare a wake-up budget. Receipts
and presence cannot consume that budget or cause acknowledgement loops. A
per-sender quota of 64 undelivered messages in one recipient's mailbox prevents
one participant from filling another's queue indefinitely.

A delivery may request a deadline at most seven days out
(`MAX_DELIVERY_TTL_SECONDS`, scaled into the network's `time_unit`). Agreed
network time decides expiry, never a laptop's wall clock. Expiry prevents
further delivery; it does not undo accepted work. Delivery records are kept:
a terminal record is the deduplication evidence for its request, and the
daemon retires its own journal entry for a record only once the network clock
has passed that record's deadline.

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
5. Retry identical and conflicting delivery requests. Crash before input, after
   input but before receipt, and after receipt. Verify deduplication or explicit
   `DeliveryUnknown`; no fabricated read/complete status or duplicate deployment.
6. Reassign a task while its previous device is disconnected. Reject that old
   attempt's late publication; preserve its evidence as an old attempt. Test
   cancellation racing with an already committed action.
7. Detach a participant or replace its binding while a message is queued. The
   old credential cannot receive new content or acknowledge under the new
   binding. Test cross-network ops.
8. Exceed reference, queue and wake-up budgets; verify explicit errors. Test a
   delivery request past its deadline.
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
