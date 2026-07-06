//! snapshot/install round-trip for the docs harness: committed state built
//! through the real execute path (install, minted idempotency keys, failure
//! rows, a lifecycle flip) crosses to a fresh module as canonical bytes —
//! the exact `root()` preimage — and re-derives the identical root with
//! query parity. the bytes arrive UNTRUSTED (a byzantine peer serves them),
//! so the flip side is exercised too: tampered, truncated, padded, and
//! misordered snapshots are rejected and the target module is left
//! byte-identical to before the call. because `install_snapshot`
//! authenticates the BYTES first, the strict-decode cases are driven under a
//! COLLUDING root (sha256 of the evil bytes): even that must not smuggle in
//! an execute-unreachable state.

use docs_harness::{ACTION_COMMENT_ADD, DocsHarness, DocsQuery, DocsReply, MAX_FAILURE_ROWS};
use futures::executor::block_on;
use package::{
    ActionRoute, AgentSeed, EngagementRule, HarnessMsg, InstallSpec, MANIFEST_HASH_LEN,
    ModuleBinding, PackageActionMsg, PromptSeed, UninstallPolicy, encode_action_msg,
    encode_harness_msg,
};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

const HARNESS: &str = "docs-harness";
const PKG: &str = "org.ducktape.docs";

/// a minimal `Ctx`: the jobs board reads empty (probe-before-emit passes) and
/// the pages module serves one page root `p1`; emitted follow-ups are dropped
/// (the consumer half is not under test here).
struct TestCtx {
    env: Env,
}

impl TestCtx {
    fn at(origin: Origin) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height: 1,
                consensus_time: 1,
                origin,
                me: HARNESS.into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        Some(StateRoot::ZERO)
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match target {
            "jobs" => Ok(jobs::encode_reply(&jobs::JobsReply::Job(None))),
            // the staged prompt seed the install arm pins (fresh path: gen 1).
            "memory" => match memory::decode_query(req).map_err(Error::Module)? {
                memory::MemoryQuery::Stat { path } => Ok(memory::encode_reply(
                    &memory::MemoryReply::Stat(Some(memory::FileStat {
                        path,
                        latest_generation: 1,
                        generations: 1,
                        latest_meta: memory::Meta::new(),
                        latest_author: "package".into(),
                        latest_published_at_height: 1,
                        body_len: 0,
                    })),
                )),
                other => Err(Error::Module(format!("unexpected query: {other:?}"))),
            },
            "pages" => {
                let reply = match pages::decode_query(req).map_err(Error::Module)? {
                    pages::PageQuery::GetBlock { block_id } => {
                        pages::PageReply::Block((block_id == "p1").then(|| pages::Block {
                            id: "p1".into(),
                            parent: None,
                            page: "p1".into(),
                            kind: pages::BlockKind::Page,
                            text: "Docs".into(),
                            checked: false,
                            children: Vec::new(),
                        }))
                    }
                    pages::PageQuery::CommentThread { .. } => pages::PageReply::CommentThread(None),
                    other => return Err(Error::Module(format!("unexpected query: {other:?}"))),
                };
                Ok(pages::encode_reply(&reply))
            }
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _ev: Event) {}
    fn request_effect(&mut self, _eff: Effect) {}
}

fn module() -> DocsHarness {
    DocsHarness::new(
        HARNESS, "package", "agent", "jobs", "memory", "pages", "runs",
    )
}

fn spec() -> InstallSpec {
    let content = "# Docs Editor\n";
    InstallSpec {
        package: PKG.into(),
        version: "0.1.0".into(),
        manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
        modules: vec![
            ModuleBinding {
                logical: "pages".into(),
                module_id: "pages".into(),
            },
            ModuleBinding {
                logical: HARNESS.into(),
                module_id: HARNESS.into(),
            },
        ],
        harness: HARNESS.into(),
        prompts: vec![PromptSeed {
            logical: "docs_editor_prompt".into(),
            path: "/packages/org.ducktape.docs/prompts/docs-editor.md".into(),
            content: content.into(),
            sha256: Sha256::digest(content.as_bytes()).to_vec(),
        }],
        agents: vec![AgentSeed {
            agent_id: "docs.editor".into(),
            display_name: "Docs Editor".into(),
            capability: "codex".into(),
            prompt: "docs_editor_prompt".into(),
            actions: vec![ACTION_COMMENT_ADD.into()],
            active: true,
        }],
        actions: vec![ActionRoute {
            tag: ACTION_COMMENT_ADD.into(),
            owner: HARNESS.into(),
        }],
        engagements: vec![EngagementRule {
            source: "pages".into(),
            event: "comment_added".into(),
            agent: "docs.editor".into(),
            policy: "mention_or_assigned".into(),
        }],
        uninstall: UninstallPolicy {
            pending_runs: "drain".into(),
            user_data: "preserve".into(),
        },
    }
}

fn exec(m: &mut DocsHarness, origin: Origin, payload: Vec<u8>) {
    let mut ctx = TestCtx::at(origin);
    block_on(m.execute(
        &mut ctx,
        &Msg {
            target: HARNESS.into(),
            payload,
        },
    ))
    .expect("op applies");
    block_on(m.commit_block()).expect("commit");
}

