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
}

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
    }
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
        let CollabCmd::Query(args) = parsed.cmd;
        assert_eq!(args.target, "collaboration");
        assert!(args.query.contains("conversation_id"));
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
        // the doc comment above names these on purpose; strip the prose so the
        // lint reads CODE, not the explanation of itself.
        let code: String = source
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
