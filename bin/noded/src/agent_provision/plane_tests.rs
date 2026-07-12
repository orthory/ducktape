//! the agent TOOL PLANE both lanes hand every run: the node base its tools
//! dial (`DUCKTAPE_NODE`), the identity they act under (`DUCKTAPE_RUN_AGENT`),
//! and the bin dir on PATH where `ducktape-mcp` is found.
//!
//! the duckfs-lane cases here run the REAL `checkout_with` engine against a
//! stand-in files actor on the `NodeCommand` lane ([`spawn_files_actor`], which
//! the forge-lane W6 tests share) — the mount bracket is exercised for real,
//! without booting a node.

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use commonware_codec::DecodeExt as _;
use commonware_cryptography::Signer as _;
use dispatch_oracle::{RoMount, WorkspaceProvisioner as _, WorkspaceSource, WorkspaceSpec};
use duckfs_client::chunk::{chunk_ids, file_object_id};
use duckfs_core::{
    EntryInfo, EntryKindWire, FilesQuery, FilesReply, RefsInfo, decode_query, encode_reply,
};
use futures::StreamExt as _;

use super::*;
use crate::NodeCommand;

/// the skill subtree every W6 test mounts, and the file it must materialize.
pub(super) const SKILL_PREFIX: &str = "/shared/skills/qa";
pub(super) const SKILL_FILE: &str = "SKILL.md";
pub(super) const SKILL_BODY: &str = "always quack twice\n";

/// the in-memory duckfs the stand-in actor serves: one skill subtree.
pub(super) fn skill_tree() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([(
        format!("{SKILL_PREFIX}/{SKILL_FILE}"),
        SKILL_BODY.as_bytes().to_vec(),
    )])
}

/// the W6 mount every skill test pins (`<ro-root>/qa` ⇐ the skill subtree).
/// on-demand by default: the `always` (inlined-persona) half is exercised by
/// [`super::tests`]' soul assembly, which is where a body read can fail a run.
pub(super) fn skill_mount() -> RoMount {
    RoMount {
        source_prefix: SKILL_PREFIX.into(),
        source_snapshot: None,
        mount_subpath: "qa".into(),
        always: false,
    }
}

/// a stand-in files actor on the `NodeCommand` lane: answers the three queries
/// a checkout makes (`refs` / `find` / `read`) over one in-memory tree, so the
/// REAL engine production runs (`checkout_with` through `ActorNodeApi`) drives
/// the mount bracket without a booted node.
///
/// `reject_reads` fails every `read` AFTER the enumeration succeeded — the
/// shape of a mid-checkout failure (a transport drop, an unavailable chunk):
/// the mount dirs are already on disk when the error fires, so the W5 unwind
/// has real debris to remove.
pub(super) fn spawn_files_actor(
    mut rx: futures::channel::mpsc::Receiver<NodeCommand>,
    tree: BTreeMap<String, Vec<u8>>,
    reject_reads: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(cmd) = rx.next().await {
            match cmd {
                // the ONE write a provision makes: the agent session bind, which
                // this actor is the assignee for and so commits. the refusing
                // case is [`spawn_session_actor`].
                NodeCommand::Submit { reply, .. } => {
                    let _ = reply.send(Ok(committed_block()));
                }
                NodeCommand::Query { req, reply, .. } => {
                    let _ = reply.send(files_reply(&tree, reject_reads, &req));
                }
                // the mount lane otherwise only ever reads.
                _ => panic!("the stand-in files actor got an unexpected command"),
            }
        }
    })
}

/// every session bind this actor saw, in order — the ops the provisioner ACTUALLY
/// submitted, so a test asserts against the wire and not against its own belief
/// about it.
pub(super) type SessionBinds = std::sync::Arc<std::sync::Mutex<Vec<runs::RunsMsg>>>;

