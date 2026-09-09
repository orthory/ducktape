//! the `collaboration` module's public wire surface — types only.
//!
//! This is the network side of agent messaging: participants, conversations,
//! bindings, immutable messages and their per-recipient delivery receipts. It
//! owns NO scheduler — a message that references a task references [`tasks`]'s
//! job record and is fenced against that record's current attempt.
//!
//! Two rules run through the whole surface:
//!
//! * **Identity comes from the authenticated envelope, never the payload.**
//!   A write resolves its actor from `sdk::Env::origin`; a read resolves its
//!   caller from the query context's origin. `sender_participant_id` and
//!   `CollaborationQuery::Read::participant_id` say which of the caller's own
//!   identities it is acting as, and the module verifies that claim.
//! * **Every credential owns its own sequence space.** A [`MessageId`] names
//!   a [`Credential`] number, and one participant attached to two
//!   conversations on two devices holds two of them — so two services both
//!   starting at sequence 1 never collide.

pub use attribution::Actor as Party;
use sdk::genesis_config::TimeUnit;
use sdk::AccountNumber;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---- bounds ---------------------------------------------------------------

/// Longest UTF-8 message body admitted. Oversized input is REFUSED at
/// admission, never truncated.
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// Most references one message may carry.
pub const MAX_REFERENCES: usize = 16;
/// Longest single reference (kind and value together).
pub const MAX_REFERENCE_BYTES: usize = 1024;
/// Ceiling on one encoded [`Message`] record.
pub const MAX_ENCODED_MESSAGE_BYTES: usize = 32 * 1024;
/// Longest identifier (participant, conversation, task, device label).
pub const MAX_ID_BYTES: usize = 128;
/// Longest display name / topic.
pub const MAX_LABEL_BYTES: usize = 256;
/// Most participants one conversation roster holds.
pub const MAX_ROSTER: usize = 64;
/// Longest bound service key. Wide enough for every public key an
/// `Origin::External` carries today and for a certificate-sized one, narrow
/// enough that a binding record stays bounded.
pub const MAX_SERVICE_KEY_BYTES: usize = 128;
/// Undelivered messages one participant mailbox holds, checked at admission
/// INCLUDING while that participant is disconnected. Replacing a binding does
/// not reset it.
pub const MAX_MAILBOX_UNDELIVERED: u64 = 256;
/// Encoded queued payload one participant mailbox holds, same accounting.
pub const MAX_MAILBOX_QUEUED_BYTES: u64 = 2 * 1024 * 1024;
/// Undelivered messages ONE sender may hold in ONE recipient's mailbox: the
/// per-sender admission quota that stops one participant filling another's
/// queue indefinitely.
pub const MAX_UNDELIVERED_PER_SENDER: u64 = 64;
/// Longest event page one [`ProtectedRead::Events`] answers.
pub const MAX_PAGE_LIMIT: u64 = 64;

/// The spec's longest delivery deadline, in SECONDS: seven days.
///
/// Seconds, because a deadline is a duration and `consensus_time` is not —
/// it carries a per-network unit (block height on the validator and replica
/// lanes, a millisecond epoch clock on the sim lane), so one raw number cannot
/// mean seven days on both. The network states its unit as the `time_unit`
/// genesis parameter, and [`max_delivery_ttl`] turns this duration into that
/// lane's ceiling. A sender states an ABSOLUTE `expires_at` in the same unit —
/// the split `bin/node`'s `expiry_from_clock` already uses for identity
/// consents.
pub const MAX_DELIVERY_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;

/// [`MAX_DELIVERY_TTL_SECONDS`] in `unit`'s consensus-time units: the ceiling
/// [`crate::Collaboration::new`] takes. Seven days is 604_800 height units at
/// a one-block-per-second heartbeat and 604_800_000 millisecond ones, and
/// neither overflows.
pub const fn max_delivery_ttl(unit: TimeUnit) -> u64 {
    MAX_DELIVERY_TTL_SECONDS * unit.per_second()
}

/// A credential's number inside one participant. Allocated monotonically and
/// never reused, so a replaced binding's credential can never come back.
///
/// The participant's owner key admits under [`Participant::owner_credential`];
/// every [`Binding`] carries its own. This number is the `generation` half of
/// a [`MessageId`], which is what gives each attached device an independent
/// sequence space.
pub type Credential = u64;

