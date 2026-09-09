//! The node's collaboration half: committed state out to the agent daemon,
//! the daemon's receipts back into committed state.
//!
//! The daemon owns no chain and the module owns no pty, so something has to
//! stand between them. That is this, and it is deliberately the only place
//! either direction crosses:
//!
//! ```text
//!   collaboration module ──read──> pump ──wire::Command──> agent daemon
//!   collaboration module <─Acknowledge─ pump <─wire::Event─ agent daemon
//! ```
//!
//! ## every read is authenticated as the binding, not as the node
//!
//! Reads go out on [`noded::NodeCommand::QueryAs`] carrying the binding's
//! SCOPED SERVICE KEY as the reader, never the node's own identity. The module
//! refuses `Origin::System` for all of `ProtectedRead`, so a node cannot read a
//! conversation it holds no binding on — including the ones its own operator
//! owns. `via` names the conversation the key is scoped to, because the key
//! store is hashed and the module cannot find the binding by scanning.
//!
//! ## the deadline is the network's, and it is asked immediately before
//!
//! Nothing here consults a wall clock. A delivery window is checked with
//! `ProtectedRead::DeliveryEligibility` — judged against the block's
//! `consensus_time` — in the same sweep that hands the message over, so a
//! message that expired while an earlier one was being read is never delivered
//! under a verdict taken before it. A historical `adapter_accepted` in the
//! receipt is a FACT about what happened, and the module is explicit that it is
//! not permission to deliver again; the pump asks eligibility and never reads a
//! receipt to decide.
//!
//! ## what is never replayed
//!
//! Only a `Stored` record is handed over. `Queued` means the daemon's durable
//! journal already owns it, `Held` means a provider is holding it, and
//! `DeliveryUnknown` means nobody can establish whether input was accepted —
//! the spec forbids replaying that automatically, and the module answers
//! `NotReplayable` so the pump cannot even by mistake.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_service::wire;
use collaboration as collab;
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use tokio::sync::mpsc as lane;

use crate::collab_keys::Attached;

/// The module every read and write here names.
const COLLABORATION: &str = "collaboration";

/// Events read per page. The module caps a page itself; this is the pump's own
/// ceiling on how much one conversation may hold the sweep for.
const PAGE: u64 = 64;

/// Pages one conversation may consume in one sweep. A conversation catching up
/// over thousands of events must not starve the ones behind it — the cursor
/// persists across sweeps, so it resumes exactly where this stopped.
const PAGES_PER_SWEEP: usize = 8;

/// How deep the daemon's receipts may queue before the terminal plane starts
/// dropping them ([`noded::TerminalSessions::route_collab_to`]). Each one costs
/// a chain submission, so this is a few blocks of head room and not a buffer to
/// hide a wedged pump behind.
const RECEIPT_LANE: usize = 256;

/// How many MESSAGES may owe the chain a receipt before the pump stops handing
/// NEW ones to the daemon.
///
/// Backpressure and not a bin. Nothing owed is ever dropped: the diagram refuses
/// `Stored -> AdapterAccepted`, so a lost `Queued` makes the acceptance behind
/// it permanently unsubmittable, and a later state cannot stand in for an
/// earlier one. The pressure is applied at the only point where there is still
/// a choice — a delivery not yet made — because by the time a RECEIPT arrives
/// the daemon has already spent the fact and nobody would re-report it.
const MAX_OWING_MESSAGES: usize = 4096;

/// How often production sweeps committed state.
///
/// The sweep is what notices mail; the receipts come back on their own lane and
/// are never waited for here. It is a POLL because the module's sibling-module
/// notification (`CollaborationEvent::ConversationAdvanced`) reaches modules,
/// not the node process — so there is no push to subscribe to from out here.
// ponytail: a poll, because nothing pushes to this process yet. When the node
// grows a committed-event subscription, feed `wake` from it and keep this as
// the floor.
const SWEEP: std::time::Duration = std::time::Duration::from_secs(5);

/// Wire the pump onto a running node and let it run until the node stops.
///
/// Nothing is spawned when there is no terminal plane to pump for, when the
/// node serves no chain, or when the plane already has a collaboration
/// consumer — one pump per node, for the reason
/// [`noded::TerminalSessions::route_collab_to`] gives. Each is an ordinary
/// state and each says which one it was at `info`, once per boot.
pub(crate) fn spawn(
    commands: mpsc::Sender<noded::NodeCommand>,
    status: noded::StatusCell,
    terminals: Option<&noded::TerminalSessions>,
    workspace: PathBuf,
    network: &str,
) -> bool {
    let Some(terminals) = terminals else {
        return refuse_to_start("no_terminal_plane");
    };
    // an empty chain id is what a daemon serving no chain reports, and it is
    // also what scopes a service key (`collab_keys`): scoping to it would put
    // every network's bindings in one namespace. Nothing to pump, and nothing
    // safe to sign with.
    if network.is_empty() {
        return refuse_to_start("no_network");
    }
    let (receipt_tx, receipt_rx) = lane::channel(RECEIPT_LANE);
    if !terminals.route_collab_to(receipt_tx) {
        return refuse_to_start("collab_lane_taken");
    }
    let (wake_tx, wake_rx) = lane::channel(1);
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(SWEEP);
        // a sweep that outruns the interval must not queue a backlog of them:
        // each one reads the same committed state, so a skipped tick costs
        // nothing and a queued one costs a whole duplicate pass.
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            // the lane holds ONE tick. A full lane means a sweep is still
            // running and another is already queued behind it — dropping this
            // one is the same "skip" the interval does.
            if wake_tx.try_send(()).is_err() && wake_tx.is_closed() {
                return;
            }
        }
    });
    let pump = Pump::new(
        commands,
        status,
        terminals.clone(),
        workspace,
        network.to_string(),
    );
    tokio::spawn(pump.run(receipt_rx, wake_rx));
    tracing::info!(target: "ducktape::collab", "collaboration_pump_ready");
    true
}

/// this node runs no collaboration pump, and which of the three reasons it is.
/// One line per boot, so an operator whose messages go nowhere finds the cause
/// in the log rather than in the code.
fn refuse_to_start(reason: &'static str) -> bool {
    tracing::info!(
        target: "ducktape::collab",
        reason,
        "no collaboration pump on this node"
    );
    false
}

/// Everything the pump needs. Cloneable: the sweep half and the receipt half
/// run as one task, but the caller builds this before either exists.
pub(crate) struct Pump {
    commands: mpsc::Sender<noded::NodeCommand>,
    status: noded::StatusCell,
    terminals: noded::TerminalSessions,
    workspace: PathBuf,
    /// the chain id every op is bound to and every key is scoped by. Taken from
    /// the workspace at boot: a node serves exactly one network.
    network: String,
    /// what the chain still owes, PER MESSAGE and in reported order. See
    /// [`Pump::owe`]. `std::sync::Mutex`: every critical section takes what it
    /// needs and drops the guard before any `.await`.
    owed: std::sync::Mutex<BTreeMap<Message, std::collections::VecDeque<Unsent>>>,
}

/// one message's delivery record, as the module keys it.
type Message = (String, String, u64);

/// one receipt the daemon reported and the chain has not taken.
#[derive(Debug, Clone)]
struct Unsent {
    /// the generation the daemon reported, carried VERBATIM through every
    /// retry. Substituting the current one would let a stale device's receipt
    /// pass the module's fence on the second attempt.
    credential: collab::Credential,
    state: wire::State,
    reason: Option<String>,
}

/// What the daemon has already been told about one binding.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Announced {
    /// the binding credential last sent as a `MsgBind` generation. 0 = never.
    credential: collab::Credential,
    /// the retention floor last sent as a `MsgRetain`.
    floor: u64,
    /// the next committed event sequence to read.
    cursor: u64,
}

/// What one eligibility question answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Eligible {
    /// The network says a new attempt may be made, and this record is the one
    /// this pump owns.
    Deliver,
    /// The network answered, and the answer is no. Settled; nothing to revisit.
    No,
    /// The question could not be ASKED. Nothing is known, so nothing is done —
    /// and the message stays in front of the cursor for the next sweep.
    Unresolved,
}

/// What the committed record says about a receipt the chain did not take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Stop carrying it, for a reason READ BACK off the chain.
    Retire(&'static str),
    /// The record is behind this transition: `Queued` has to commit before it
    /// can. The daemon's own `Queued` receipt was lost, and this is the only
    /// bridge the delivery diagram offers.
    Bridge,
    /// Nothing was learned, or the transition is simply still pending. Keep it.
    Owe,
}

/// Everything one pump run remembers between sweeps.
#[derive(Debug, Default)]
struct Seen {
    /// which daemon this was learned about. A different one knows none of it
    /// ([`noded::TerminalSessions::attach_epoch`]).
    epoch: u64,
    /// the agreed clock value last sent as a `MsgTime`.
    time: u64,
    /// how many receipts the term plane had dropped when this pump last looked
    /// ([`noded::TerminalSessions::dropped_receipts`]).
    dropped: u64,
    /// keyed by (conversation, participant) — the binding's identity.
    bindings: BTreeMap<(String, String), Announced>,
}

impl Pump {
    /// Build the pump for `workspace` on `network`.
    pub(crate) fn new(
        commands: mpsc::Sender<noded::NodeCommand>,
        status: noded::StatusCell,
        terminals: noded::TerminalSessions,
        workspace: PathBuf,
        network: String,
    ) -> Self {
        Self {
            commands,
            status,
            terminals,
            workspace,
            network,
            owed: std::sync::Mutex::new(BTreeMap::new()),
        }
    }

    /// Take the daemon's receipt lane and run until it closes.
    ///
    /// `wake` is the sweep trigger and it is a CHANNEL, not a timer, so a test
    /// drives a sweep by sending on it and observes the effects on the daemon's
    /// command lane — no sleeping, no polling. Production feeds it from a
    /// ticker ([`spawn`]).
    pub(crate) async fn run(
        self,
        mut receipts: lane::Receiver<wire::Event>,
        mut wake: lane::Receiver<()>,
    ) {
        let mut seen = Seen::default();
        loop {
            // both arms are cancel-safe `recv`s, so the loser of a race loses
            // nothing: its message is still queued for the next turn.
            tokio::select! {
                event = receipts.recv() => match event {
                    Some(event) => self.receipt(event).await,
                    None => return self.closing("receipt_lane_closed"),
                },
                tick = wake.recv() => match tick {
                    Some(()) => self.sweep(&mut seen).await,
                    None => return self.closing("wake_lane_closed"),
                },
            }
        }
    }

    fn closing(&self, reason: &'static str) {
        tracing::info!(
            target: "ducktape::collab",
            reason,
            "collaboration pump stopping"
        );
    }

    // ---- committed state -> the daemon -------------------------------------