/// a stand-in actor for the SESSION lane: it records every op the provisioner
/// submits and answers it with `bind` — `Ok` for the node that holds the run's
/// lease, `Err(detail)` for one that does not (the module's own refusal string).
/// files queries are answered over an empty tree, so the rw checkout materializes
/// an empty dir and the provision reaches the session either way.
fn spawn_session_actor(
    mut rx: futures::channel::mpsc::Receiver<NodeCommand>,
    bind: Result<(), &'static str>,
) -> (tokio::task::JoinHandle<()>, SessionBinds) {
    let binds: SessionBinds = Default::default();
    let seen = binds.clone();
    let actor = tokio::spawn(async move {
        while let Some(cmd) = rx.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    reply,
                    ..
                } => {
                    assert_eq!(target, "runs", "the only op a provision submits");
                    seen.lock()
                        .unwrap()
                        .push(runs::decode_msg(&payload).expect("a runs op"));
                    let _ = reply.send(bind.map(|()| committed_block()).map_err(Into::into));
                }
                NodeCommand::Query { req, reply, .. } => {
                    let _ = reply.send(files_reply(&BTreeMap::new(), false, &req));
                }
                _ => panic!("the stand-in session actor got an unexpected command"),
            }
        }
    });
    (actor, binds)
}

pub(super) fn committed_block() -> crate::BlockSummary {
    crate::BlockSummary {
        height: 1,
        app_hash: "ab".repeat(32),
    }
}

/// answer one files query over `tree` — the read half both stand-ins share.
///
/// `reject_reads` fails every `read` AFTER the enumeration succeeded (see
/// [`spawn_files_actor`]).
pub(super) fn files_reply(
    tree: &BTreeMap<String, Vec<u8>>,
    reject_reads: bool,
    req: &[u8],
) -> Result<Vec<u8>, String> {
    match decode_query(req).expect("a files query") {
        FilesQuery::Refs {} => Ok(encode_reply(&FilesReply::Refs(RefsInfo {
            head: None,
            pins: BTreeMap::new(),
            window_len: 0,
        }))),
        FilesQuery::Find { prefix, .. } => Ok(encode_reply(&FilesReply::Find {
            entries: tree
                .iter()
                .filter(|(path, _)| path.starts_with(&prefix))
                .map(|(path, bytes)| file_entry(path, bytes))
                .collect(),
            next: None,
        })),
        FilesQuery::Read { path, .. } if !reject_reads => {
            let bytes = tree.get(&path).cloned().unwrap_or_default();
            Ok(encode_reply(&FilesReply::Read {
                b64: STANDARD.encode(&bytes),
                eof: true,
            }))
        }
        // the verbatim module contract string the engine's taxonomy keys on.
        FilesQuery::Read { .. } => Err("files: chunk not available".to_string()),
        other => panic!("the checkout asked for {other:?}"),
    }
}

/// one file entry, content-addressed EXACTLY as the module stores it — the
/// checkout re-derives this id over the bytes it assembled and refuses a
/// mismatch, so a sloppy stand-in would fail the verify, not the test.
fn file_entry(path: &str, bytes: &[u8]) -> EntryInfo {
    let meta = BTreeMap::new();
    let size = bytes.len() as u64;
    EntryInfo {
        path: path.into(),
        kind: EntryKindWire::File,
        size,
        exec: false,
        object: duckfs_core::to_hex(&file_object_id(size, &chunk_ids(bytes), &meta)),
        meta,
    }
}

/// the run id CONSENSUS knows these runs by — minted with the SAME `run_id_for`
/// the runs module keys its pending map with, never hand-written. the spec's own
/// `run_id` (`{saga_id}:{attempt}`) is the host's dir key and names no run in
/// consensus, so a test that asserted the session bind carried THAT would pass
/// on an id `runs` can never resolve — which is exactly how the write plane
/// shipped dead. the end-to-end proof that these two ids stay distinct and the
/// right one is bound lives in [`super::session_boundary_tests`].
fn consensus_run_id() -> String {
    runs::run_id_for("general", 1, "quackbot")
}

