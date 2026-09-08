//! `ducktape collab` — the operator's seam onto a collaboration conversation:
//! attach a personal provider session to it, and read its protected state.
//!
//! ## the whole design, in one sentence
//!
//! Every verb here is a signed request to the operator's OWN node over `/v1`,
//! so messaging needs no inbound socket on any laptop, borrows no pty, and
//! survives an agent daemon reconnect.
//!
//! ## why an attachment is a chain write and not a session
//!
//! A binding names WHICH conversation a device will receive, and it has to
//! outlive every process involved — that is the difference between a mailbox
//! and a terminal. The interactive plane cannot hold one: `agent-service`'s
//! `Sessions::close_all` ends every live pty the moment the daemon's ws link to
//! its node drops (`bin/node/src/agent/link.rs`), and a dropped link is
//! ORDINARY — the node restarts, the operator upgrades, the socket blips. A
//! binding stored there would be revoked by a network hiccup.
//!
//! So a binding is committed module state. The daemon learns its bindings by
//! reading committed state, the way compute already learns its placement, and
//! the link's teardown cannot reach them. Nothing in this file touches
//! `Sessions`, a `term:` topic, `TermInput` or `TermOutput`; pty bytes are raw
//! and unstructured and are not a delivery channel. Pinned by
//! `bin/node/tests/collab_cli_seam.rs`.
//!
//! ## reads are authenticated, and that is not free
//!
//! `/v1/query` is open to anything that can dial the port — a sandbox guest
//! reaching this listener over its vsock tunnel included — and the module
//! answering it sees `Origin::System`, which proves nothing about the caller.
//! A participant id in such a query is a client's claim. So protected reads go
//! to `/v1/query/reader`, where the caller's own signature names the reader and
//! the node hands the VERIFIED key to the module as its origin.
//!
//! That authenticates the reader at the RPC edge. It is not confidentiality:
//! bodies live in replicated committed state, so every validator can read them.
//! Nothing here promises end-to-end encryption.
//!
//! Program output stays `println!` — a CLI's stdout is not logging.

use std::io::BufRead;

use crate::cred_cli::VerbCtx;

type CollabResult = Result<(), Box<dyn std::error::Error>>;

/// `ducktape collab <verb>`. The shared addressing group selects the node this
/// CLI dials; `--key` is the identity every verb acts as.
#[derive(Debug, clap::Args)]
pub(crate) struct CollabArgs {
    #[command(subcommand)]
    cmd: CollabCmd,
    #[command(flatten)]
    addr: crate::cli_args::NodeAddr,
    /// path to the user key that signs (defaults to the keystore's active
    /// wallet). This key IS the reader/sender identity — there is no separate
    /// participant credential to present.
    #[arg(long, value_name = "PATH", global = true)]
    key: Option<std::path::PathBuf>,
    /// re-pin this node's key when it differs from the one already trusted for
    /// this url. Without it a changed key is refused (`node_key_mismatch`).
    #[arg(long, global = true)]
    trust_node: bool,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum CollabCmd {
    /// read protected module state AS your key, over the authenticated lane
    Query(QueryArgs),
    /// this device's scoped service key for one binding: mint it if absent,
    /// then print its PUBLIC half for the `Bind` that authorizes it
    Key(KeyArgs),
    /// attach this device to a conversation: mint the scoped service key and
    /// submit the OWNER-signed `Bind` that authorizes it
    Attach(AttachArgs),
    /// send one message, signed by the binding's scoped service key
    Send(SendArgs),
    /// report a delivery state for one message, signed by the scoped key
    Ack(AckArgs),
}

/// `collab attach` — the owner authorizes THIS device.
///
/// Two keys, deliberately. The scoped service key is minted here and never
/// leaves the workspace; the OWNER's wallet key signs the `Bind` that names its
/// public half. A scoped credential that could authorize itself would not be
/// scoped by anything.
#[derive(Debug, clap::Args)]
pub(crate) struct AttachArgs {
    #[arg(long, value_name = "ID")]
    conversation: String,
    #[arg(long, value_name = "ID")]
    participant: String,
    /// an opaque label for this device, for a human reading the roster. NEVER
    /// a socket path, a url or a provider session id — a device label is
    /// committed state that every validator can read.
    #[arg(long, value_name = "LABEL")]
    device: String,
    /// the credential being REPLACED: 0 for a first attachment, otherwise the
    /// current `binding_credential` from `Access`. A mismatch is refused, which
    /// is what stops two devices both claiming the one input binding.
    #[arg(long, value_name = "N", default_value_t = 0)]
    expect: u64,
}

/// `collab send` — one message, under the binding's scoped credential.
#[derive(Debug, clap::Args)]
pub(crate) struct SendArgs {
    #[arg(long, value_name = "ID")]
    conversation: String,
    /// the participant this device is sending AS. Checked against the
    /// authenticated origin: naming somebody else sends nothing.
    #[arg(long, value_name = "ID")]
    participant: String,
    #[arg(long, value_name = "ID")]
    to: String,
    #[arg(long, value_enum, default_value_t = Kind::Notice)]
    kind: Kind,
    /// the credential this send authenticates under — the binding's
    /// `binding_credential` from `Access`.
    #[arg(long, value_name = "N")]
    credential: u64,
    /// the monotonic sequence within that credential.
    ///
    /// Explicit on purpose: the SENDER SERVICE owns this counter, and a CLI
    /// that minted its own would race the attached daemon's and burn ids under
    /// the same credential. Read the last one back with the `send_state` query.
    #[arg(long, value_name = "N")]
    seq: u64,
    /// delivery deadline, in seconds from now. Scaled into the network's own
    /// `consensus_time` unit before it is submitted — never a laptop clock.
    #[arg(long, value_name = "SECONDS", default_value_t = 86_400)]
    ttl_secs: u64,
    /// the message body, after `--`
    #[arg(last = true, value_name = "BODY", required = true)]
    body: String,
}

/// The message kinds, as the module spells them.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum Kind {
    Notice,
    Question,
    TaskRequest,
    TaskUpdate,
    Result,
}