    /// One pass over every binding this device holds a key for.
    ///
    /// Gated on an ATTACHED DAEMON. Without one every command would be dropped
    /// by the term plane's writer, and a cursor advanced past a message whose
    /// `MsgDeliver` went nowhere is a message nothing looks at again until the
    /// operator re-attaches. Skipping the sweep keeps the cursor where it is.
    async fn sweep(&self, seen: &mut Seen) {
        if !self.terminals.has_sandbox() {
            return;
        }
        // a DIFFERENT daemon knows none of what was told to the last one, and
        // "is one attached" cannot tell the two apart — a daemon that dies and
        // redials with the same bindings looks identical to one that never left,
        // and its epoch is what says otherwise. Everything cached is forgotten:
        // the binds, the clock, the floors and the cursors all get re-sent, and
        // the daemon's own dedup journal absorbs anything it already had.
        let epoch = self.terminals.attach_epoch();
        if epoch != seen.epoch {
            tracing::info!(
                target: "ducktape::collab",
                epoch,
                reason = "daemon_reattached",
                "re-announcing every binding to a new agent daemon"
            );
            *seen = Seen {
                epoch,
                ..Default::default()
            };
        }
        // a receipt whose submission failed is retried here, before any new
        // delivery: the network learning what already happened comes first.
        self.resubmit().await;
        // a receipt the term plane could not hand over is GONE, and re-reading
        // the chain cannot recover it: the module records what was submitted,
        // and that receipt is precisely the one that never was. The daemon's
        // delivery journal is the only remaining witness, so it is asked to say
        // everything it holds again. Its own report is idempotent here — a
        // transition already owed folds ([`push_once`]) and one already
        // committed retires on the read ([`Pump::standing`]).
        let dropped = self.terminals.dropped_receipts();
        let lost_a_receipt = dropped != seen.dropped;
        seen.dropped = dropped;
        let now = self.status.current().consensus_time;
        // the daemon owns no clock; this is the only thing that advances the
        // one it judges expiry against. Sent before anything is delivered, so a
        // message handed over in this sweep is judged against this value and not
        // the older one frozen onto its frame at admission.
        if now > seen.time {
            seen.time = now;
            self.terminals
                .send(wire::Command::MsgTime { network_now: now })
                .await;
        }
        let attachments = match crate::collab_keys::attached(&self.workspace, &self.network) {
            Ok(attachments) => attachments,
            Err(error) => return self.refuse("attachments_unreadable", &error),
        };
        for attachment in attachments {
            let key = (
                attachment.conversation.clone(),
                attachment.participant.clone(),
            );
            if lost_a_receipt {
                self.terminals
                    .send(wire::Command::MsgReplay {
                        conversation: attachment.conversation.clone(),
                        participant: attachment.participant.clone(),
                    })
                    .await;
            }
            let mut announced = seen.bindings.get(&key).copied().unwrap_or_default();
            self.pump_one(&attachment, &mut announced).await;
            seen.bindings.insert(key, announced);
        }
    }

    /// Bring one binding up to date: its generation, its retention floor, and
    /// every message admitted for it since this pump last looked.
    async fn pump_one(&self, attachment: &Attached, announced: &mut Announced) {
        let binding = crate::collab_keys::BindingRef {
            network: &self.network,
            conversation: &attachment.conversation,
            participant: &attachment.participant,
        };
        let key = match crate::collab_keys::load(&self.workspace, binding) {
            Ok(Some(key)) => key,
            // listed but keyless: the operator removed the key file. Nothing to
            // authenticate with, so there is nothing to say about it.
            Ok(None) => return self.skip(attachment, "no_service_key"),
            Err(error) => return self.refuse("service_key_unreadable", &error),
        };
        let Some(collab::CollaborationReply::Access(access)) = self
            .read(
                attachment,
                &key,
                collab::ProtectedRead::Access {
                    conversation_id: attachment.conversation.clone(),
                },
            )
            .await
        else {
            return;
        };
        // 0 is "holds no binding here": the owner never signed the `Bind`, or an
        // `Unbind` spent it. Forget the generation so a later re-bind is sent as
        // a fresh one rather than looking unchanged.
        if access.binding_credential == 0 {
            announced.credential = 0;
            return self.skip(attachment, "unbound");
        }
        if announced.credential != access.binding_credential {
            announced.credential = access.binding_credential;
            // a new attachment has seen none of the conversation. Read from the
            // floor: everything below it is gone from the module, and everything
            // above may still be waiting for this participant.
            announced.cursor = 0;
            self.terminals
                .send(wire::Command::MsgBind(wire::Bind {
                    conversation: attachment.conversation.clone(),
                    participant: attachment.participant.clone(),
                    generation: access.binding_credential,
                    device: attachment.device.clone(),
                }))
                .await;
        }
        let Some(collab::CollaborationReply::Conversation(conversation)) = self
            .read(
                attachment,
                &key,
                collab::ProtectedRead::Conversation {
                    conversation_id: attachment.conversation.clone(),
                },
            )
            .await
        else {
            return;
        };
        // the daemon prunes its dedup record ONLY where the network has stopped
        // retaining the message, so this is what turns its bounded tracking
        // table from a permanent refusal into a working one.
        if conversation.floor_seq > announced.floor {
            announced.floor = conversation.floor_seq;
            self.terminals
                .send(wire::Command::MsgRetain {
                    conversation: attachment.conversation.clone(),
                    floor_seq: conversation.floor_seq,
                })
                .await;
        }
        announced.cursor = announced.cursor.max(conversation.floor_seq);
        self.drain_events(attachment, &key, announced).await;
    }

    /// Read forward from the cursor, delivering what is admitted for this
    /// participant.
    async fn drain_events(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        announced: &mut Announced,
    ) {
        for _ in 0..PAGES_PER_SWEEP {
            let Some(collab::CollaborationReply::Events(page)) = self
                .read(
                    attachment,
                    key,
                    collab::ProtectedRead::Events {
                        conversation_id: attachment.conversation.clone(),
                        from_seq: announced.cursor,
                        limit: PAGE,
                    },
                )
                .await
            else {
                return;
            };
            let (events, messages, next_seq) = match page {
                collab::EventPage::Page {
                    events,
                    messages,
                    next_seq,
                } => (events, messages, next_seq),
                // the cursor fell below the floor between two sweeps. Resync
                // there rather than advancing past events that no longer exist.
                collab::EventPage::HistoryGap { floor_seq } => {
                    announced.cursor = floor_seq;
                    continue;
                }
            };
            let exhausted = next_seq <= announced.cursor;
            match self
                .offer_page(attachment, key, announced.credential, &events, &messages)
                .await
            {
                // this page handed something over, or could not establish what
                // to do with something on it. STOP, with the cursor ON that
                // message: the next sweep reads the page again and either walks
                // past the receipt that has since committed, or retries the read
                // that failed.
                Some(hold) => {
                    announced.cursor = hold;
                    return;
                }
                None => announced.cursor = next_seq,
            }
            if exhausted {
                return;
            }
        }
    }

    /// Hand every message on one page that is admitted for this participant to
    /// the daemon. Answers the FIRST sequence the cursor must not advance past,
    /// or `None` when every message on the page was resolved and none delivered.
    ///
    /// Two things hold the cursor, and both must:
    ///
    /// * a message handed over — it stays `Stored` until the daemon's `Queued`
    ///   receipt commits a block later, and a cursor past it is a message
    ///   nothing looks at again if the link died between the two;
    /// * a message whose eligibility could not be ESTABLISHED — a dropped actor
    ///   lane, a refused read, a reply this build cannot decode. Walking past
    ///   one silently drops a queued message on a transient failure, which is
    ///   the same loss with none of the evidence.
    ///
    /// Later messages on the page are still offered either way: one unresolved
    /// read must not stall the mail behind it.
    async fn offer_page(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        credential: collab::Credential,
        events: &[collab::ConversationEvent],
        messages: &[collab::Message],
    ) -> Option<u64> {
        let mut hold: Option<u64> = None;
        let mut keep = |seq: u64| hold = Some(hold.map_or(seq, |first: u64| first.min(seq)));
        for seq in admitted_for(&attachment.participant, events) {
            let Some(message) = messages.iter().find(|message| message.seq == seq) else {
                // the event survives its message: the body was pruned out from
                // under the page. There is nothing to deliver and never will be.
                self.skip(attachment, "message_pruned");
                continue;
            };
            // the bound, applied where there is still a choice: a receipt is
            // already spent by the time it arrives, but a delivery is not. The
            // message stays `Stored` in front of the cursor and goes out when
            // the owed chain drains.
            if self.overloaded() {
                keep(seq);
                continue;
            }
            match self.eligible(attachment, key, seq).await {
                Eligible::Deliver => keep(seq),
                Eligible::No => continue,
                Eligible::Unresolved => {
                    keep(seq);
                    continue;
                }
            }
            self.terminals
                .send(wire::Command::MsgDeliver(Box::new(deliver(
                    message,
                    &attachment.participant,
                    credential,
                ))))
                .await;
            tracing::debug!(
                target: "ducktape::collab",
                conversation = %attachment.conversation,
                seq,
                "offered a message to the daemon"
            );
        }
        hold
    }

    /// May a NEW delivery attempt be made for this message, right now?
    ///
    /// Asked immediately before the handover and answered against the block's
    /// agreed time. Only a `Stored` record is work: every other state either
    /// belongs to a durable record this pump does not own, or is settled.
    ///
    /// Three answers and not two. "The network says no" and "I could not ask"
    /// are different facts: the first is settled and the message is finished
    /// with, the second is a transient failure whose message must be looked at
    /// again. Collapsing them into one `false` is how a queued message gets
    /// walked past and never delivered.
    async fn eligible(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        seq: u64,
    ) -> Eligible {
        let answer = self
            .read(
                attachment,
                key,
                collab::ProtectedRead::DeliveryEligibility {
                    conversation_id: attachment.conversation.clone(),
                    seq,
                },
            )
            .await;
        let reason = match answer {
            Some(collab::CollaborationReply::Eligibility(verdict)) => {
                let Err(reason) = admits_delivery(&verdict) else {
                    return Eligible::Deliver;
                };
                reason
            }
            // NOT permission, and not a refusal either: the module was never
            // heard from, or answered something this build cannot read as an
            // eligibility. `read` has already named which.
            None => return self.unresolved(attachment, seq, "read_failed"),
            Some(_) => return self.unresolved(attachment, seq, "unexpected_reply"),
        };
        tracing::debug!(
            target: "ducktape::collab",
            conversation = %attachment.conversation,
            seq,
            reason,
            "not offering a message to the daemon"
        );
        Eligible::No
    }