/// a duckfs-source spec: the agent's own workspace prefix (empty in the
/// stand-in tree — the rw checkout materializes nothing) plus the skill mounts.
fn duckfs_spec(agent: Option<&str>, mounts: Vec<RoMount>) -> WorkspaceSpec {
    WorkspaceSpec {
        run_id: "s1:0".into(),
        consensus_run_id: Some(consensus_run_id()),
        agent_id: agent.map(Into::into),
        agent_display_name: agent.map(Into::into),
        source: WorkspaceSource::Duckfs {
            source_prefix: "/shared/agent-workspaces/quackbot".into(),
            source_snapshot: None,
        },
        ro_mounts: mounts,
        library_readable: false,
    }
}

// ---- the node base -------------------------------------------------------

#[test]
fn the_node_base_is_the_bare_http_root_the_forge_lane_hangs_off() {
    // a wildcard bind must become a CONNECTABLE loopback in the SAME family (a
    // bindv6only [::] listener refuses 127.0.0.1) — a run dials this back.
    assert_eq!(
        node_http_base(Some("0.0.0.0:8844")).as_deref(),
        Some("http://127.0.0.1:8844")
    );
    assert_eq!(
        node_http_base(Some("[::]:9001")).as_deref(),
        Some("http://[::1]:9001")
    );
    // an explicit host passes through untouched (the operator chose it).
    assert_eq!(
        node_http_base(Some("10.0.0.5:8844")).as_deref(),
        Some("http://10.0.0.5:8844")
    );
    assert_eq!(
        node_http_base(Some("localhost:8844")).as_deref(),
        Some("http://localhost:8844")
    );
    // no http surface ⇒ nothing to dial ⇒ nothing to hand the run.
    assert_eq!(node_http_base(None), None);

    // DUCKTAPE_NODE is the node ROOT: the /forge mount is the FORGE lane's
    // suffix on top of it, and must never leak into the tool plane's base.
    let base = node_http_base(Some("0.0.0.0:8844")).unwrap();
    assert!(
        !base.contains("/forge"),
        "the node base carries no suffix: {base}"
    );
    assert_eq!(
        forge_push_base(Some("0.0.0.0:8844")).unwrap(),
        format!("{base}/forge"),
        "one derivation, two bases — the push lane is the node base plus /forge"
    );
}

// ---- what a run actually gets ---------------------------------------------

#[tokio::test]
async fn a_run_gets_the_node_base_its_agent_id_and_the_tool_bin_dir_on_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    let _actor = spawn_files_actor(rx, skill_tree(), false);
    let prov = NodedProvisioner::new(handle, tmp.path())
        .with_node_url(Some("http://127.0.0.1:8844".into()));

    let ws = prov
        .provision(&duckfs_spec(Some("quackbot"), vec![skill_mount()]))
        .await
        .expect("provision");
    let dir = ws.workdir();
    let env = ws.env();

    // the tool plane's two halves: WHERE the node is …
    assert_eq!(
        env.get("DUCKTAPE_NODE").map(String::as_str),
        Some("http://127.0.0.1:8844")
    );
    // … and WHO the run acts for. the grant (owner/allowed_actions/caps) is
    // deliberately NOT here: it is read back from the committed registry by
    // this id, so it can never drift from the record.
    assert_eq!(
        env.get("DUCKTAPE_RUN_AGENT").map(String::as_str),
        Some("quackbot")
    );
    assert!(!env.contains_key("DUCKTAPE_RUN_OWNER"));
    assert!(!env.contains_key("DUCKTAPE_RUN_ALLOWED_ACTIONS"));
    // the workspace + skill roots (the skill tree is the -ro SIBLING).
    assert_eq!(
        env.get("DUCKTAPE_RUN_WORKSPACE").map(String::as_str),
        Some(dir.display().to_string().as_str())
    );
    let ro = PathBuf::from(env.get("DUCKTAPE_RUN_SKILLS").expect("skills root"));
    assert_eq!(ro, PathBuf::from(format!("{}-ro", dir.display())));
    assert_eq!(
        std::fs::read_to_string(ro.join("qa").join(SKILL_FILE)).unwrap(),
        SKILL_BODY
    );

    // PATH: the dir holding the RUNNING binary — `ducktape-mcp` ships beside
    // it, and the runner CLI resolves the server by bare command name.
    let exe_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(ws.path_entries(), vec![exe_dir]);

    ws.cleanup().await;
    assert!(
        !dir.exists() && !ro.exists(),
        "W5: both trees are the run's debris"
    );
}