// ---- identity and addressing ----------------------------------------------

/// An owner-authorized collaboration identity. Independent of any process: a
/// participant outlives the run, session or device that speaks for it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub id: String,
    /// The authenticated party that registered it, and the only one that may
    /// issue its credentials or revoke it.
    pub owner: Party,
    /// Discovery label. NOT unique, never resolved for a mutation.
    pub display_name: String,
    /// An existing agent account this participant speaks for, when one was
    /// named at registration. Carrying it grants no authority of its own.
    pub agent_account: Option<AccountNumber>,
    /// The credential the owner's own key admits messages under.
    pub owner_credential: Credential,
    /// The next credential number to hand a scoped binding. Monotonic; a
    /// number it has issued is never issued again.
    pub next_credential: Credential,
    /// A revoked participant admits nothing and receives nothing, on every
    /// credential at once. Its history stays readable — revocation fences the
    /// future, it does not rewrite the past.
    pub revoked: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

/// What a roster entry may do. An observer subscribes without becoming an
/// input owner.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Role {
    Member,
    Observer,
}

/// An explicit participant roster and a shared topic, with its own committed
/// event sequence. Independent of individual runs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub id: String,
    pub topic: String,
    /// The authenticated party that created it: the roster's sole editor.
    pub owner: Party,
    /// participant id -> role, at most [`MAX_ROSTER`] entries.
    pub roster: BTreeMap<String, Role>,
    /// The next COMMITTED EVENT SEQUENCE to be assigned. One space for every
    /// committed change — message admissions, delivery transitions, binding
    /// replacements and roster edits alike — so a consumer resuming after its
    /// last acknowledged sequence sees a `Held` or `AdapterAccepted` update,
    /// not only new mail.
    pub next_seq: u64,
    /// The oldest sequence still retained. A consumer reading from below it
    /// gets [`EventPage::HistoryGap`] and resyncs; it never advances its
    /// cursor while silently losing actionable events.
    pub floor_seq: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

/// WHO a binding authorizes. A CLOSED set, and `Origin::Module` is
/// deliberately not in it: a module origin names the module in the middle, not
/// a principal, so admitting one would let every caller inside that module act
/// as any participant that had ever bound it.
///
/// Each arm is matched against the ORIGIN the host minted, never against
/// anything on the wire:
///
/// * [`BoundPrincipal::ServiceKey`] ← `Origin::External(key)`. The
///   owner-issued, conversation-scoped key a local adapter signs with. It is
///   deliberately NOT the owner's account key, so the messaging agent never
///   needs that key on the device.
/// * [`BoundPrincipal::Program`] ← `Origin::Program(account)`. Only the
///   dispatch CALL lane mints that origin, and only after `identity` proves
///   the account is `Control::Program { executor }` executed by the requesting
///   module at an unmoved generation. So an agent reaching this module carries
///   TWO independent authorizations — identity's, and the owner's binding —
///   and neither of them is a module vouching for itself.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BoundPrincipal {
    ServiceKey(Vec<u8>),
    Program(AccountNumber),
}

/// A principal as a READ exposes it. A service key is a credential its holder
/// already has and nobody reads back, so it is reported by SHAPE only; a
/// program account is a public number and is named.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PrincipalView {
    ServiceKey,
    Program(AccountNumber),
}

impl From<&BoundPrincipal> for PrincipalView {
    fn from(principal: &BoundPrincipal) -> Self {
        match principal {
            BoundPrincipal::ServiceKey(_) => Self::ServiceKey,
            BoundPrincipal::Program(account) => Self::Program(*account),
        }
    }
}

/// A participant's connection to ONE local provider session on ONE device, for
/// ONE conversation. Two devices cannot both hold it: a replacement must name
/// the credential it expects to replace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub conversation_id: String,
    pub participant_id: String,
    /// This attachment's credential — its fence AND its message sequence
    /// space. Drawn from [`Participant::next_credential`], so it is unique
    /// across every conversation the participant is attached to: two devices
    /// both starting at sequence 1 write different dedup keys.
    pub credential: Credential,
    /// WHO this attachment authorizes to send, acknowledge and read under
    /// [`Binding::credential`] — a scoped service key, or a program account
    /// reached over the call lane. See [`BoundPrincipal`].
    pub principal: BoundPrincipal,
    /// An opaque operator-chosen device label. NEVER a socket path, a URL, a
    /// provider session id or any credential — those stay local by
    /// construction.
    pub device: String,
    pub attached_at: u64,
    /// A detached binding is fenced: its key can neither send, receive nor
    /// acknowledge, and its credential number is spent for good.
    pub detached: bool,
}