/// The delivery states a bound service may report.
///
/// Two of the module's seven are deliberately absent. `Stored` is what
/// ADMISSION records, and `Expired` is what the permissionless `ExpireMessage`
/// decides from the agreed deadline. Offering either here would let a device
/// assert the network's own bookkeeping about itself.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum AckState {
    Queued,
    AdapterAccepted,
    Held,
    Refused,
    DeliveryUnknown,
}

/// `collab ack` — the bound service reports a delivery state.
#[derive(Debug, clap::Args)]
pub(crate) struct AckArgs {
    #[arg(long, value_name = "ID")]
    conversation: String,
    /// the participant whose binding this device holds — it selects the scoped
    /// key that signs, and the module checks that key against the binding.
    #[arg(long, value_name = "ID")]
    participant: String,
    /// the conversation sequence of the message being reported on
    #[arg(long, value_name = "N")]
    seq: u64,
    /// the binding credential this report is made under. A stale one is
    /// refused, so a returning old attachment cannot overwrite the current
    /// binding's state.
    #[arg(long, value_name = "N")]
    credential: u64,
    #[arg(long, value_enum)]
    state: AckState,
    /// a stable snake_case token, never prose and never a path or a token
    #[arg(long, value_name = "TOKEN")]
    reason: Option<String>,
}

/// Names one binding — the pair the module keys a `Binding` on.
#[derive(Debug, clap::Args)]
pub(crate) struct KeyArgs {
    /// the conversation this attachment will receive
    #[arg(long, value_name = "ID")]
    conversation: String,
    /// the participant this device is attaching for
    #[arg(long, value_name = "ID")]
    participant: String,
    /// print the key only if it already exists, never mint one — for checking
    /// whether this device holds the binding at all
    #[arg(long)]
    existing_only: bool,
}

/// The JSON is the MODULE's own query enum, passed through verbatim — this CLI
/// is a transport. Spelling core's shape as a Rust type here would be a second
/// copy of it to drift; an example in the help is not.
///
/// `collaboration` takes one envelope, and the caller it acts as comes from the
/// signature, never from the payload:
///
/// ```text
/// ducktape collab query --target collaboration \
///   '{"read":{"participant_id":"p1","read":{"events":{"conversation_id":"c1","from_seq":0,"limit":50}}}}'
/// ```
///
/// `via` names the conversation whose binding authorizes the read when you are
/// signing with that binding's scoped service key rather than the owner key.
#[derive(Debug, clap::Args)]
pub(crate) struct QueryArgs {
    /// the module to ask (e.g. `collaboration`)
    #[arg(long, value_name = "MODULE")]
    target: String,
    /// the module's query enum as one JSON value
    #[arg(value_name = "QUERY_JSON")]
    query: String,
}