#[tokio::test]
async fn an_unreachable_node_or_an_anonymous_run_omits_the_var_rather_than_guessing() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    let _actor = spawn_files_actor(rx, skill_tree(), false);
    // no with_node_url (a node serving no http surface) and no agent_id.
    let ws = NodedProvisioner::new(handle, tmp.path())
        .provision(&duckfs_spec(None, Vec::new()))
        .await
        .expect("provision");

    let env = ws.env();
    assert!(
        !env.contains_key("DUCKTAPE_NODE"),
        "a guessed base is worse than an absent one"
    );
    assert!(!env.contains_key("DUCKTAPE_RUN_AGENT"));
    assert!(
        !env.contains_key("DUCKTAPE_RUN_SKILLS"),
        "no mounts, no root"
    );
    // the run still runs — only the tool plane is missing, never the workspace.
    assert!(env.contains_key("DUCKTAPE_RUN_WORKSPACE"));
    assert!(
        !ws.path_entries().is_empty(),
        "the bin dir is unconditional"
    );
    ws.cleanup().await;
}

// ---- the agent session key --------------------------------------------------

#[tokio::test]
async fn an_agent_run_gets_a_fresh_session_key_whose_public_half_is_what_was_bound() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    let (_actor, binds) = spawn_session_actor(rx, Ok(()));

    let ws = NodedProvisioner::new(handle, tmp.path())
        .provision(&duckfs_spec(Some("quackbot"), Vec::new()))
        .await
        .expect("provision");
    let env = ws.env();

    // BOTH vars or neither: the key signs the op, the run id says which session
    // it belongs to. the id is the CONSENSUS one — the MCP server stamps this
    // var onto every AgentAction, so the host-local `{saga_id}:{attempt}` here
    // would make every mid-run write name a run that does not exist.
    let key_hex = env
        .get("DUCKTAPE_RUN_SESSION_KEY")
        .expect("an agent run holds a session key");
    assert_eq!(
        env.get("DUCKTAPE_RUN_ID").map(String::as_str),
        Some(consensus_run_id().as_str()),
        "the run the AGENT names is the one runs resolves, never the spec's dir key"
    );
    assert_eq!(key_hex.len(), 64, "32 bytes of lowercase hex");
    let seed = duckfs_core::from_hex_32(key_hex).expect("the key is lowercase hex");

    // THE invariant: what consensus was asked to bind is the PUBLIC half of the
    // key the run holds — a bind of anything else would authorize a key nobody
    // can sign with, and the agent's ops would be refused for the rest of the run.
    let bound = match binds.lock().unwrap().as_slice() {
        [
            runs::RunsMsg::OpenAgentSession {
                run_id,
                session_key,
            },
        ] => {
            assert_eq!(
                run_id,
                &consensus_run_id(),
                "the bind names the run in the id space runs can resolve"
            );
            session_key.clone()
        }
        other => panic!("expected exactly one session bind, got {other:?}"),
    };
    assert_eq!(bound.len(), runs::SESSION_KEY_LEN);
    let public = commonware_cryptography::ed25519::PrivateKey::decode(seed.as_slice())
        .expect("32 bytes decode")
        .public_key();
    assert_eq!(
        bound,
        public.as_ref().to_vec(),
        "the bound key is the pair of the private key handed to the run"
    );

    // and the NODE key is nowhere near the run: only the session key, which can
    // do exactly what this agent may already do, for this run, until it settles.
    assert!(!env.contains_key("DUCKTAPE_NODE_KEY"));
    ws.cleanup().await;
}