/// A binding as a READ exposes it — never the scoped service key itself. The
/// key is a credential: its holder already has it, and nobody reads it back.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingView {
    pub conversation_id: String,
    pub participant_id: String,
    pub credential: Credential,
    pub principal: PrincipalView,
    pub device: String,
    pub attached_at: u64,
    pub detached: bool,
}

impl From<&Binding> for BindingView {
    fn from(binding: &Binding) -> Self {
        Self {
            conversation_id: binding.conversation_id.clone(),
            participant_id: binding.participant_id.clone(),
            credential: binding.credential,
            principal: PrincipalView::from(&binding.principal),
            device: binding.device.clone(),
            attached_at: binding.attached_at,
            detached: binding.detached,
        }
    }
}

// ---- messages -------------------------------------------------------------

/// The sender-scoped identity of one message: the credential that admitted it
/// and that CREDENTIAL's monotonic sequence. Retrying a message reuses this
/// pair; a relay MUST NOT mint a fresh one.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct MessageId {
    /// The [`Credential`] the sender is authenticating with.
    pub generation: Credential,
    /// Monotonic within that credential, and at most [`MAX_SEQUENCE`].
    pub sequence: u64,
}

/// The highest sequence one credential may admit.
///
/// It stops one short of `u64::MAX` on purpose: retention advances a
/// credential's replay floor to `sequence + 1`, so admitting the last
/// representable sequence would leave no floor above it. A credential that
/// reaches this rotates — the owner issues a fresh binding, which draws a
/// fresh credential with its own empty sequence space.
pub const MAX_SEQUENCE: u64 = u64::MAX - 1;

/// What a message is for. Recording or delivering a [`MessageKind::TaskRequest`]
/// does NOT claim the task — acceptance runs through the existing task
/// claim/dispatch authority.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageKind {
    Notice,
    Question,
    TaskRequest,
    TaskUpdate,
    Result,
}

/// A task reference and the execution attempt the sender meant. A message
/// naming a superseded attempt is refused as stale rather than delivered into
/// the new one.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskRef {
    /// A `tasks` job id. This module reads that record's authority; it never
    /// creates, claims or schedules one.
    pub id: String,
    pub expected_attempt: u64,
}

/// One reference a message carries. A CLOSED set, structurally validated:
/// immutable commit and blob references, or a scoped `duck://` link — exactly
/// what the spec admits.
///
/// The set is closed on purpose rather than being a `{kind, value}` pair with
/// a blocklist. A free-form value admits `file:///tmp/private`, a mutable
/// branch name and every scheme nobody thought of; there is no arm for those
/// here, so the whole class is unreachable instead of filtered.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Reference {
    /// A Forge commit by its immutable object id — 40 lowercase hex. A branch
    /// or tag name moves, so it is not a reference and has no arm.
    Commit { repo: String, commit: String },
    /// Content-addressed bytes: lowercase hex sha256.
    Blob { hash: String },
    /// A scoped `duck://` link. Sending one does not grant its recipient read
    /// access — the recipient fetches under its own authority.
    Duck { url: String },
}

/// The scheme a [`Reference::Duck`] must carry.
pub const DUCK_SCHEME: &str = "duck://";
/// Length of a git object id in lowercase hex.
pub const COMMIT_HEX_LEN: usize = 40;
/// Length of a sha256 digest in lowercase hex.
pub const BLOB_HEX_LEN: usize = 64;

