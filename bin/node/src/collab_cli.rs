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