#[tokio::test]
async fn a_run_with_no_agent_opens_no_session_and_submits_no_bind() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    let (_actor, binds) = spawn_session_actor(rx, Ok(()));

    let ws = NodedProvisioner::new(handle, tmp.path())
        .provision(&duckfs_spec(None, Vec::new()))
        .await
        .expect("provision");

    let env = ws.env();
    assert!(!env.contains_key("DUCKTAPE_RUN_SESSION_KEY"));
    assert!(!env.contains_key("DUCKTAPE_RUN_ID"));
    assert!(
        binds.lock().unwrap().is_empty(),
        "a workspace nobody acts for asks consensus for nothing"
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn an_envelope_with_no_consensus_run_id_opens_no_session_and_submits_no_bind() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    let (_actor, binds) = spawn_session_actor(rx, Ok(()));

    // a pre-field (or foreign) envelope: an AGENT run, but no run id consensus
    // would recognize. binding on the spec's own `{saga_id}:{attempt}` would ask
    // `runs` to open a session against a run that does not exist — so we ask for
    // nothing at all and degrade to the read-only plane.
    let spec = WorkspaceSpec {
        consensus_run_id: None,
        ..duckfs_spec(Some("quackbot"), Vec::new())
    };
    let ws = NodedProvisioner::new(handle, tmp.path())
        .provision(&spec)
        .await
        .expect("a run without a consensus id still gets its workspace");

    let env = ws.env();
    assert!(!env.contains_key("DUCKTAPE_RUN_SESSION_KEY"));
    assert!(!env.contains_key("DUCKTAPE_RUN_ID"));
    assert!(
        binds.lock().unwrap().is_empty(),
        "no run to bind to ⇒ no op is spent asking"
    );
    // the READ half is untouched: the run still executes.
    assert_eq!(
        env.get("DUCKTAPE_RUN_AGENT").map(String::as_str),
        Some("quackbot")
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn a_refused_bind_degrades_to_a_read_only_plane_and_never_fails_the_run() {
    let tmp = tempfile::tempdir().unwrap();
    let (handle, rx, _hub) = NodeHandle::channel();
    // the shape of a node that is somehow not the run's committed lease-holder.
    let (_actor, binds) = spawn_session_actor(rx, Err("runs: not the run's assignee"));

    // the run STILL provisions: a session is an additive capability, and losing
    // it must never cost the run its workspace (it can still return a response).
    let ws = NodedProvisioner::new(handle, tmp.path())
        .with_node_url(Some("http://127.0.0.1:8844".into()))
        .provision(&duckfs_spec(Some("quackbot"), Vec::new()))
        .await
        .expect("a refused session does not fail the provision");

    let env = ws.env();
    assert_eq!(binds.lock().unwrap().len(), 1, "the bind was attempted");
    assert!(
        !env.contains_key("DUCKTAPE_RUN_SESSION_KEY") && !env.contains_key("DUCKTAPE_RUN_ID"),
        "no session, no key — the agent must not hold a key consensus refused"
    );
    // the READ half of the tool plane is untouched: this is exactly the
    // pre-session behaviour, not a broken run.
    assert_eq!(
        env.get("DUCKTAPE_NODE").map(String::as_str),
        Some("http://127.0.0.1:8844")
    );
    assert_eq!(
        env.get("DUCKTAPE_RUN_AGENT").map(String::as_str),
        Some("quackbot")
    );
    ws.cleanup().await;
}