/// One admitted message. Immutable: a correction or withdrawal is a NEW
/// message referencing this one through `reply_to`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// The conversation event sequence this admission occupies. Receipts and
    /// `reply_to` name it.
    pub seq: u64,
    pub message_id: MessageId,
    pub conversation_id: String,
    /// Resolved by the module from the authenticated origin, never trusted
    /// from prose or a client-supplied name.
    pub sender: String,
    pub recipient: String,
    pub kind: MessageKind,
    /// The causal parent's `seq` in this same conversation.
    pub reply_to: Option<u64>,
    pub task: Option<TaskRef>,
    pub body: String,
    pub references: Vec<Reference>,
    /// The agreed-network-time deadline for delivery.
    pub expires_at: u64,
    pub admitted_at: u64,
    /// Lowercase hex sha256 over the canonical send request. A retry with
    /// these exact bytes answers with the existing record; different bytes
    /// under the same [`MessageId`] are refused.
    pub digest: String,
}

// ---- delivery -------------------------------------------------------------

/// The one current delivery state per recipient.
///
/// ```text
/// Stored -> Queued -> AdapterAccepted
///                   -> Held -> AdapterAccepted | Refused | Expired
///                   -> DeliveryUnknown -> AdapterAccepted | Expired
/// Stored or Queued -> Refused | Expired
/// ```
///
/// [`DeliveryState::AdapterAccepted`] means the provider input interface
/// accepted the input. It does NOT mean the model read it, understood it,
/// acted on it, or claimed a task.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryState {
    /// The network accepted the immutable record.
    Stored,
    /// The bound service has durably queued it.
    Queued,
    /// The provider input interface accepted it.
    AdapterAccepted,
    /// A provider or local approval barrier, exposed without overriding it.
    Held,
    Refused,
    Expired,
    /// The service cannot establish whether input was accepted. It is NOT a
    /// failure and MUST NOT be replayed automatically.
    DeliveryUnknown,
}

impl DeliveryState {
    /// Terminal states free the recipient's queue accounting and never
    /// transition again.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            DeliveryState::AdapterAccepted | DeliveryState::Refused | DeliveryState::Expired
        )
    }

    /// The transitions the delivery diagram admits. Everything else is
    /// refused — including any transition out of a terminal state.
    pub fn may_advance_to(self, next: DeliveryState) -> bool {
        use DeliveryState::*;
        match self {
            Stored => matches!(next, Queued | Refused | Expired),
            Queued => matches!(
                next,
                AdapterAccepted | Held | DeliveryUnknown | Refused | Expired
            ),
            Held => matches!(next, AdapterAccepted | Refused | Expired),
            DeliveryUnknown => matches!(next, AdapterAccepted | Expired),
            AdapterAccepted | Refused | Expired => false,
        }
    }

    /// The stable snake_case token refusals and displays carry.
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryState::Stored => "stored",
            DeliveryState::Queued => "queued",
            DeliveryState::AdapterAccepted => "adapter_accepted",
            DeliveryState::Held => "held",
            DeliveryState::Refused => "refused",
            DeliveryState::Expired => "expired",
            DeliveryState::DeliveryUnknown => "delivery_unknown",
        }
    }
}

/// The durable per-recipient delivery record. Separate from the message: the
/// message is immutable, this is the one thing that moves.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub conversation_id: String,
    pub seq: u64,
    pub recipient: String,
    pub state: DeliveryState,
    /// The binding credential that last advanced this record, or 0 while it
    /// is still `Stored`. Only the currently authorized binding may advance
    /// it; a stale service's report is refused rather than allowed to
    /// overwrite the current state.
    pub advanced_by: Credential,
    /// A stable snake_case token — never prose, never a path or credential.
    pub reason: Option<String>,
    pub updated_at: u64,
}

// ---- the committed event stream -------------------------------------------

/// What one committed event was. EVERY committed change to a conversation
/// takes a sequence here, so a consumer resuming after its last acknowledged
/// sequence sees delivery and binding transitions too — not only new mail.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EventBody {
    /// A message was admitted AT this sequence; its record is `m/{seq}` and
    /// [`EventPage::Page`] carries it in `messages`.
    MessageAdmitted { sender: String, recipient: String },
    /// One message's delivery record moved.
    DeliveryAdvanced {
        message_seq: u64,
        state: DeliveryState,
        reason: Option<String>,
    },
    /// A participant's input binding was claimed or released.
    BindingChanged {
        participant_id: String,
        credential: Credential,
        detached: bool,
    },
    /// A roster seat was granted, changed or revoked (`role: None`).
    RosterChanged {
        participant_id: String,
        role: Option<Role>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConversationEvent {
    pub seq: u64,
    /// The admitting block's agreed time.
    pub at: u64,
    pub body: EventBody,
}

/// A page of the committed event stream, served only to an authenticated
/// reader on the roster.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EventPage {
    Page {
        events: Vec<ConversationEvent>,
        /// The messages the page's [`EventBody::MessageAdmitted`] events point
        /// at, joined by `Message::seq`. A message pruned since is absent.
        messages: Vec<Message>,
        /// The cursor to resume from.
        next_seq: u64,
    },
    /// The cursor is below the retained floor: resync from `floor_seq`
    /// instead of advancing past events that no longer exist.
    HistoryGap { floor_seq: u64 },
}