    /// the eligibility question could not be asked. Latched, because whatever
    /// stopped it stops every message behind it in the same sweep.
    fn unresolved(&self, attachment: &Attached, seq: u64, reason: &'static str) -> Eligible {
        if let Some(occurrences) = PUMP_WARN.hit(reason) {
            tracing::warn!(
                target: "ducktape::collab",
                conversation = %attachment.conversation,
                seq,
                reason,
                occurrences,
                "could not establish whether a message may be delivered; holding it"
            );
        }
        Eligible::Unresolved
    }

    // ---- the daemon's receipts -> committed state ---------------------------

    /// THE dispatch for everything the daemon reports about collaboration. One
    /// arm per variant, each a single delegation.
    async fn receipt(&self, event: wire::Event) {
        match event {
            wire::Event::MsgBound {
                conversation,
                participant,
                generation,
                capabilities,
            } => self.bound(&conversation, &participant, generation, capabilities),
            wire::Event::MsgBindRefused {
                conversation,
                participant,
                generation,
                reason,
            } => self.bind_refused(&conversation, &participant, generation, reason),
            wire::Event::MsgDelivery {
                conversation,
                participant,
                seq,
                binding_generation,
                state,
                reason,
                ..
            } => {
                self.acknowledge(
                    &conversation,
                    &participant,
                    seq,
                    binding_generation,
                    state,
                    reason,
                )
                .await;
            }
            // the term plane routes only the three above to this lane, so these
            // cannot arrive — named rather than wildcarded so a receipt added to
            // either plane fails the build here instead of being swallowed.
            misrouted @ (wire::Event::TermCreated { .. }
            | wire::Event::TermRefused { .. }
            | wire::Event::TermOutput { .. }
            | wire::Event::TermEnded { .. }) => self.misrouted(&misrouted),
        }
    }

    /// the binding is live on the daemon. A lifecycle fact, once per bind.
    fn bound(
        &self,
        conversation: &str,
        participant: &str,
        generation: collab::Credential,
        capabilities: wire::Capabilities,
    ) {
        tracing::info!(
            target: "ducktape::collab",
            conversation = %conversation,
            participant = %participant,
            generation,
            accepts_while_busy = capabilities.accepts_while_busy,
            wakes_idle = capabilities.wakes_idle,
            reports_acceptance = capabilities.reports_acceptance,
            steers_active_turn = capabilities.steers_active_turn,
            "a collaboration binding is live on the agent daemon"
        );
    }

    /// the daemon refused the bind. Nothing is retried here: every refusal names
    /// a state only the operator or a fresh committed `Bind` can change.
    fn bind_refused(
        &self,
        conversation: &str,
        participant: &str,
        generation: collab::Credential,
        reason: wire::BindRefusal,
    ) {
        tracing::warn!(
            target: "ducktape::collab",
            conversation = %conversation,
            participant = %participant,
            generation,
            reason = refusal_token(reason),
            "the agent daemon refused a collaboration binding"
        );
    }

    /// Commit what the daemon observed, as the binding itself.
    ///
    /// `binding_generation` becomes `binding_credential` verbatim: the daemon
    /// echoes the generation the delivery was aimed at, and the module refuses
    /// one that is not current. That is the fence — a returning stale device
    /// cannot overwrite the state of the attachment that replaced it, and this
    /// must not "helpfully" substitute the credential it just read.
    async fn acknowledge(
        &self,
        conversation: &str,
        participant: &str,
        seq: u64,
        binding_generation: collab::Credential,
        state: wire::State,
        reason: Option<String>,
    ) {
        let message = (conversation.to_string(), participant.to_string(), seq);
        let unsent = Unsent {
            credential: binding_generation,
            state,
            reason,
        };
        // BEHIND whatever this message already owes. The diagram is a chain —
        // `Stored -> Queued -> AdapterAccepted` — so submitting this now, while
        // an earlier transition of the SAME message is still waiting, is the
        // out-of-order submission the module refuses. Another message's queue
        // is unrelated and is not waited on.
        if self.owe_behind(&message, &unsent) {
            return;
        }
        self.commit_receipt(&message, unsent).await;
    }

    /// Queue `unsent` behind this message's existing debt, if it has any.
    /// Answers whether it was queued.
    fn owe_behind(&self, message: &Message, unsent: &Unsent) -> bool {
        let mut owed = self.owed.lock().expect("collab owed lock poisoned");
        let Some(chain) = owed.get_mut(message) else {
            return false;
        };
        push_once(chain, unsent);
        true
    }

    /// Submit one receipt, and OWE it if the submission did not land.
    ///
    /// A dropped acknowledgement is not self-healing. Once `Queued` commits, the
    /// eligibility read answers `already_queued` forever, so the pump never
    /// re-offers the message and the daemon never re-reports it: a failed
    /// `AdapterAccepted` submission would leave the network reading `Queued` for
    /// a message a provider took, with no operator action short of re-binding
    /// able to correct it.
    async fn commit_receipt(&self, message: &Message, unsent: Unsent) {
        let (conversation, participant, seq) = message;
        let binding = crate::collab_keys::BindingRef {
            network: &self.network,
            conversation,
            participant,
        };
        let key = match crate::collab_keys::load(&self.workspace, binding) {
            Ok(Some(key)) => key,
            // ABSENT is permanent: the operator removed the key, and nothing
            // here can ever sign for that binding again.
            Ok(None) => {
                return self.refuse(
                    "receipt_without_key",
                    "a receipt named a binding this device holds no service key for",
                );
            }
            // UNREADABLE is not. A permissions problem or a half-written file is
            // a local fault that clears, and a fact must not be thrown away
            // because this process could not open a file for a moment. Owed with
            // nothing to verify against, because verifying also needs the key.
            Err(error) => return self.owe(message, unsent, None, &error).await,
        };
        let op = collab::CollaborationMsg::Acknowledge {
            conversation_id: conversation.clone(),
            seq: *seq,
            binding_credential: unsent.credential,
            state: delivery_state(unsent.state),
            reason: unsent.reason.clone(),
        };
        match self.submit(&key, op).await {
            Ok(height) => tracing::debug!(
                target: "ducktape::collab",
                conversation = %conversation,
                seq,
                height,
                "acknowledged a delivery on-chain"
            ),
            Err(error) => self.owe(message, unsent, Some(&key), &error).await,
        }
    }

    /// Keep a receipt the chain did not take — unless the chain has ALREADY
    /// been shown to hold the fact, or to have moved somewhere this transition
    /// can never reach.
    ///
    /// There is no attempt counter, deliberately. A count cannot tell a busy
    /// actor from a permanent refusal, so counting means eventually throwing
    /// away a fact that was merely unlucky. The committed record can tell:
    /// [`Pump::standing`] reads it and retires the receipt only on a VERIFIED
    /// outcome. Everything else is owed, for as long as it takes — and when the
    /// record is merely BEHIND, the missing `Queued` goes in front of it so the
    /// pair submits in the order the diagram admits.
    ///
    /// `verify` is the binding's key when one could be loaded. Without it the
    /// receipt is simply owed: an unreadable key is exactly the transient fault
    /// that must not cost a fact, and it is also the thing a verification would
    /// have needed.
    ///
    /// This never turns a receipt away. The daemon has already SPENT the fact by
    /// reporting it — refusing here would drop it with nobody to re-report it,
    /// which is the eviction this whole design refuses. The table is instead
    /// bounded upstream, where the pump still has a choice: it stops handing new
    /// messages to the daemon while it is this far behind ([`Pump::overloaded`]),
    /// and a repeated report of a transition already owed is folded rather than
    /// appended ([`push_once`]), so a daemon replaying its journal cannot grow
    /// one message's chain past the few transitions the diagram allows.
    async fn owe(
        &self,
        message: &Message,
        unsent: Unsent,
        verify: Option<&commonware_cryptography::ed25519::PrivateKey>,
        error: &str,
    ) {
        let standing = match verify {
            Some(key) => self.standing(message, unsent.state, key).await,
            None => Standing::Owe,
        };
        let bridge = match standing {
            Standing::Retire(reason) => return self.refuse(reason, error),
            Standing::Bridge => Some(Unsent {
                // the daemon's generation, not the current one: the bridge is
                // part of the same report and passes the same fence.
                credential: unsent.credential,
                state: wire::State::Queued,
                // nothing to say about it. The daemon never told us why it
                // queued this, and inventing a token would put a sentence on
                // chain that no service ever said.
                reason: None,
            }),
            Standing::Owe => None,
        };
        let owing = {
            let mut owed = self.owed.lock().expect("collab owed lock poisoned");
            let chain = owed.entry(message.clone()).or_default();
            if let Some(bridge) = bridge {
                push_once(chain, &bridge);
            }
            push_once(chain, &unsent);
            owed.len()
        };
        if owing >= MAX_OWING_MESSAGES {
            self.refuse(
                "owed_receipts_high",
                "no new message goes to the daemon until the chain catches up",
            );
        }
        self.refuse("acknowledge_retrying", error);
    }

    /// Is the chain so far behind that no NEW message should be handed over?
    ///
    /// This is where the bound is applied, and it is the only place it CAN be: a
    /// receipt has already been spent by the time it reaches [`Pump::owe`], but
    /// a delivery has not been made yet. Holding one back costs a sweep; the
    /// message stays `Stored`, in front of the cursor, and goes out when the
    /// backlog drains. Nothing is lost and nothing is dropped.
    fn overloaded(&self) -> bool {
        self.owed.lock().expect("collab owed lock poisoned").len() >= MAX_OWING_MESSAGES
    }

    /// Where does the committed record leave this transition?
    ///
    /// [`Standing::Retire`] only on a fact READ BACK off the chain:
    ///
    /// * the committed state IS the one being reported — it landed after all
    ///   (a lost reply is indistinguishable from a lost submission from here);
    /// * there is no record — the message was pruned, and nothing will accept a
    ///   receipt for it;
    /// * the record can never reach the reported state, by the diagram — every
    ///   transition out of a terminal state, and the few non-terminal pairs the
    ///   diagram simply does not join. Carrying one of those forever is a leak
    ///   with no outcome at the end of it.
    ///
    /// [`Standing::Owe`] on every read failure. An unverified receipt is never
    /// abandoned.
    async fn standing(
        &self,
        message: &Message,
        state: wire::State,
        key: &commonware_cryptography::ed25519::PrivateKey,
    ) -> Standing {
        let (conversation, participant, seq) = message;
        // the device label is not part of a READ — only the participant acting
        // and the conversation its key is scoped to are.
        let attachment = Attached {
            network: self.network.clone(),
            conversation: conversation.clone(),
            participant: participant.clone(),
            device: String::new(),
        };
        let answer = self
            .read(
                &attachment,
                key,
                collab::ProtectedRead::Receipt {
                    conversation_id: conversation.clone(),
                    seq: *seq,
                },
            )
            .await;
        // an unreadable answer, or one this build cannot interpret: nothing has
        // been verified, so nothing is given up.
        let Some(collab::CollaborationReply::Receipt(receipt)) = answer else {
            return Standing::Owe;
        };
        let Some(receipt) = receipt else {
            return Standing::Retire("receipt_gone");
        };
        let reported = delivery_state(state);
        if receipt.state == reported {
            return Standing::Retire("acknowledge_already_landed");
        }
        // the diagram, asked directly. `Queued` is the ONLY state anything
        // bridges through — a record still `Stored` because the daemon's queue
        // receipt was lost cannot take the acceptance that followed it.
        let directly = receipt.state.may_advance_to(reported);
        let through_queued = receipt.state.may_advance_to(collab::DeliveryState::Queued)
            && collab::DeliveryState::Queued.may_advance_to(reported);
        match (directly, through_queued) {
            (true, _) => Standing::Owe,
            (false, true) => Standing::Bridge,
            (false, false) => Standing::Retire("acknowledge_unreachable"),
        }
    }