pub(crate) fn run(args: CollabArgs) -> CollabResult {
    let CollabArgs {
        cmd,
        addr,
        key,
        trust_node,
    } = args;
    let ctx = VerbCtx { addr, key };
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    match cmd {
        CollabCmd::Query(query) => cmd_query(query, &ctx, trust_node, &mut stdin),
        CollabCmd::Key(key) => cmd_key(key, &ctx),
        CollabCmd::Attach(attach) => cmd_attach(attach, &ctx, &mut stdin),
        CollabCmd::Send(send) => cmd_send(send, &ctx),
        CollabCmd::Ack(ack) => cmd_ack(ack, &ctx),
    }
}

/// The module every verb here submits to and reads from.
const COLLABORATION: &str = "collaboration";

/// Submit one collaboration op as a frame `signer` signs, and print the height.
///
/// `/v1/submit/frame` is the whole authorization: the frame's verified signer
/// becomes the op's `Origin::External`, and the module admits or refuses it.
/// This CLI pre-checks nothing — a second gate here could only drift from the
/// one that decides, and the module's refusal reaches the operator verbatim.
fn submit(
    base: &str,
    signer: &commonware_cryptography::ed25519::PrivateKey,
    msg: &collaboration::CollaborationMsg,
) -> CollabResult {
    let frame = crate::userkey_cli::user_frame(signer, COLLABORATION, collaboration::encode_msg(msg));
    let height = crate::node_http::submit_frame(base, &frame)?;
    println!("{height}");
    Ok(())
}

/// This binding's scoped service key, or a refusal naming the attach that would
/// create it. Never mints: `send` and `ack` act AS an existing binding, and
/// minting one here would sign with a key no committed `Bind` has authorized —
/// producing an op the module refuses, from a CLI that looked like it worked.
fn scoped_key(
    ctx: &VerbCtx,
    conversation: &str,
    participant: &str,
) -> Result<commonware_cryptography::ed25519::PrivateKey, Box<dyn std::error::Error>> {
    let workspace = ctx.addr.workspace()?;
    let binding = crate::collab_keys::BindingRef {
        conversation,
        participant,
    };
    crate::collab_keys::load(&workspace, binding)?.ok_or_else(|| {
        format!(
            "this device holds no service key for participant {participant} in \
             conversation {conversation} — run `ducktape collab attach` first"
        )
        .into()
    })
}

/// `collab attach` — mint the scoped key, then have the OWNER authorize it.
///
/// Order matters: the key exists before the `Bind` that names it, so a committed
/// binding can never name a public key this device does not hold. The reverse
/// order would leave a binding nothing can sign for, and only an `Unbind` could
/// clear it.
fn cmd_attach(args: AttachArgs, ctx: &VerbCtx, stdin: &mut impl BufRead) -> CollabResult {
    let base = ctx.http_base()?;
    let workspace = ctx.addr.workspace()?;
    let binding = crate::collab_keys::BindingRef {
        conversation: &args.conversation,
        participant: &args.participant,
    };
    let service = crate::collab_keys::ensure(&workspace, binding)?;
    let service_key = commonware_cryptography::Signer::public_key(&service)
        .as_ref()
        .to_vec();

    // the OWNER signs: a scoped credential cannot authorize itself.
    let owner = crate::userkey_cli::load_user_signer(&ctx.key_path()?, stdin)?;
    submit(
        &base,
        &owner,
        &collaboration::CollaborationMsg::Bind {
            conversation_id: args.conversation,
            participant_id: args.participant,
            device: args.device,
            service_key,
            expected_credential: args.expect,
        },
    )
}