// ---- writes ---------------------------------------------------------------

/// The send request as one value, so its canonical bytes ARE the dedup
/// preimage: a retry with identical bytes returns the existing receipt,
/// different bytes under the same [`MessageId`] are refused.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SendRequest {
    pub conversation_id: String,
    /// Which of the caller's identities it is acting as. Checked against the
    /// authenticated origin: naming somebody else here sends nothing.
    pub sender_participant_id: String,
    /// `generation` must be the credential the origin authenticates with —
    /// the participant's `owner_credential` for an owner-key send, or the
    /// binding's `credential` for a scoped service-key send.
    pub message_id: MessageId,
    pub recipient_participant_id: String,
    pub kind: MessageKind,
    #[serde(default)]
    pub reply_to: Option<u64>,
    #[serde(default)]
    pub task: Option<TaskRef>,
    pub body: String,
    #[serde(default)]
    pub references: Vec<Reference>,
    /// The delivery deadline as an ABSOLUTE agreed-network-time value, in this
    /// network's `consensus_time` unit. The sender computes it from the unit
    /// its node reports; the module refuses a deadline already past and one
    /// further out than the composer's configured ceiling. There is no module
    /// default, because a default would have to guess the unit.
    pub expires_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CollaborationMsg {
    /// Register a collaboration identity owned by the authenticated origin.
    RegisterParticipant {
        participant_id: String,
        display_name: String,
        #[serde(default)]
        agent_account: Option<AccountNumber>,
    },
    /// Retire every credential of a participant and stop it admitting or
    /// receiving. Owner only. Its committed history survives.
    RevokeParticipant { participant_id: String },
    CreateConversation {
        conversation_id: String,
        topic: String,
    },
    /// Add, change or (with `role: None`) revoke one roster entry. The
    /// conversation owner only.
    SetRoster {
        conversation_id: String,
        participant_id: String,
        role: Option<Role>,
    },
    /// Claim the participant's ONE input binding in this conversation and
    /// authorize `principal` under a FRESH credential.
    /// `expected_credential` is the credential being replaced — 0 for the
    /// first attachment. A mismatch is refused, so two devices cannot both
    /// claim it. Replacing a binding on one conversation leaves the same
    /// participant's bindings on other conversations untouched.
    ///
    /// The participant's owner issues this; the attached principal then sends,
    /// acknowledges and reads under its own origin alone — never the owner's.
    Bind {
        conversation_id: String,
        participant_id: String,
        device: String,
        principal: BoundPrincipal,
        expected_credential: Credential,
    },
    /// Release the binding. The owner or the bound principal; the credential
    /// number is spent either way.
    Unbind {
        conversation_id: String,
        participant_id: String,
        expected_credential: Credential,
    },
    Send(SendRequest),
    /// The currently bound service reports this recipient's delivery state.
    /// Refused when `binding_credential` is not the current one, when the
    /// transition is not in the delivery diagram, or when the message names a
    /// task attempt that has since been superseded.
    Acknowledge {
        conversation_id: String,
        seq: u64,
        binding_credential: Credential,
        state: DeliveryState,
        #[serde(default)]
        reason: Option<String>,
    },
    /// Move a past-deadline record to `Expired` and free its queue slot.
    /// Permissionless: the deadline is agreed network time, so any caller
    /// asking produces the same answer. Expiry stops further delivery; it
    /// does not undo work already accepted.
    ExpireMessage { conversation_id: String, seq: u64 },
    /// Advance the retention floor to `through_seq` (exclusive), dropping
    /// those events and message bodies. Conversation owner only. The senders'
    /// replay floors rise with it, so expired request bytes cannot be admitted
    /// as a new message after their body is pruned.
    Prune {
        conversation_id: String,
        through_seq: u64,
    },
}