    /// Retry what the chain is owed, per message and in reported order.
    ///
    /// A message stops at its FIRST failure and keeps the rest of its chain
    /// behind it — that is what makes this a recovery rather than a second way
    /// to lose the record, because the diagram refuses a transition taken out of
    /// order. Messages do not wait on each other.
    async fn resubmit(&self) {
        let debts: Vec<(Message, std::collections::VecDeque<Unsent>)> = {
            let mut owed = self.owed.lock().expect("collab owed lock poisoned");
            std::mem::take(&mut *owed).into_iter().collect()
        };
        for (message, chain) in debts {
            for (offset, unsent) in chain.iter().enumerate() {
                self.commit_receipt(&message, unsent.clone()).await;
                let still_owing = self
                    .owed
                    .lock()
                    .expect("collab owed lock poisoned")
                    .contains_key(&message);
                if still_owing {
                    // it failed again and re-owed itself. Everything after it
                    // goes back behind it, unattempted.
                    let mut owed = self.owed.lock().expect("collab owed lock poisoned");
                    let queue = owed.entry(message.clone()).or_default();
                    for later in chain.iter().skip(offset + 1) {
                        queue.push_back(later.clone());
                    }
                    break;
                }
            }
        }
    }

    fn misrouted(&self, event: &wire::Event) {
        let kind = match event {
            wire::Event::TermCreated { .. } => "term_created",
            wire::Event::TermRefused { .. } => "term_refused",
            wire::Event::TermOutput { .. } => "term_output",
            wire::Event::TermEnded { .. } => "term_ended",
            wire::Event::MsgBound { .. }
            | wire::Event::MsgBindRefused { .. }
            | wire::Event::MsgDelivery { .. } => "collab",
        };
        tracing::warn!(
            target: "ducktape::collab",
            reason = "misrouted_event",
            event = kind,
            "a terminal event reached the collaboration pump"
        );
    }

    // ---- the two lanes to the node actor ------------------------------------

    /// One authenticated read, as the binding's scoped service key.
    ///
    /// `None` on every failure — a closed actor lane, a module refusal, an
    /// undecodable reply — each with its own reason token. A caller treats
    /// `None` as "learned nothing", never as a permissive default.
    async fn read(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        read: collab::ProtectedRead,
    ) -> Option<collab::CollaborationReply> {
        let query = collab::CollaborationQuery::Read {
            participant_id: attachment.participant.clone(),
            // the key is scoped to ONE conversation and the store is hashed, so
            // the module cannot find the binding that holds it by scanning. The
            // caller names the one it holds.
            via: Some(attachment.conversation.clone()),
            read,
        };
        let (reply, answer) = oneshot::channel();
        let sent = self
            .commands
            .clone()
            .send(noded::NodeCommand::QueryAs {
                target: COLLABORATION.to_string(),
                req: collab::encode_query(&query),
                reader: commonware_cryptography::Signer::public_key(key)
                    .as_ref()
                    .to_vec(),
                reply,
            })
            .await;
        if sent.is_err() {
            self.refuse("node_actor_gone", "the node actor is not accepting reads");
            return None;
        }
        let bytes = match answer.await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(error)) => {
                self.refuse("read_refused", &error);
                return None;
            }
            Err(_) => {
                self.refuse("node_actor_gone", "the node actor dropped a read");
                return None;
            }
        };
        match collab::decode_reply(&bytes) {
            Ok(collab::CollaborationReply::Denied(denial)) => {
                self.refuse("read_denied", denial_token(denial));
                None
            }
            Ok(reply) => Some(reply),
            Err(error) => {
                self.refuse("reply_undecodable", &error);
                None
            }
        }
    }

    /// One collaboration op, signed by the binding's scoped service key.
    ///
    /// The op travels inside a [`collab::Request`] naming the network, and that
    /// name is inside the SIGNED payload: a submit frame carries no chain id, so
    /// without it these bytes would verify on any network the same key may
    /// submit to.
    async fn submit(
        &self,
        key: &commonware_cryptography::ed25519::PrivateKey,
        op: collab::CollaborationMsg,
    ) -> Result<u64, String> {
        let request = collab::Request::new(self.network.clone(), op);
        let frame =
            crate::userkey_cli::user_frame(key, COLLABORATION, collab::encode_msg(&request));
        let (reply, answer) = oneshot::channel();
        self.commands
            .clone()
            .send(noded::NodeCommand::SubmitFrame { frame, reply })
            .await
            .map_err(|_| "the node actor is not accepting submissions".to_string())?;
        match answer.await {
            Ok(Ok(block)) => Ok(block.height),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("the node actor dropped a submission".to_string()),
        }
    }

    // ---- the two ways a sweep says nothing happened -------------------------

    /// something this pump depends on is unusable. Latched: a gone actor or an
    /// unreadable key repeats every sweep.
    fn refuse(&self, reason: &'static str, detail: &str) {
        if let Some(occurrences) = PUMP_WARN.hit(reason) {
            tracing::warn!(
                target: "ducktape::collab",
                reason,
                occurrences,
                "collaboration pump: {detail}"
            );
        }
    }

    /// an ordinary state that means there is nothing to do for one binding.
    fn skip(&self, attachment: &Attached, reason: &'static str) {
        tracing::debug!(
            target: "ducktape::collab",
            conversation = %attachment.conversation,
            participant = %attachment.participant,
            reason,
            "nothing to pump for this binding"
        );
    }
}

/// per-sweep `ducktape::collab` warns that repeat for as long as their cause
/// stands: an unreadable key, a gone actor, a refused read. First occurrence,
/// then every 100th, carrying `occurrences`.
static PUMP_WARN: noded::log::Latch = noded::log::Latch::new(100);

// ---- decisions, made without touching anything -----------------------------

/// Append one owed transition, unless this message already owes exactly it.
///
/// A daemon that restarts replays its durable journal, so the SAME
/// `(credential, state)` can be reported many times over. Appending each would
/// grow one message's chain without bound and re-submit a transition the module
/// would refuse as already made. Folding is safe because the pair is the whole
/// content of the op: two identical entries produce two identical submissions.
///
/// The `reason` is deliberately not compared. It is a token about the same
/// transition, so a second report of it is the same fact told slightly
/// differently, not a new one.
fn push_once(chain: &mut std::collections::VecDeque<Unsent>, unsent: &Unsent) {
    let already = chain
        .iter()
        .any(|owed| owed.credential == unsent.credential && owed.state == unsent.state);
    if already {
        return;
    }
    chain.push_back(unsent.clone());
}

/// The sequences on this page that admitted a message FOR `participant`.
///
/// A `DeliveryAdvanced` on the page is this pump's own acknowledgement coming
/// back, or an expiry somebody ran; neither is work. A `BindingChanged` or
/// `RosterChanged` is read from `Access` on the next sweep, which is the
/// authority for both.
fn admitted_for(participant: &str, events: &[collab::ConversationEvent]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|event| match &event.body {
            collab::EventBody::MessageAdmitted { recipient, .. } if recipient == participant => {
                Some(event.seq)
            }
            collab::EventBody::MessageAdmitted { .. }
            | collab::EventBody::DeliveryAdvanced { .. }
            | collab::EventBody::BindingChanged { .. }
            | collab::EventBody::RosterChanged { .. } => None,
        })
        .collect()
}

/// Whether this verdict admits a NEW delivery, or the token naming why not.
///
/// `Eligible` alone is not enough: it is returned for every non-terminal state,
/// and only `Stored` is work this pump owns. `Queued` and `Held` are records
/// something else holds durably, and re-offering either would duplicate an
/// instruction a model has already been given.
fn admits_delivery(verdict: &collab::DeliveryEligibility) -> Result<(), &'static str> {
    let collab::DeliveryEligibility::Eligible { state, .. } = verdict else {
        return Err(match verdict {
            collab::DeliveryEligibility::Eligible { .. } => unreachable!("matched above"),
            collab::DeliveryEligibility::Expired { .. } => "expired",
            collab::DeliveryEligibility::Settled { .. } => "settled",
            collab::DeliveryEligibility::NotReplayable => "not_replayable",
            collab::DeliveryEligibility::Unbound => "unbound",
            collab::DeliveryEligibility::Unknown => "unknown",
        });
    };
    match state {
        collab::DeliveryState::Stored => Ok(()),
        collab::DeliveryState::Queued => Err("already_queued"),
        collab::DeliveryState::Held => Err("held"),
        collab::DeliveryState::DeliveryUnknown => Err("not_replayable"),
        collab::DeliveryState::AdapterAccepted
        | collab::DeliveryState::Refused
        | collab::DeliveryState::Expired => Err("settled"),
    }
}

/// The committed message as the frame the daemon places into a session.
///
/// A straight projection with nothing added: every field is the module's, and
/// `urgent` is false because no committed field says otherwise — steering an
/// active turn is a sender's explicit request and the module carries none, so
/// inventing one here would let the pump interrupt a model on its own authority.
///
/// `credential` is the RECIPIENT's current binding credential, read this sweep.
/// It is the fence: a daemon holding a newer generation drops this delivery
/// rather than handing a message aimed at a replaced attachment to the device
/// that replaced it. The sender's credential is a different number and lives in
/// `message_id`.
fn deliver(
    message: &collab::Message,
    participant: &str,
    credential: collab::Credential,
) -> wire::Deliver {
    wire::Deliver {
        conversation: message.conversation_id.clone(),
        participant: participant.to_string(),
        seq: message.seq,
        binding_generation: credential,
        message_id: wire::MessageId {
            generation: message.message_id.generation,
            sequence: message.message_id.sequence,
        },
        sender: message.sender.clone(),
        kind: kind(message.kind),
        task: message.task.as_ref().map(|task| wire::TaskRef {
            id: task.id.clone(),
            expected_attempt: task.expected_attempt,
        }),
        reply_to: message.reply_to,
        body: message.body.clone(),
        references: message.references.iter().map(reference).collect(),
        expires_at: message.expires_at,
        network_now: message.admitted_at,
        urgent: false,
    }
}