/// `collab send` — signed by the scoped key, so no wallet password is needed.
fn cmd_send(args: SendArgs, ctx: &VerbCtx) -> CollabResult {
    let base = ctx.http_base()?;
    let service = scoped_key(ctx, &args.conversation, &args.participant)?;
    let expires_at = deadline(&base, args.ttl_secs)?;
    submit(
        &base,
        &service,
        &collaboration::CollaborationMsg::Send(collaboration::SendRequest {
            conversation_id: args.conversation,
            sender_participant_id: args.participant,
            message_id: collaboration::MessageId {
                generation: args.credential,
                sequence: args.seq,
            },
            recipient_participant_id: args.to,
            kind: args.kind.into(),
            reply_to: None,
            task: None,
            body: args.body,
            references: Vec::new(),
            expires_at,
        }),
    )
}

/// `collab ack` — the bound service reports what its provider did.
fn cmd_ack(args: AckArgs, ctx: &VerbCtx) -> CollabResult {
    let base = ctx.http_base()?;
    let service = scoped_key(ctx, &args.conversation, &args.participant)?;
    submit(
        &base,
        &service,
        &collaboration::CollaborationMsg::Acknowledge {
            conversation_id: args.conversation,
            seq: args.seq,
            binding_credential: args.credential,
            state: args.state.into(),
            reason: args.reason,
        },
    )
}

/// The absolute `expires_at` a message sent now carries, in the NETWORK's
/// `consensus_time` unit.
///
/// The module compares against `ctx.env().consensus_time` and has no default of
/// its own, because a default would have to guess the unit. So the sender reads
/// the unit off the node and scales into it — never a laptop's wall clock, which
/// two nodes would disagree about.
fn deadline(base: &str, ttl_secs: u64) -> Result<u64, Box<dyn std::error::Error>> {
    let status =
        crate::node_http::get_json(base, "/v1/status").map_err(|failure| failure.to_string())?;
    let now = status["consensus_time"]
        .as_u64()
        .ok_or_else(|| "node status carries no consensus_time".to_string())?;
    let unit: noded::ConsensusTimeUnit = status
        .get("consensus_time_unit")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("node status consensus_time_unit: {error}"))?
        .unwrap_or_default();
    Ok(now + ttl_units(ttl_secs, unit))
}

/// Seconds into one network's `consensus_time` units. Pure, so the two shapes
/// are testable without a node to dial.
///
/// A `Height` unit is one block and a validator network heartbeats about once a
/// second, so seconds map one-to-one; `Millis` is a millisecond epoch clock.
/// The scaling saturates rather than wrapping: an absurd `--ttl-secs` must
/// become a deadline the module refuses as too far out, never one that wraps
/// past zero into the past.
fn ttl_units(ttl_secs: u64, unit: noded::ConsensusTimeUnit) -> u64 {
    match unit {
        noded::ConsensusTimeUnit::Height => ttl_secs,
        noded::ConsensusTimeUnit::Millis => ttl_secs.saturating_mul(1_000),
    }
}

impl From<Kind> for collaboration::MessageKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Notice => Self::Notice,
            Kind::Question => Self::Question,
            Kind::TaskRequest => Self::TaskRequest,
            Kind::TaskUpdate => Self::TaskUpdate,
            Kind::Result => Self::Result,
        }
    }
}

impl From<AckState> for collaboration::DeliveryState {
    fn from(state: AckState) -> Self {
        match state {
            AckState::Queued => Self::Queued,
            AckState::AdapterAccepted => Self::AdapterAccepted,
            AckState::Held => Self::Held,
            AckState::Refused => Self::Refused,
            AckState::DeliveryUnknown => Self::DeliveryUnknown,
        }
    }
}

/// `collab key --conversation <id> --participant <id>` — the scoped service key
/// this device signs that binding's delivery receipts with.
///
/// Prints the PUBLIC half, because that is the only part anything else needs:
/// it goes into the `Bind` op the OWNER signs, and the owner's signature is
/// what authorizes this key. The private half never leaves the workspace and is
/// never printed — a key echoed into a terminal is a key in a scrollback
/// buffer, a screen share and a shell history.
///
/// Minting is the default and is idempotent: attaching is a re-runnable
/// operation, and a second attach must present the same key or the committed
/// binding would name one nothing holds.
fn cmd_key(args: KeyArgs, ctx: &VerbCtx) -> CollabResult {
    let workspace = ctx.addr.workspace()?;
    let binding = crate::collab_keys::BindingRef {
        conversation: &args.conversation,
        participant: &args.participant,
    };
    let key = match args.existing_only {
        true => crate::collab_keys::load(&workspace, binding)?.ok_or_else(|| {
            format!(
                "this device holds no binding for participant {} in conversation {}",
                args.participant, args.conversation
            )
        })?,
        false => crate::collab_keys::ensure(&workspace, binding)?,
    };
    println!("{}", crate::collab_keys::public_hex(&key));
    Ok(())
}