// ---- reads ----------------------------------------------------------------

/// What the AUTHENTICATED caller may do in one conversation, as the acting
/// participant it named. Fails closed: an unknown conversation, an unknown or
/// revoked participant and a caller off the roster all answer all-false.
///
/// It answers about the caller and nobody else, so it cannot be used to
/// enumerate a roster by probing parties.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConversationAccess {
    /// The acting participant is on the roster, so it may read the events.
    pub may_read: bool,
    /// It holds [`Role::Member`], so a send would be admitted.
    pub may_send: bool,
    /// Its current binding credential, or 0 when it holds none.
    pub binding_credential: Credential,
}

/// What a retrying sender learns about a [`MessageId`] it already used.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SendState {
    /// Never used under this credential, and at or above the replay floor.
    Absent,
    /// Admitted: this is the conversation sequence it holds.
    Admitted { seq: u64, digest: String },
    /// Pruned below the replay floor. A retry answers `ReceiptPruned`; it can
    /// never become a new admission.
    ReceiptPruned,
}

/// One participant's mailbox accounting, in the units the caps use.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct MailboxUsage {
    pub undelivered: u64,
    pub queued_bytes: u64,
}

/// What an authenticated caller is asking for, acting as one of its own
/// participants. EVERY read in this module is one of these: there is no open
/// lane, because a participant record, a roster and a binding are all
/// discovery surfaces the spec scopes to who may discover them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ProtectedRead {
    /// The acting participant's own record.
    Participant,
    /// The acting participant's mailbox accounting.
    Mailbox,
    /// The dedup / replay-floor probe a retrying sender reads first.
    SendState {
        generation: Credential,
        sequence: u64,
    },
    /// One conversation the acting participant is on, roster included.
    Conversation { conversation_id: String },
    /// What the acting participant may do there.
    Access { conversation_id: String },
    /// The acting participant's own binding there, without its service key.
    Binding { conversation_id: String },
    /// One page of the committed event stream, ascending from `from_seq`.
    Events {
        conversation_id: String,
        from_seq: u64,
        limit: u64,
    },
    /// One message's delivery record.
    Receipt { conversation_id: String, seq: u64 },
    /// Whether a NEW delivery attempt for one message may still be made — the
    /// question a bound service asks BEFORE handing it to a provider. Answered
    /// for the CALLER as recipient; a sender asking about somebody else's
    /// mailbox is refused.
    DeliveryEligibility { conversation_id: String, seq: u64 },
}

/// The answer to [`ProtectedRead::DeliveryEligibility`], computed against the
/// block's AGREED time — never a caller's clock.
///
/// This is deliberately NOT the receipt. A receipt is a truthful log of what a
/// bound service observed, so a late but honest `adapter_accepted` stays
/// readable there and is never relabelled. Eligibility is the forward-looking
/// question, and the deadline decides it alone: past `expires_at` there is no
/// new submission to make, whatever the log says. A historical acceptance is a
/// fact, not an authorization to deliver again.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryEligibility {
    /// Deliver it. `state` is where the record stands, `expires_at` the
    /// deadline this answer was computed against and `asked_at` the agreed
    /// time it was computed at — so the caller sees how much window is left
    /// without a second read.
    Eligible {
        state: DeliveryState,
        expires_at: u64,
        asked_at: u64,
    },
    /// The deadline has passed. Nothing further is delivered under it.
    Expired { expires_at: u64, asked_at: u64 },
    /// Already settled: accepted, refused or expired. Not work.
    Settled { state: DeliveryState },
    /// The service could not establish whether input was accepted, and the
    /// spec is explicit that such a record MUST NOT be replayed automatically.
    /// A human or an explicit operator decision resolves it.
    NotReplayable,
    /// Nobody is bound to carry it: the participant has no live binding on
    /// this conversation.
    Unbound,
    /// No such message, or its body has been pruned out from under the query.
    Unknown,
}