/// committed state exercising every encoded surface: an installed package
/// (suspended, so the phase byte is non-zero), minted idempotency keys, and
/// failure rows.
fn populated() -> DocsHarness {
    let mut m = module();
    exec(
        &mut m,
        Origin::Module("package".into()),
        encode_harness_msg(&HarnessMsg::InstallPackage {
            package: PKG.into(),
            spec: spec(),
        }),
    );
    // two mention comments mint two idempotency keys.
    for comment_id in ["c1", "c2"] {
        exec(
            &mut m,
            Origin::Module("pages".into()),
            pages::encode_page_event(&pages::PageEvent::CommentAdded {
                page_id: "p1".into(),
                target: "p1".into(),
                thread_id: "t1".into(),
                comment_id: comment_id.into(),
                author: pages::AuthorRef::User(vec![7; 32]),
                text: "@docs.editor please".into(),
            }),
        );
    }
    // a failing apply lands one error row.
    exec(
        &mut m,
        Origin::Module("runs".into()),
        encode_action_msg(&PackageActionMsg::Apply {
            action_id: "a1".into(),
            tag: ACTION_COMMENT_ADD.into(),
            payload: br#"{"target":"ghost","text":"hi"}"#.to_vec(),
            run_context: br#"{"run_id":"r1","agent_id":"docs.editor"}"#.to_vec(),
        }),
    );
    // a lifecycle flip exercises the phase byte.
    exec(
        &mut m,
        Origin::Module("package".into()),
        encode_harness_msg(&HarnessMsg::SuspendPackage {
            package: PKG.into(),
        }),
    );
    m
}

fn status_of(m: &DocsHarness) -> Vec<u8> {
    block_on(m.query(&docs_harness::encode_query(&DocsQuery::Status))).expect("status query")
}

#[test]
fn snapshot_round_trips_byte_identically_with_query_parity() {
    let m = populated();
    let root = m.root();
    let bytes = m.snapshot();
    assert_eq!(
        StateRoot(Sha256::digest(&bytes).into()),
        root,
        "the snapshot is the exact root preimage"
    );

    let mut fresh = module();
    fresh.install_snapshot(&bytes, root).expect("install");
    assert_eq!(fresh.root(), root, "the fresh root re-derives");
    assert_eq!(fresh.snapshot(), bytes, "byte-identical re-encode");

    // query parity: status and failures read the same on both sides.
    assert_eq!(status_of(&fresh), status_of(&m));
    let failures = block_on(fresh.query(&docs_harness::encode_query(&DocsQuery::Failures)))
        .expect("failures query");
    match docs_harness::decode_reply(&failures).expect("reply decodes") {
        DocsReply::Failures(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].action_id, "a1");
        }
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn tampered_truncated_and_padded_snapshots_are_rejected() {
    let m = populated();
    let root = m.root();
    let bytes = m.snapshot();

    let mut target = module();
    let before = target.snapshot();

    // flipped byte, truncation, padding: all reject against the honest root.
    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xff;
    for evil in [
        tampered,
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.clone(), vec![0u8]].concat(),
    ] {
        assert!(
            target.install_snapshot(&evil, root).is_err(),
            "malformed bytes must reject"
        );
        assert_eq!(target.snapshot(), before, "the target is untouched");
    }
    // honest bytes against a wrong root reject too.
    assert!(target.install_snapshot(&bytes, StateRoot::ZERO).is_err());
    assert_eq!(target.snapshot(), before);
}

#[test]
fn colluding_roots_cannot_smuggle_execute_unreachable_states() {
    // hand-build evil encodings and pair each with ITS OWN sha256 root: the
    // strict decoder, not the hash check, must reject them.
    fn push_str(out: &mut Vec<u8>, value: &str) {
        out.extend_from_slice(&(value.len() as u64).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }
    fn count(out: &mut Vec<u8>, n: u64) {
        out.extend_from_slice(&n.to_le_bytes());
    }

    let mut evils: Vec<(&str, Vec<u8>)> = Vec::new();

    // an invalid phase byte.
    let mut evil = Vec::new();
    evil.push(1);
    push_str(&mut evil, PKG);
    evil.push(9); // no such phase
    count(&mut evil, 0);
    count(&mut evil, 0);
    count(&mut evil, 0);
    evils.push(("invalid phase", evil));

    // misordered minted keys.
    let mut evil = Vec::new();
    evil.push(0);
    count(&mut evil, 2);
    push_str(&mut evil, "b");
    push_str(&mut evil, "a");
    count(&mut evil, 0);
    evils.push(("misordered minted keys", evil));

    // a failure log over its cap.
    let mut evil = Vec::new();
    evil.push(0);
    count(&mut evil, 0);
    count(&mut evil, (MAX_FAILURE_ROWS + 1) as u64);
    for i in 0..=MAX_FAILURE_ROWS {
        push_str(&mut evil, &format!("a{i}"));
        push_str(&mut evil, "tag");
        push_str(&mut evil, "reason");
    }
    evils.push(("failure log over cap", evil));

    // an empty package id.
    let mut evil = Vec::new();
    evil.push(1);
    push_str(&mut evil, "");
    evil.push(0);
    count(&mut evil, 0);
    count(&mut evil, 0);
    count(&mut evil, 0);
    evils.push(("empty package id", evil));

    // duplicate agent ids.
    let mut evil = Vec::new();
    evil.push(1);
    push_str(&mut evil, PKG);
    evil.push(0);
    count(&mut evil, 2);
    push_str(&mut evil, "docs.editor");
    push_str(&mut evil, "docs.editor");
    count(&mut evil, 0);
    count(&mut evil, 0);
    evils.push(("duplicate agents", evil));

    for (what, evil) in evils {
        let colluding = StateRoot(Sha256::digest(&evil).into());
        let mut target = module();
        assert!(
            target.install_snapshot(&evil, colluding).is_err(),
            "{what}: an execute-unreachable snapshot must reject even under a colluding root"
        );
    }
}