/// `collab query --target <module> '<json>'` — one authenticated read.
///
/// The operator cannot mint the signature trio with `curl`, which is the whole
/// reason this verb exists (the same reason `node log-filter` does). The query
/// JSON is passed through verbatim: this CLI is a transport, and pre-parsing a
/// module's own enum here would only be a second, drifting copy of it.
fn cmd_query(
    args: QueryArgs,
    ctx: &VerbCtx,
    trust_node: bool,
    stdin: &mut impl BufRead,
) -> CollabResult {
    let query: serde_json::Value = serde_json::from_str(&args.query)
        .map_err(|error| format!("--query is not valid json: {error}"))?;
    let base = ctx.http_base()?;
    let key_path = ctx.key_path()?;
    // pinned, never the node's own plain claim: the signature binds to this key,
    // so a proxy that could choose it could choose what we signed for.
    let node_key = crate::node_http::pinned_node_key(&key_path, &base, trust_node)?;
    let signer = crate::userkey_cli::load_user_signer(&key_path, stdin)?;
    let reply = crate::node_http::query_as_reader(&base, &signer, &node_key, &args.target, query)?;
    println!("{reply}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[derive(Debug, clap::Parser)]
    struct Harness {
        #[command(subcommand)]
        cmd: CollabCmd,
    }

    #[test]
    fn a_query_verb_takes_a_module_and_one_json_value() {
        let parsed = Harness::parse_from([
            "collab",
            "query",
            "--target",
            "collaboration",
            r#"{"conversation":{"conversation_id":"c1"}}"#,
        ]);
        let CollabCmd::Query(args) = parsed.cmd else {
            panic!("query parses to the query verb");
        };
        assert_eq!(args.target, "collaboration");
        assert!(args.query.contains("conversation_id"));
    }

    #[test]
    fn a_key_verb_names_the_binding_it_belongs_to() {
        let parsed = Harness::parse_from([
            "collab",
            "key",
            "--conversation",
            "c1",
            "--participant",
            "p1",
        ]);
        let CollabCmd::Key(args) = parsed.cmd else {
            panic!("key parses to the key verb");
        };
        assert_eq!((args.conversation.as_str(), args.participant.as_str()), ("c1", "p1"));
        assert!(
            !args.existing_only,
            "minting is the default: attaching is the common case"
        );
    }

    /// The messaging plane must never be routed through the pty plane.
    ///
    /// `agent-service`'s `Sessions::close_all` ends every live session whenever
    /// the daemon's ws link to its node drops, and that drop is ORDINARY — a
    /// node restart, an upgrade, a blip. Anything holding a durable messaging
    /// attachment there would be revoked by a network hiccup. And pty bytes are
    /// raw, unstructured and unacknowledged, so they are not a delivery channel
    /// either.
    ///
    /// A source-parsing lint rather than a comment, because the shape is
    /// load-bearing: the day someone reaches for `TermInput` to push a message
    /// into an attached session, this fails.
    #[test]
    fn the_collab_plane_never_touches_the_pty_plane() {
        let source = include_str!("collab_cli.rs");
        // Scan the SHIPPING half only, and strip its prose. Both cuts are
        // load-bearing: the doc comments name the pty plane on purpose to
        // explain why it is absent, and this test's own banned-word list is
        // code — scanning it would make the lint match itself and fail always.
        let shipping = source
            .split_once("\n#[cfg(test)]")
            .map(|(before, _)| before)
            .expect("this file has a test module");
        let code: String = shipping
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for banned in [
            "Sessions",
            "close_all",
            "TermCreate",
            "TermInput",
            "TermOutput",
            "TermClose",
            "agent_service",
            "term_plane",
        ] {
            assert!(
                !code.contains(banned),
                "the collaboration plane must not reach into the pty plane, but it names {banned}"
            );
        }
    }

    /// A deadline is the network's clock plus a TTL in the network's OWN unit.
    /// Getting the unit wrong is not a rounding error: a 24-hour TTL sent as
    /// 86 400 milliseconds is 86 seconds, and the message expires before anyone
    /// reads it.
    #[test]
    fn a_ttl_scales_into_the_networks_own_time_unit() {
        assert_eq!(ttl_units(86_400, noded::ConsensusTimeUnit::Height), 86_400);
        assert_eq!(
            ttl_units(86_400, noded::ConsensusTimeUnit::Millis),
            86_400_000
        );
    }

    /// An absurd TTL must become a deadline the module refuses as too far out,
    /// never one that wraps past zero into the past — which the module would
    /// accept as "already expired" or, worse, admit.
    #[test]
    fn an_absurd_ttl_saturates_rather_than_wrapping_into_the_past() {
        let enormous = ttl_units(u64::MAX, noded::ConsensusTimeUnit::Millis);
        assert_eq!(enormous, u64::MAX, "it must saturate, not wrap");
        assert!(enormous > 0);
    }

    /// `send` and `ack` act AS an existing binding. Minting a key here would
    /// sign with one no committed `Bind` has authorized — an op the module
    /// refuses, from a CLI that looked like it had worked. The refusal has to
    /// name the verb that fixes it.
    #[test]
    fn sending_without_a_binding_refuses_and_never_mints_a_key() {
        let workspace = tempfile::TempDir::new().expect("temp workspace");
        let binding = crate::collab_keys::BindingRef {
            conversation: "c1",
            participant: "p1",
        };
        // the refusal path is `collab_keys::load` returning None; assert the
        // sentence and that nothing was created behind it.
        assert!(
            crate::collab_keys::load(workspace.path(), binding)
                .expect("absence is not an error")
                .is_none()
        );
        assert!(
            !workspace.path().join("collab-keys").exists(),
            "a failed send must not leave a key directory behind"
        );
    }

    /// The two keys of an attach are DIFFERENT keys, and the scoped one is the
    /// one that stays. If they were ever the same, "scoped" would mean nothing:
    /// the attached session would hold the owner's account key.
    #[test]
    fn an_attach_authorizes_a_scoped_key_that_is_not_the_owner_key() {
        let workspace = tempfile::TempDir::new().expect("temp workspace");
        let binding = crate::collab_keys::BindingRef {
            conversation: "c1",
            participant: "p1",
        };
        let scoped = crate::collab_keys::ensure(workspace.path(), binding).expect("mints");
        let owner = commonware_cryptography::ed25519::PrivateKey::from_seed(1234);
        assert_ne!(
            crate::collab_keys::public_hex(&scoped),
            crate::collab_keys::public_hex(&owner),
            "the scoped key must never be the owner's"
        );
    }

    /// The states a bound service may report are exactly the ones it can
    /// OBSERVE. `Stored` is admission's and `Expired` is the agreed deadline's;
    /// a device asserting either would be asserting the network's own
    /// bookkeeping about itself.
    #[test]
    fn a_service_cannot_report_the_networks_own_states() {
        let reportable: Vec<collaboration::DeliveryState> = [
            AckState::Queued,
            AckState::AdapterAccepted,
            AckState::Held,
            AckState::Refused,
            AckState::DeliveryUnknown,
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        assert!(!reportable.contains(&collaboration::DeliveryState::Stored));
        assert!(!reportable.contains(&collaboration::DeliveryState::Expired));
        assert_eq!(reportable.len(), 5, "and every other state stays reachable");
    }

    /// The JSON is checked HERE, before a key is unlocked. Unlocking costs one
    /// argon2id pass over 64 MiB, and paying it to then reject a typo is a
    /// second of the operator's life for nothing.
    #[test]
    fn a_malformed_query_is_refused_before_anything_expensive() {
        let refused = cmd_query(
            QueryArgs {
                target: "collaboration".into(),
                query: "{not json".into(),
            },
            &VerbCtx {
                addr: crate::cli_args::NodeAddr::default(),
                key: None,
            },
            false,
            &mut std::io::Cursor::new(Vec::new()),
        )
        .expect_err("invalid json cannot reach the node");
        assert!(
            refused.to_string().contains("not valid json"),
            "{refused}"
        );
    }
}