/// A read on behalf of `participant_id`.
///
/// The caller is taken from the QUERY CONTEXT's authenticated origin — the
/// participant's owner, or the scoped service key of its current binding — and
/// from nothing else. The payload asserts no identity: an unauthenticated
/// context (today's public `/v1/query` lane, which reaches a module as
/// `Origin::System`) is refused, so raw public query fails closed.
///
/// Residual, stated rather than papered over: committed module state is
/// replicated in plaintext to every validator. This gate bounds the RPC read
/// lane; it is not end-to-end confidentiality, and this release neither
/// implements nor claims that.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CollaborationQuery {
    Read {
        participant_id: String,
        /// The conversation whose binding authorizes this read, when the
        /// caller is a scoped service key. Omitted for an owner-key read.
        ///
        /// A service key is issued per conversation and the store's keys are
        /// hashed, so there is no way to find "the binding holding this key"
        /// by scanning: the caller names the one it holds. That keeps the
        /// messaging agent working with its scoped credential alone, never
        /// the owner's account key.
        #[serde(default)]
        via: Option<String>,
        read: ProtectedRead,
    },
}

/// Why an authenticated read was refused. One token, never a hint about what
/// the record would have said.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DenyReason {
    /// The query context carries no authenticated caller. The public read
    /// lane lands here.
    Unauthenticated,
    /// Authenticated, but not this participant's owner or bound service key.
    NotReader,
    /// The participant is revoked, or is not on the conversation's roster.
    NotPermitted,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CollaborationReply {
    Participant(Participant),
    Conversation(Conversation),
    Binding(Option<BindingView>),
    Access(ConversationAccess),
    Events(EventPage),
    Receipt(Option<Receipt>),
    Eligibility(DeliveryEligibility),
    SendState(SendState),
    Mailbox(MailboxUsage),
    /// The authenticated read was refused. The only answer a caller that is
    /// not the reader ever gets.
    Denied(DenyReason),
}

/// The follow-up notification this module emits to a registered sibling module
/// on admission. A hint that the conversation moved — the authoritative body
/// is the committed event stream, fetched after the consumer's last
/// acknowledged sequence.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CollaborationEvent {
    ConversationAdvanced { conversation_id: String, seq: u64 },
}

/// The values this module assigned in-state that an op payload cannot carry,
/// stamped on each applied operation for the derived tier.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CollaborationAssigned {
    Conversation {
        conversation_id: String,
        seq: u64,
        actor: Party,
    },
    Registry {
        actor: Party,
    },
}

/// One op, BOUND TO THE NETWORK it was authorized for.
///
/// A submitted frame's signed preimage is `(scheme, origin, seq, target,
/// payload)` under the global namespace `ducktape:op-frame:v1`
/// (`node::frame_preimage`) — it binds no chain id. So the same signed bytes
/// are valid on every network where that key may submit, and a `Send` alice
/// authorized on one network would otherwise replay on another where the same
/// key is bound.
///
/// The binding therefore lives here, in the PAYLOAD the signature covers, and
/// it wraps every op rather than the one that looked risky: a caller cannot
/// leave it off the op that mattered. It protects every principal equally —
/// an owner-credential send exactly as much as a scoped service key's or a
/// program account's — because it constrains the BYTES, not the signer.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// The `chain_id` of the network this op is for. Must equal the one this
    /// module was composed with; both being empty is refused, never matched
    /// (an empty id on both sides would collapse every network into one).
    pub network: String,
    pub op: CollaborationMsg,
}

impl Request {
    pub fn new(network: impl Into<String>, op: CollaborationMsg) -> Self {
        Self {
            network: network.into(),
            op,
        }
    }
}

// ---- codecs ---------------------------------------------------------------

pub fn encode_msg(m: &Request) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<Request, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &CollaborationQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<CollaborationQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &CollaborationReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<CollaborationReply, String> {
    sdk::wire::decode(b)
}
pub fn encode_event(e: &CollaborationEvent) -> Vec<u8> {
    sdk::wire::encode(e)
}
pub fn decode_event(b: &[u8]) -> Result<CollaborationEvent, String> {
    sdk::wire::decode(b)
}
pub fn encode_assigned(a: &CollaborationAssigned) -> Vec<u8> {
    sdk::wire::encode(a)
}
pub fn decode_assigned(b: &[u8]) -> Result<CollaborationAssigned, String> {
    sdk::wire::decode(b)
}
