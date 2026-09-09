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

/// Everything one pump run remembers between sweeps.
#[derive(Debug, Default)]
struct Seen {
    /// the agreed clock value last sent as a `MsgTime`.
    time: u64,
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
                // this page handed something over. STOP: the cursor stays on the
                // first one delivered, and the sweep after the daemon's receipts
                // commit reads it again and walks past.
                Some(offered) => {
                    announced.cursor = offered;
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
    /// the daemon. Answers the sequence of the FIRST one handed over, or `None`
    /// when the page produced no delivery.
    ///
    /// The cursor stops there rather than advancing past it. A delivered message
    /// stays `Stored` until the daemon's `Queued` receipt commits a block later,
    /// and a cursor past it would be a message nothing looks at again if the
    /// link died between the two. Re-reading one page until the receipt lands is
    /// the whole cost of never losing one.
    async fn offer_page(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        credential: collab::Credential,
        events: &[collab::ConversationEvent],
        messages: &[collab::Message],
    ) -> Option<u64> {
        let mut offered = None;
        for seq in admitted_for(&attachment.participant, events) {
            let Some(message) = messages.iter().find(|message| message.seq == seq) else {
                // the event survives its message: the body was pruned out from
                // under the page. There is nothing to deliver and never will be.
                self.skip(attachment, "message_pruned");
                continue;
            };
            if !self.eligible(attachment, key, seq).await {
                continue;
            }
            offered = Some(offered.map_or(seq, |first: u64| first.min(seq)));
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
        offered
    }

    /// May a NEW delivery attempt be made for this message, right now?
    ///
    /// Asked immediately before the handover and answered against the block's
    /// agreed time. Only a `Stored` record is work: every other state either
    /// belongs to a durable record this pump does not own, or is settled.
    async fn eligible(
        &self,
        attachment: &Attached,
        key: &commonware_cryptography::ed25519::PrivateKey,
        seq: u64,
    ) -> bool {
        let Some(collab::CollaborationReply::Eligibility(verdict)) = self
            .read(
                attachment,
                key,
                collab::ProtectedRead::DeliveryEligibility {
                    conversation_id: attachment.conversation.clone(),
                    seq,
                },
            )
            .await
        else {
            // a refused, undecodable or unanswered read is NOT permission. It
            // reads as ineligible, which costs a re-read next sweep.
            return false;
        };
        let Err(reason) = admits_delivery(&verdict) else {
            return true;
        };
        tracing::debug!(
            target: "ducktape::collab",
            conversation = %attachment.conversation,
            seq,
            reason,
            "not offering a message to the daemon"
        );
        false
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
        let binding = crate::collab_keys::BindingRef {
            network: &self.network,
            conversation,
            participant,
        };
        let key = match crate::collab_keys::load(&self.workspace, binding) {
            Ok(Some(key)) => key,
            Ok(None) => {
                return self.refuse(
                    "receipt_without_key",
                    "a receipt named a binding this device holds no service key for",
                );
            }
            Err(error) => return self.refuse("service_key_unreadable", &error),
        };
        let op = collab::CollaborationMsg::Acknowledge {
            conversation_id: conversation.to_string(),
            seq,
            binding_credential: binding_generation,
            state: delivery_state(state),
            reason,
        };
        match self.submit(&key, op).await {
            Ok(height) => tracing::debug!(
                target: "ducktape::collab",
                conversation = %conversation,
                seq,
                height,
                "acknowledged a delivery on-chain"
            ),
            // NOT retried. The module refuses a transition that is not in the
            // delivery diagram and a credential that is not current, and both
            // refusals are permanent: re-submitting the identical op produces
            // the identical refusal. A transport failure is re-derivable — the
            // daemon's journal is durable and its state is re-reported on the
            // next bind.
            Err(error) => self.refuse("acknowledge_refused", &error),
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
        _attached: noded::AttachGuard,
        owner: commonware_cryptography::ed25519::PrivateKey,
        service: commonware_cryptography::ed25519::PrivateKey,
        // dropped LAST: the daemon's actor thread closes qmdb on the way out.
        dir: tempfile::TempDir,
    }

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
                _attached: attached,
                owner,
                service,
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
            Pump::new(
                self.daemon.commands(),
                self.daemon.status(),
                self.terminals.clone(),
                self.dir.path().to_path_buf(),
                NETWORK.to_string(),
            )
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