fn kind(kind: collab::MessageKind) -> wire::Kind {
    match kind {
        collab::MessageKind::Notice => wire::Kind::Notice,
        collab::MessageKind::Question => wire::Kind::Question,
        collab::MessageKind::TaskRequest => wire::Kind::TaskRequest,
        collab::MessageKind::TaskUpdate => wire::Kind::TaskUpdate,
        collab::MessageKind::Result => wire::Kind::Result,
    }
}

fn reference(reference: &collab::Reference) -> wire::Reference {
    match reference {
        collab::Reference::Commit { repo, commit } => wire::Reference::Commit {
            repo: repo.clone(),
            commit: commit.clone(),
        },
        collab::Reference::Blob { hash } => wire::Reference::Blob { hash: hash.clone() },
        collab::Reference::Duck { url } => wire::Reference::Duck { url: url.clone() },
    }
}

/// What the daemon observed, as the module's own state.
///
/// `DeliveryState::Stored` has no arm because it is the NETWORK's: it means the
/// module admitted the record and no service has queued it. A daemon reporting
/// it would be reporting somebody else's fact.
fn delivery_state(state: wire::State) -> collab::DeliveryState {
    match state {
        wire::State::Queued => collab::DeliveryState::Queued,
        wire::State::AdapterAccepted => collab::DeliveryState::AdapterAccepted,
        wire::State::Held => collab::DeliveryState::Held,
        wire::State::Refused => collab::DeliveryState::Refused,
        wire::State::Expired => collab::DeliveryState::Expired,
        wire::State::DeliveryUnknown => collab::DeliveryState::DeliveryUnknown,
    }
}

fn refusal_token(reason: wire::BindRefusal) -> &'static str {
    match reason {
        wire::BindRefusal::StaleGeneration => "stale_generation",
        wire::BindRefusal::UnknownDevice => "unknown_device",
        wire::BindRefusal::SessionUnreachable => "session_unreachable",
        wire::BindRefusal::AtCapacity => "at_capacity",
    }
}

fn denial_token(denial: collab::DenyReason) -> &'static str {
    match denial {
        collab::DenyReason::Unauthenticated => "unauthenticated",
        collab::DenyReason::NotReader => "not_reader",
        collab::DenyReason::NotPermitted => "not_permitted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collab::{DeliveryEligibility as Verdict, DeliveryState as State};
    use commonware_cryptography::Signer as _;

    fn eligible(state: State) -> Verdict {
        Verdict::Eligible {
            state,
            expires_at: 100,
            asked_at: 10,
        }
    }

    /// THE rule this whole read exists for. A receipt is a LOG: an
    /// `adapter_accepted` in it is a true statement about something that
    /// happened, and reading it as "so it may be delivered" is how a message
    /// gets handed to a model twice.
    #[test]
    fn a_historical_acceptance_is_not_permission_to_deliver_again() {
        assert_eq!(
            admits_delivery(&Verdict::Settled {
                state: State::AdapterAccepted
            }),
            Err("settled"),
            "an accepted record is history, not work"
        );
        assert_eq!(
            admits_delivery(&eligible(State::Stored)),
            Ok(()),
            "an admitted record no service has queued is the one thing to deliver"
        );
    }

    /// The deadline decides, and it is the network's. Every verdict that is not
    /// an eligible `Stored` names why it is not, and none of them is a maybe.
    #[test]
    fn every_verdict_but_a_stored_one_refuses_a_new_delivery() {
        let cases = [
            (
                Verdict::Expired {
                    expires_at: 10,
                    asked_at: 10,
                },
                Err("expired"),
            ),
            (Verdict::NotReplayable, Err("not_replayable")),
            (Verdict::Unbound, Err("unbound")),
            (Verdict::Unknown, Err("unknown")),
            // non-terminal, but not this pump's to move: a durable record
            // somewhere else already owns each of them.
            (eligible(State::Queued), Err("already_queued")),
            (eligible(State::Held), Err("held")),
            // the spec is explicit: never replayed automatically.
            (eligible(State::DeliveryUnknown), Err("not_replayable")),
            (eligible(State::Stored), Ok(())),
        ];
        for (verdict, expected) in cases {
            assert_eq!(
                admits_delivery(&verdict),
                expected,
                "{verdict:?} must answer {expected:?}"
            );
        }
    }

    /// A conversation's event stream carries everything that ever happened in
    /// it. Only mail addressed to THIS participant is work — a message to
    /// somebody else on the same roster is not this device's to deliver.
    #[test]
    fn only_mail_addressed_to_this_participant_is_work() {
        let events = vec![
            event(
                1,
                collab::EventBody::MessageAdmitted {
                    sender: "alice".into(),
                    recipient: "bob".into(),
                },
            ),
            event(
                2,
                collab::EventBody::MessageAdmitted {
                    sender: "bob".into(),
                    recipient: "carol".into(),
                },
            ),
            event(
                3,
                collab::EventBody::DeliveryAdvanced {
                    message_seq: 1,
                    state: State::Queued,
                    reason: None,
                },
            ),
            event(
                4,
                collab::EventBody::BindingChanged {
                    participant_id: "bob".into(),
                    credential: 7,
                    detached: false,
                },
            ),
            event(
                5,
                collab::EventBody::MessageAdmitted {
                    sender: "carol".into(),
                    recipient: "bob".into(),
                },
            ),
        ];
        assert_eq!(
            admitted_for("bob", &events),
            vec![1, 5],
            "carol's mail, our own receipts and a binding change are not deliveries"
        );
    }

    fn event(seq: u64, body: collab::EventBody) -> collab::ConversationEvent {
        collab::ConversationEvent { seq, at: seq, body }
    }

    /// The generation on a delivery is the RECIPIENT's binding, not the
    /// sender's credential. Confusing the two hands a message aimed at a
    /// replaced attachment to the device that replaced it — the exact thing the
    /// daemon's fence is there to stop.
    #[test]
    fn a_delivery_is_fenced_by_the_recipients_binding_and_not_the_senders() {
        let message = collab::Message {
            seq: 4,
            message_id: collab::MessageId {
                generation: 11,
                sequence: 3,
            },
            conversation_id: CONVERSATION.into(),
            sender: "alice".into(),
            recipient: "bob".into(),
            kind: collab::MessageKind::Question,
            reply_to: Some(2),
            task: None,
            body: "ready?".into(),
            references: vec![collab::Reference::Blob { hash: "ab".into() }],
            expires_at: 900,
            admitted_at: 50,
            digest: "d".into(),
        };
        let frame = deliver(&message, "bob", 42);
        assert_eq!(frame.binding_generation, 42, "the recipient's binding");
        assert_eq!(
            frame.message_id,
            wire::MessageId {
                generation: 11,
                sequence: 3
            },
            "the sender's credential stays where it belongs"
        );
        assert_eq!(frame.expires_at, 900, "the network's deadline, unconverted");
        assert_eq!(frame.network_now, 50, "the agreed clock at admission");
        assert!(
            !frame.urgent,
            "no committed field asks for a turn to be steered"
        );
        assert_eq!(
            frame.references,
            vec![wire::Reference::Blob { hash: "ab".into() }]
        );
    }

    // ---- committed collaboration -> node -> daemon -> committed receipt ----

    const NETWORK: &str = "pump-test#a1b2c3d4";
    const CONVERSATION: &str = "standup";
    const SENDER: &str = "alice";
    const RECIPIENT: &str = "bob";
    const DEVICE: &str = "laptop";
    const LINK_TOKEN: &str = "pump-test-link-token";
    /// well past the testkit's clock, which is one tick per committed block.
    const DEADLINE: u64 = 1_000_000;

    /// a node with the collaboration module live, a workspace holding this
    /// device's binding, and an agent daemon attached to the terminal plane.
    struct Fixture {
        daemon: noded::testkit::InProcDaemon,
        terminals: noded::TerminalSessions,
        /// what the "agent daemon" receives. The real one is a ws connection;
        /// this is the same lane, taken by the same `attach`.
        link: tokio::sync::mpsc::Receiver<wire::Command>,
        /// dropping it detaches the daemon, which is how a reconnect is staged.
        attached: Option<noded::AttachGuard>,
        owner: commonware_cryptography::ed25519::PrivateKey,
        service: commonware_cryptography::ed25519::PrivateKey,
        /// every `Acknowledge` a [`Fixture::flaky_times`] lane carried, in
        /// order, INCLUDING the ones it then refused. What landed is readable
        /// off the chain; only this says what was ATTEMPTED.
        attempts: Attempts,
        // dropped LAST: the daemon's actor thread closes qmdb on the way out.
        dir: tempfile::TempDir,
    }

    type Attempts = std::sync::Arc<std::sync::Mutex<Vec<collab::DeliveryState>>>;

    impl Fixture {
        /// stand the whole lane up, through the real committed ops.
        async fn start() -> Self {
            let dir = tempfile::Builder::new()
                .prefix("ducktape-collab-pump")
                .tempdir()
                .expect("workspace");
            let daemon = noded::testkit::InProcDaemon::start(
                || {
                    host::Host::genesis(vec![
                        Box::new(identity::Identity::new(
                            "identity",
                            Box::new(sdk_testkit::MemStore::new()),
                            NETWORK.into(),
                        )),
                        Box::new(attribution::AttributionModule::new(
                            "attribution",
                            Box::new(sdk_testkit::MemStore::new()),
                        )),
                        Box::new(tasks::Tasks::new(
                            "tasks",
                            "identity",
                            "attribution",
                            Box::new(sdk_testkit::MemStore::new()),
                        )),
                        Box::new(collab::Collaboration::new(
                            COLLABORATION,
                            "identity",
                            "tasks",
                            Box::new(sdk_testkit::MemStore::new()),
                            DEADLINE * 2,
                            NETWORK,
                        )),
                    ])
                    .expect("genesis")
                },
                vec![COLLABORATION.into()],
            );
            let hub = noded::StreamHub::with_log_ring(64, noded::LogRing::default());
            let terminals = noded::TerminalSessions::new(
                hub.terminals(),
                hub.term_commands(),
                Some(LINK_TOKEN.to_string()),
            );
            let (attached, link) = terminals.attach(LINK_TOKEN).expect("the link is free");

            let owner =
                commonware_cryptography::ed25519::PrivateKey::from_seed(rand::random::<u64>());
            // the binding's own key, minted the way `collab attach` mints it —
            // and RECORDED the same way, because the key file is named by a
            // digest and the pump could not read the ids back out of it.
            let binding = crate::collab_keys::BindingRef {
                network: NETWORK,
                conversation: CONVERSATION,
                participant: RECIPIENT,
            };
            let service = crate::collab_keys::ensure(dir.path(), binding).expect("mint");
            crate::collab_keys::remember(dir.path(), binding, DEVICE).expect("remember");

            let fixture = Fixture {
                daemon,
                terminals,
                link,
                attached: Some(attached),
                owner,
                service,
                attempts: Attempts::default(),
                dir,
            };
            fixture.compose().await;
            fixture
        }

        /// the committed setup: two participants, a conversation both sit on,
        /// and this device's binding — every one a REAL signed op.
        async fn compose(&self) {
            let service_key = commonware_cryptography::Signer::public_key(&self.service)
                .as_ref()
                .to_vec();
            for op in [
                collab::CollaborationMsg::RegisterParticipant {
                    participant_id: SENDER.into(),
                    display_name: "Alice".into(),
                    agent_account: None,
                },
                collab::CollaborationMsg::RegisterParticipant {
                    participant_id: RECIPIENT.into(),
                    display_name: "Bob".into(),
                    agent_account: None,
                },
                collab::CollaborationMsg::CreateConversation {
                    conversation_id: CONVERSATION.into(),
                    topic: "standup".into(),
                },
                collab::CollaborationMsg::SetRoster {
                    conversation_id: CONVERSATION.into(),
                    participant_id: SENDER.into(),
                    role: Some(collab::Role::Member),
                },
                collab::CollaborationMsg::SetRoster {
                    conversation_id: CONVERSATION.into(),
                    participant_id: RECIPIENT.into(),
                    role: Some(collab::Role::Member),
                },
                collab::CollaborationMsg::Bind {
                    conversation_id: CONVERSATION.into(),
                    participant_id: RECIPIENT.into(),
                    device: DEVICE.into(),
                    principal: collab::BoundPrincipal::ServiceKey(service_key),
                    expected_credential: 0,
                },
            ] {
                self.submit(&self.owner, op).await.expect("the owner's op");
            }
        }

        /// admit one message from alice to bob, as the owner of alice.
        async fn send(&self, sequence: u64, body: &str) {
            self.send_expiring(sequence, body, DEADLINE).await;
        }

        async fn send_expiring(&self, sequence: u64, body: &str, expires_at: u64) {
            let collab::CollaborationReply::Participant(alice) = self
                .read(
                    &self.owner,
                    SENDER,
                    None,
                    collab::ProtectedRead::Participant,
                )
                .await
            else {
                panic!("the owner reads its own participant");
            };
            self.submit(
                &self.owner,
                collab::CollaborationMsg::Send(collab::SendRequest {
                    conversation_id: CONVERSATION.into(),
                    sender_participant_id: SENDER.into(),
                    message_id: collab::MessageId {
                        generation: alice.owner_credential,
                        sequence,
                    },
                    recipient_participant_id: RECIPIENT.into(),
                    kind: collab::MessageKind::Question,
                    reply_to: None,
                    task: None,
                    body: body.into(),
                    references: Vec::new(),
                    expires_at,
                }),
            )
            .await
            .expect("alice's message is admitted");
        }

        async fn submit(
            &self,
            signer: &commonware_cryptography::ed25519::PrivateKey,
            op: collab::CollaborationMsg,
        ) -> Result<u64, String> {
            let request = collab::Request::new(NETWORK, op);
            let frame =
                crate::userkey_cli::user_frame(signer, COLLABORATION, collab::encode_msg(&request));
            let (reply, answer) = oneshot::channel();
            self.daemon
                .commands()
                .send(noded::NodeCommand::SubmitFrame { frame, reply })
                .await
                .expect("the actor takes a submission");
            answer
                .await
                .expect("the actor answers")
                .map(|block| block.height)
        }

        async fn read(
            &self,
            signer: &commonware_cryptography::ed25519::PrivateKey,
            participant: &str,
            via: Option<&str>,
            read: collab::ProtectedRead,
        ) -> collab::CollaborationReply {
            let query = collab::CollaborationQuery::Read {
                participant_id: participant.to_string(),
                via: via.map(str::to_string),
                read,
            };
            let (reply, answer) = oneshot::channel();
            self.daemon
                .commands()
                .send(noded::NodeCommand::QueryAs {
                    target: COLLABORATION.to_string(),
                    req: collab::encode_query(&query),
                    reader: commonware_cryptography::Signer::public_key(signer)
                        .as_ref()
                        .to_vec(),
                    reply,
                })
                .await
                .expect("the actor takes a read");
            let bytes = answer.await.expect("the actor answers").expect("read ok");
            collab::decode_reply(&bytes).expect("a module reply decodes")
        }

        /// bob's committed delivery record for one message.
        async fn receipt(&self, seq: u64) -> collab::Receipt {
            let collab::CollaborationReply::Receipt(receipt) = self
                .read(
                    &self.service,
                    RECIPIENT,
                    Some(CONVERSATION),
                    collab::ProtectedRead::Receipt {
                        conversation_id: CONVERSATION.into(),
                        seq,
                    },
                )
                .await
            else {
                panic!("the binding reads its own receipt");
            };
            receipt.expect("an admitted message has a receipt")
        }

        /// the agreed network time, as this node last settled it.
        fn now(&self) -> u64 {
            self.daemon.status().current().consensus_time
        }

        /// commit one ordinary op, which is what advances the agreed clock here.
        async fn tick(&self) {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.submit(
                &self.owner,
                collab::CollaborationMsg::CreateConversation {
                    conversation_id: format!("filler-{n}"),
                    topic: "advancing the agreed clock".into(),
                },
            )
            .await
            .expect("a block commits");
        }

        /// the oldest sequence the conversation still retains.
        async fn floor(&self) -> u64 {
            let collab::CollaborationReply::Conversation(conversation) = self
                .read(
                    &self.service,
                    RECIPIENT,
                    Some(CONVERSATION),
                    collab::ProtectedRead::Conversation {
                        conversation_id: CONVERSATION.into(),
                    },
                )
                .await
            else {
                panic!("the binding reads the conversation it is on");
            };
            conversation.floor_seq
        }

        /// the conversation sequence of the one message admitted for bob.
        async fn only_admitted_seq(&self) -> u64 {
            let reply = self
                .read(
                    &self.service,
                    RECIPIENT,
                    Some(CONVERSATION),
                    collab::ProtectedRead::Events {
                        conversation_id: CONVERSATION.into(),
                        // the floor, not zero: sequences start at 1 and reading
                        // below the floor answers `HistoryGap`, which is what
                        // the pump's own cursor is clamped up to.
                        from_seq: self.floor().await,
                        limit: PAGE,
                    },
                )
                .await;
            let collab::CollaborationReply::Events(collab::EventPage::Page { events, .. }) = reply
            else {
                panic!("the binding reads its own conversation, got {reply:?}");
            };
            let mut admitted = admitted_for(RECIPIENT, &events);
            assert_eq!(admitted.len(), 1, "this fixture admits exactly one message");
            admitted.remove(0)
        }

        fn pump(&self) -> Pump {
            self.pump_behind(self.daemon.commands())
        }

        fn pump_behind(&self, commands: mpsc::Sender<noded::NodeCommand>) -> Pump {
            Pump::new(
                commands,
                self.daemon.status(),
                self.terminals.clone(),
                self.dir.path().to_path_buf(),
                NETWORK.to_string(),
            )
        }

        /// A command lane that swallows the FIRST command matching `fault` and
        /// answers it as a failure, then forwards everything, including every
        /// retry of the swallowed one.
        ///
        /// The node stays real: this is the actor lane going wrong, which is
        /// exactly the transient class the pump has to survive — a dropped
        /// reply, a busy actor, a submission that did not land.
        fn flaky(&self, fault: Fault) -> mpsc::Sender<noded::NodeCommand> {
            self.flaky_times(fault, 1)
        }

        /// [`Fixture::flaky`], failing the first `times` matching commands.
        fn flaky_times(&self, fault: Fault, times: u32) -> mpsc::Sender<noded::NodeCommand> {
            use futures::StreamExt as _;
            let (tx, mut rx) = mpsc::channel(16);
            let mut real = self.daemon.commands();
            let attempts = self.attempts.clone();
            tokio::spawn(async move {
                let mut left = times;
                while let Some(command) = rx.next().await {
                    if let Some(state) = acknowledged(&command) {
                        attempts.lock().expect("attempts").push(state);
                    }
                    if left > 0 && fault.matches(&command) {
                        left -= 1;
                        fault.refuse(command);
                        continue;
                    }
                    if real.send(command).await.is_err() {
                        return;
                    }
                }
            });
            tx
        }

        /// a command lane that records every acknowledgement and fails none.
        fn watched(&self) -> mpsc::Sender<noded::NodeCommand> {
            self.flaky_times(Fault::Eligibility, 0)
        }

        /// every acknowledgement the pump TRIED to submit, in order.
        fn attempted(&self) -> Vec<collab::DeliveryState> {
            self.attempts.lock().expect("attempts").clone()
        }

        /// re-attach the "daemon" — the same bindings, a new connection.
        fn reconnect(&mut self) {
            self.attached.take();
            let (attached, link) = self
                .terminals
                .attach(LINK_TOKEN)
                .expect("the link is free once the last daemon let go");
            self.attached = Some(attached);
            self.link = link;
        }
    }

    /// which one command a [`Fixture::flaky`] lane fails.
    #[derive(Debug, Clone, Copy)]
    enum Fault {
        /// the eligibility question itself cannot be asked.
        Eligibility,
        /// the acknowledgement of an accepted delivery does not land.
        AcceptedReceipt,
        /// the acknowledgement that the daemon queued it does not land.
        QueuedReceipt,
    }

    impl Fault {
        fn matches(self, command: &noded::NodeCommand) -> bool {
            match (self, command) {
                (Fault::Eligibility, noded::NodeCommand::QueryAs { req, .. }) => matches!(
                    collab::decode_query(req),
                    Ok(collab::CollaborationQuery::Read {
                        read: collab::ProtectedRead::DeliveryEligibility { .. },
                        ..
                    })
                ),
                (
                    Fault::AcceptedReceipt | Fault::QueuedReceipt,
                    noded::NodeCommand::SubmitFrame { .. },
                ) => acknowledged(command) == Some(self.acknowledges()),
                _ => false,
            }
        }

        /// the one delivery state this fault refuses to let commit.
        fn acknowledges(self) -> collab::DeliveryState {
            match self {
                Fault::AcceptedReceipt => collab::DeliveryState::AdapterAccepted,
                Fault::QueuedReceipt => collab::DeliveryState::Queued,
                // never reached: `matches` only asks on a SubmitFrame arm.
                Fault::Eligibility => collab::DeliveryState::Stored,
            }
        }

        /// answer the swallowed command the way a node under strain does.
        fn refuse(self, command: noded::NodeCommand) {
            match command {
                noded::NodeCommand::QueryAs { reply, .. } => {
                    let _ = reply.send(Err("the actor is busy".to_string()));
                }
                noded::NodeCommand::Query { reply, .. } => {
                    let _ = reply.send(Err("the actor is busy".to_string()));
                }
                noded::NodeCommand::SubmitFrame { reply, .. } => {
                    let _ = reply.send(Err("the block was not produced".to_string()));
                }
                noded::NodeCommand::Submit { reply, .. } => {
                    let _ = reply.send(Err("the block was not produced".to_string()));
                }
            }
        }
    }

    /// THE end-to-end lane: an admitted message reaches the daemon through the
    /// node, the daemon's receipt comes back, and the network holds it.
    ///
    /// Every hop is the real one — signed frames into a real module, a real
    /// authenticated read as the binding's scoped key, the real terminal-plane
    /// link, the real committed clock. The only stand-in is the PROVIDER: this
    /// proves that what a daemon reports becomes a committed receipt, and
    /// nothing at all about a Claude or Codex session accepting input.
    ///
    /// `sweep` and `receipt` are awaited directly rather than driven through
    /// [`Pump::run`]'s select loop, because awaiting them IS the synchronization
    /// — when `receipt` returns, the acknowledgement's block has settled. A
    /// spawned pump would need the test to poll for that.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_admitted_message_reaches_the_daemon_and_its_receipt_reaches_the_chain() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "what is the status?").await;
        let pump = fixture.pump();
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let sent = drained(&mut fixture.link);
        let wire::Command::MsgTime { network_now } = sent[0] else {
            panic!("the daemon owns no clock; the node's is pushed first, got {sent:?}");
        };
        assert!(
            network_now > 0,
            "the agreed clock, as the node last settled it"
        );
        let (bound, deliver) = bind_and_delivery(&sent);
        assert_eq!(bound.participant, RECIPIENT);
        assert_eq!(bound.device, DEVICE, "the label the operator attached");
        assert_eq!(deliver.body, "what is the status?");
        assert_eq!(deliver.sender, SENDER);
        assert_eq!(
            deliver.binding_generation, bound.generation,
            "the delivery is fenced by the binding just announced"
        );
        assert_eq!(deliver.expires_at, DEADLINE);
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::Stored,
            "nothing has claimed it yet: the daemon has not answered"
        );

        // the daemon answers, twice, as a real one does: durably queued, then
        // the provider's own acceptance.
        for state in [wire::State::Queued, wire::State::AdapterAccepted] {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ))
            .await;
            assert_eq!(
                fixture.receipt(deliver.seq).await.state,
                delivery_state(state),
                "the daemon's report is on the chain when `receipt` returns"
            );
        }
        assert_eq!(
            fixture.receipt(deliver.seq).await.advanced_by,
            bound.generation,
            "the binding that carried it is the one on the record"
        );

        // and a second sweep does NOT hand it over again: the eligibility read
        // answers `settled`, and a historical acceptance is not a permission.
        pump.sweep(&mut seen).await;
        assert_no_delivery(
            &drained(&mut fixture.link),
            "an accepted message must never be offered to a provider twice",
        );
    }

    /// A message past its deadline is never handed to a provider. Decided by
    /// the NETWORK's clock through the eligibility read — this process never
    /// compares a deadline to anything of its own.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_message_past_its_deadline_is_never_offered() {
        let mut fixture = Fixture::start().await;
        // admitted with a live deadline — the module refuses one already past —
        // and then the network's own clock walks over it. The testkit's agreed
        // time is one tick per committed block, so a few ordinary ops do it.
        let deadline = fixture.now() + 2;
        fixture.send_expiring(1, "too late", deadline).await;
        let seq = fixture.only_admitted_seq().await;
        while fixture.now() < deadline {
            fixture.tick().await;
        }
        let pump = fixture.pump();

        pump.sweep(&mut Seen::default()).await;
        let sent = drained(&mut fixture.link);
        assert_no_delivery(
            &sent,
            "a message past its deadline must never reach a provider",
        );
        assert!(
            sent.iter()
                .any(|command| matches!(command, wire::Command::MsgBind(_))),
            "the binding is still announced: it is live, the message is not"
        );
        assert_eq!(
            fixture.receipt(seq).await.state,
            collab::DeliveryState::Stored,
            "and nothing was claimed on its behalf"
        );
    }

    /// A message the daemon says it is HOLDING stays held, and is never offered
    /// again — a hold is a barrier being exposed, not a delivery failure to
    /// retry. The release, when it comes, is a second receipt on the same key.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_message_is_not_re_offered_and_its_release_is_recorded() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "may i deploy?").await;
        let pump = fixture.pump();
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));
        for state in [wire::State::Queued, wire::State::Held] {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ))
            .await;
        }
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::Held
        );

        pump.sweep(&mut seen).await;
        assert_no_delivery(
            &drained(&mut fixture.link),
            "a held message is at the provider already; re-offering duplicates the instruction",
        );

        // the barrier releases. `Held -> AdapterAccepted` is in the module's
        // diagram, and this is the observation the daemon's hold-follow makes.
        pump.receipt(reported(
            deliver.seq,
            bound.generation,
            deliver.message_id,
            wire::State::AdapterAccepted,
        ))
        .await;
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::AdapterAccepted,
            "a released hold reaches the chain as an acceptance"
        );
    }

    /// The terminal plane's receipts reach the pump's lane and not the floor.
    /// The wiring `route_collab_to` installs, on its own — awaited, never polled.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_terminal_planes_collaboration_receipts_reach_the_pumps_lane() {
        let hub = noded::StreamHub::with_log_ring(64, noded::LogRing::default());
        let terminals = noded::TerminalSessions::new(
            hub.terminals(),
            hub.term_commands(),
            Some(LINK_TOKEN.to_string()),
        );
        let (receipts, mut lane) = tokio::sync::mpsc::channel(4);
        assert!(terminals.route_collab_to(receipts), "the lane was free");
        let (second, _unused) = tokio::sync::mpsc::channel(4);
        assert!(
            !terminals.route_collab_to(second),
            "a second consumer would acknowledge every receipt twice"
        );

        terminals.on_event(wire::Event::MsgBindRefused {
            conversation: CONVERSATION.into(),
            participant: RECIPIENT.into(),
            generation: 3,
            reason: wire::BindRefusal::UnknownDevice,
        });
        let got = lane.recv().await.expect("the plane holds the lane open");
        assert!(
            matches!(got, wire::Event::MsgBindRefused { generation: 3, .. }),
            "got {got:?}"
        );
    }

    /// `run` ends when its wake lane closes, so a node shutting down does not
    /// leave a task holding the actor's command channel open.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_pump_stops_when_its_wake_lane_closes() {
        let fixture = Fixture::start().await;
        let (wake, wake_rx) = tokio::sync::mpsc::channel(1);
        let (_receipts, receipt_rx) = tokio::sync::mpsc::channel(1);
        let running = tokio::spawn(fixture.pump().run(receipt_rx, wake_rx));
        drop(wake);
        running.await.expect("the pump returns rather than hanging");
    }

    /// everything the pump handed the daemon in the sweep that just returned.
    /// `try_recv` is exact and not a race: a sweep sends every command before it
    /// returns, and sends nothing after.
    fn drained(link: &mut tokio::sync::mpsc::Receiver<wire::Command>) -> Vec<wire::Command> {
        std::iter::from_fn(|| link.try_recv().ok()).collect()
    }

    /// A `MsgTime` on every sweep is expected — the agreed clock advances with
    /// every block a test commits. What must not appear is a second handover.
    fn assert_no_delivery(sent: &[wire::Command], why: &str) {
        assert!(
            !sent
                .iter()
                .any(|command| matches!(command, wire::Command::MsgDeliver(_))),
            "{why}, got {sent:?}"
        );
    }

    fn bind_and_delivery(sent: &[wire::Command]) -> (wire::Bind, Box<wire::Deliver>) {
        let mut bound = None;
        let mut delivered = None;
        for command in sent {
            match command {
                wire::Command::MsgBind(bind) => bound = Some(bind.clone()),
                wire::Command::MsgDeliver(deliver) => delivered = Some(deliver.clone()),
                wire::Command::MsgTime { .. } | wire::Command::MsgRetain { .. } => {}
                other => panic!("the pump drives no pty: {other:?}"),
            }
        }
        (
            bound.unwrap_or_else(|| panic!("a binding is announced, got {sent:?}")),
            delivered.unwrap_or_else(|| panic!("the message is handed over, got {sent:?}")),
        )
    }

    /// An eligibility read that fails is NOT a refusal. The message stays in
    /// front of the cursor and the next sweep asks again — otherwise a busy
    /// actor for one round trip silently loses a queued message forever, because
    /// nothing ever reads that sequence a second time.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_message_whose_eligibility_cannot_be_read_is_held_and_delivered_next_sweep() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "did the read land?").await;
        let pump = fixture.pump_behind(fixture.flaky(Fault::Eligibility));
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        assert_no_delivery(
            &drained(&mut fixture.link),
            "an unanswered eligibility read is not permission to deliver",
        );

        // the very next sweep, with nothing else changed: no re-bind, no new
        // message, no operator action.
        pump.sweep(&mut seen).await;
        let sent = drained(&mut fixture.link);
        let delivered = sent
            .iter()
            .find_map(|command| match command {
                wire::Command::MsgDeliver(deliver) => Some(deliver),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the held message comes back, got {sent:?}"));
        assert_eq!(
            delivered.body, "did the read land?",
            "the held message is the one that comes back"
        );
        assert!(
            !sent
                .iter()
                .any(|command| matches!(command, wire::Command::MsgBind(_))),
            "and no re-bind: nothing about the binding changed, only the read"
        );
    }

    /// A receipt the chain did not take is retried, and the retry needs no
    /// provider re-delivery and no re-bind.
    ///
    /// Without it the network reads `Queued` forever for a message a provider
    /// accepted: `Queued` is committed, so eligibility answers `already_queued`
    /// and the pump never re-offers it, and the daemon has no reason to report
    /// it again.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_acknowledgement_the_chain_did_not_take_is_retried_on_the_next_sweep() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "please accept me").await;
        let pump = fixture.pump_behind(fixture.flaky(Fault::AcceptedReceipt));
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));
        for state in [wire::State::Queued, wire::State::AdapterAccepted] {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ))
            .await;
        }
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::Queued,
            "the acceptance did not land; the record still says what did"
        );

        pump.sweep(&mut seen).await;
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::AdapterAccepted,
            "the sweep re-submits it, with no provider and no binding involved"
        );
        assert_no_delivery(
            &drained(&mut fixture.link),
            "recovering a receipt must not re-offer the message it is about",
        );
    }

    /// A receipt is owed until the CHAIN says otherwise, however long that
    /// takes. There is no attempt counter to run out: a count cannot tell a
    /// busy actor from a permanent refusal, so counting means eventually
    /// throwing away a fact that was merely unlucky.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_receipt_survives_far_more_transient_failures_than_a_retry_budget_would() {
        const FAILURES: u32 = 9;
        let mut fixture = Fixture::start().await;
        fixture.send(1, "keep trying").await;
        let pump = fixture.pump_behind(fixture.flaky_times(Fault::AcceptedReceipt, FAILURES));
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));
        for state in [wire::State::Queued, wire::State::AdapterAccepted] {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ))
            .await;
        }

        // one submission attempt per sweep, and the first FAILURES of them are
        // swallowed. Nothing else happens in between: no provider, no re-bind.
        for _ in 0..FAILURES {
            assert_eq!(
                fixture.receipt(deliver.seq).await.state,
                collab::DeliveryState::Queued,
                "still owed"
            );
            pump.sweep(&mut seen).await;
        }
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::AdapterAccepted,
            "the acceptance lands as soon as the chain will take it"
        );
        assert_no_delivery(
            &drained(&mut fixture.link),
            "and no provider was asked to do anything again",
        );
    }

    /// A later transition cannot stand in for an earlier one: the diagram has
    /// no `Stored -> AdapterAccepted` edge. So while `Queued` is still owed, the
    /// `AdapterAccepted` behind it must WAIT rather than race past it — and both
    /// must land, in order, once the chain takes them.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_acceptance_waits_behind_the_queued_receipt_it_follows() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "order matters").await;
        let pump = fixture.pump_behind(fixture.flaky(Fault::QueuedReceipt));
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));
        // `Queued` is swallowed; `AdapterAccepted` arrives while it is owed.
        for state in [wire::State::Queued, wire::State::AdapterAccepted] {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ))
            .await;
        }
        // the state alone cannot show this: an acceptance that DID jump the
        // queue would be refused out of `Stored` and leave the record reading
        // `Stored` too. What was ATTEMPTED is the discriminating fact.
        assert_eq!(
            fixture.attempted(),
            vec![collab::DeliveryState::Queued],
            "only the transition the record can actually take was submitted"
        );
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::Stored,
            "and it was the swallowed one, so nothing committed"
        );

        pump.sweep(&mut seen).await;
        assert_eq!(
            fixture.attempted(),
            vec![
                collab::DeliveryState::Queued,
                collab::DeliveryState::Queued,
                collab::DeliveryState::AdapterAccepted,
            ],
            "the retry re-submits the earlier one FIRST, then the one behind it"
        );
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::AdapterAccepted,
            "both land, in the order the daemon reported them"
        );
        assert_no_delivery(
            &drained(&mut fixture.link),
            "recovering an ordered chain must not re-offer the message",
        );
    }

    /// The ceiling on owed receipts is BACKPRESSURE, not eviction: it stops the
    /// pump handing the daemon anything NEW, and the message it held back is
    /// still in front of the cursor for the sweep after the backlog drains.
    ///
    /// [`Pump::pump_one`] rather than a sweep, because a sweep would first try
    /// to resubmit all [`MAX_OWING_MESSAGES`] debts against a real chain.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_pump_this_far_behind_offers_the_daemon_nothing_new() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "not while you are this far behind").await;
        let pump = fixture.pump();
        let attachment = Attached {
            network: NETWORK.into(),
            conversation: CONVERSATION.into(),
            participant: RECIPIENT.into(),
            device: DEVICE.into(),
        };
        let mut announced = Announced::default();

        // owed to the ceiling, by messages this one has nothing to do with.
        {
            let mut owed = pump.owed.lock().expect("collab owed lock poisoned");
            for seq in 0..MAX_OWING_MESSAGES as u64 {
                owed.insert(
                    (CONVERSATION.into(), RECIPIENT.into(), 1_000_000 + seq),
                    [Unsent {
                        credential: 1,
                        state: wire::State::Queued,
                        reason: None,
                    }]
                    .into(),
                );
            }
        }
        pump.pump_one(&attachment, &mut announced).await;
        assert_no_delivery(
            &drained(&mut fixture.link),
            "a pump at the ceiling hands over no new message",
        );

        // the backlog clears, and the message it held back goes out.
        pump.owed.lock().expect("collab owed lock poisoned").clear();
        pump.pump_one(&attachment, &mut announced).await;
        let sent = drained(&mut fixture.link);
        assert!(
            sent.iter()
                .any(|command| matches!(command, wire::Command::MsgDeliver(_))),
            "the cursor never passed it, so it is still there to offer: {sent:?}"
        );
    }

    /// A daemon replaying its journal reports the same transition again. The
    /// same fact told twice is one debt, not two — otherwise a replay would
    /// grow one message's chain without bound, and every copy would be
    /// submitted separately for the module to refuse.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_transition_reported_again_is_owed_once() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "say that again").await;
        let pump = fixture.pump_behind(fixture.flaky(Fault::QueuedReceipt));
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));
        for _ in 0..3 {
            pump.receipt(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                wire::State::Queued,
            ))
            .await;
        }
        assert_eq!(
            owing(&pump, deliver.seq),
            vec![wire::State::Queued],
            "three reports of one transition are one debt"
        );
        assert_eq!(
            fixture.attempted(),
            vec![collab::DeliveryState::Queued],
            "and one submission, not three"
        );

        pump.sweep(&mut seen).await;
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::Queued,
            "it lands once the chain will take it, and the debt is gone"
        );
        assert!(owing(&pump, deliver.seq).is_empty(), "nothing left owed");
    }

    /// A receipt the terminal plane could not hand over is GONE. Re-reading the
    /// chain cannot recover it — the chain's record is precisely what was never
    /// written — so the drop is COUNTED, the pump asks the daemon to replay its
    /// durable journal, and the transition settles on what comes back.
    ///
    /// The journal reports a message's CURRENT state, so the replay of a
    /// message whose `Queued` was the receipt that went missing arrives as an
    /// `AdapterAccepted` the record cannot take out of `Stored`. The pump reads
    /// that back and puts the missing `Queued` in front of it: the ONE bridge
    /// the delivery diagram offers, and no provider is asked for anything.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_receipt_a_full_lane_dropped_is_recovered_by_a_journal_replay() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "the lane is full").await;
        let pump = fixture.pump_behind(fixture.watched());
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (bound, deliver) = bind_and_delivery(&drained(&mut fixture.link));

        // a one-deep lane nobody drains. The first receipt occupies it and
        // never reaches the pump; the second has nowhere at all to go.
        let (receipts, _undrained) = lane::channel(1);
        assert!(
            fixture.terminals.route_collab_to(receipts),
            "the pump's lane is free"
        );
        for state in [wire::State::Queued, wire::State::AdapterAccepted] {
            fixture.terminals.on_event(reported(
                deliver.seq,
                bound.generation,
                deliver.message_id,
                state,
            ));
        }
        assert_eq!(
            fixture.terminals.dropped_receipts(),
            1,
            "the acceptance was dropped rather than wedging the pty plane"
        );

        pump.sweep(&mut seen).await;
        let asked: Vec<_> = drained(&mut fixture.link)
            .into_iter()
            .filter(|command| matches!(command, wire::Command::MsgReplay { .. }))
            .collect();
        assert_eq!(
            asked.len(),
            1,
            "the pump noticed the drop and asked for the journal: {asked:?}"
        );

        // the journal answers with where the message ACTUALLY is.
        pump.receipt(reported(
            deliver.seq,
            bound.generation,
            deliver.message_id,
            wire::State::AdapterAccepted,
        ))
        .await;
        assert_eq!(
            owing(&pump, deliver.seq),
            vec![wire::State::Queued, wire::State::AdapterAccepted],
            "the record is behind, so the missing transition goes in front"
        );

        pump.sweep(&mut seen).await;
        assert_eq!(
            fixture.receipt(deliver.seq).await.state,
            collab::DeliveryState::AdapterAccepted,
            "and the network holds what the provider actually did"
        );
        assert_eq!(
            fixture.attempted(),
            vec![
                collab::DeliveryState::AdapterAccepted,
                collab::DeliveryState::Queued,
                collab::DeliveryState::AdapterAccepted,
            ],
            "one refused attempt, then the pair in the order the diagram admits"
        );
        assert_no_delivery(
            &drained(&mut fixture.link),
            "a replay recovers a receipt; it never re-offers the message",
        );
    }

    /// what one message still owes the chain, as states in order.
    fn owing(pump: &Pump, seq: u64) -> Vec<wire::State> {
        pump.owed
            .lock()
            .expect("collab owed lock poisoned")
            .get(&(CONVERSATION.into(), RECIPIENT.into(), seq))
            .into_iter()
            .flatten()
            .map(|unsent| unsent.state)
            .collect()
    }

    /// A daemon that dies and redials knows nothing of what the last one was
    /// told. "Is one attached" cannot see that — the two look identical — so the
    /// pump keys its cache on WHICH daemon, and re-announces everything.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_reconnecting_daemon_is_told_everything_again() {
        let mut fixture = Fixture::start().await;
        fixture.send(1, "hello").await;
        let pump = fixture.pump();
        let mut seen = Seen::default();

        pump.sweep(&mut seen).await;
        let (first, _) = bind_and_delivery(&drained(&mut fixture.link));

        // the daemon goes away and comes back. Same bindings, same credential,
        // same conversation — nothing on the NETWORK changed.
        fixture.reconnect();
        pump.sweep(&mut seen).await;
        let sent = drained(&mut fixture.link);
        let (second, redelivered) = bind_and_delivery(&sent);
        assert_eq!(
            second.generation, first.generation,
            "the credential did not change; the daemon did"
        );
        assert_eq!(
            redelivered.body, "hello",
            "a message the last daemon was told about is told to this one"
        );
        assert!(
            sent.iter()
                .any(|command| matches!(command, wire::Command::MsgTime { .. })),
            "and the clock, which a fresh daemon also does not have: {sent:?}"
        );
    }

    /// The delivery state one node command acknowledges, if it acknowledges
    /// one. `None` for everything else on the lane — reads, and any other
    /// collaboration op.
    fn acknowledged(command: &noded::NodeCommand) -> Option<collab::DeliveryState> {
        let noded::NodeCommand::SubmitFrame { frame, .. } = command else {
            return None;
        };
        let (_, msg) = node::decode_frame(frame).ok()?;
        let collab::CollaborationMsg::Acknowledge { state, .. } =
            collab::decode_msg(&msg.payload).ok()?.op
        else {
            return None;
        };
        Some(state)
    }

    /// one delivery state as the daemon reports it.
    fn reported(
        seq: u64,
        generation: u64,
        message_id: wire::MessageId,
        state: wire::State,
    ) -> wire::Event {
        wire::Event::MsgDelivery {
            conversation: CONVERSATION.into(),
            participant: RECIPIENT.into(),
            seq,
            binding_generation: generation,
            sender: SENDER.into(),
            message_id,
            state,
            reason: None,
        }
    }
}
