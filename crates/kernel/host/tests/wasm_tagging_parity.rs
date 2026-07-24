//! the adapter-port equivalence proof for the tagging cutover: the
//! `tagging` guest component (the NATIVE `tagging` crate compiled to wasm behind
//! `guest-adapter`) and the native `TaggingModule` answer the SAME op sequence
//! with IDENTICAL routing decisions, and their roots move in lockstep (move on
//! commit, hold on no-ops and abort). the roots THEMSELVES differ from the
//! first committed write — the port persists the native canonical snapshot as
//! one host-KV value, an intentional greenfield root break pinned by this proof.
//!
//! tagging is a CROSS-MODULE plane, which is the point of this tenant: a
//! Subscribe's acceptance depends on a SIBLING read (`ctx.module_root(source)`)
//! and a Tag's direct-owner route on `ctx.module_root(tag.module)` — inside the
//! guest those resolve through the wasm runtime's memoized-replay machinery
//! under real dispatch. both hosts therefore carry REAL sibling modules
//! ([`Recorder`]s under the ids the plane resolves and routes to), and the
//! plane's observable read surface is what it DELIVERS to them (tagging itself
//! has no query surface).

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};
use tagging::{Author, EntityRef, TagEvent, TaggingModule, TaggingMsg, decode_event, encode_msg};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `tagging` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const TAGGING_WASM: &[u8] = include_bytes!("fixtures/tagging.component.wasm");

/// the plane truncates one event's tag list to this many entries and caps a
/// scope at this many subscribers. the native crate keeps the constants
/// private; the matrix pins the OBSERVABLE behavior, so a native change to
/// either bound fails here on both runtimes at once.
const MAX_TAGS_PER_EVENT: usize = 16;
const MAX_SUBSCRIBERS_PER_SCOPE: usize = 8;

fn wasm_tagging() -> WasmModule {
    WasmModule::from_bytes("tagging", TAGGING_WASM).expect("load component")
}

/// EXACTLY the production wiring in bin/node's host state — the direct-owner
/// set is genesis config, so both runtimes (and the guest itself) must wire
/// the same set or the routing decisions fork.
fn native_tagging() -> TaggingModule {
    TaggingModule::new("tagging").with_direct_owner("runs")
}

/// a REAL sibling standing in for the modules the plane reads and routes to:
/// its registration is what `ctx.module_root` resolves, and it records every
/// follow-up `Msg` it receives — under the native staging contract, so an
/// aborted block leaves no trace here either — serving the committed log via
/// its query surface. the DELIVERED stream is the plane's observable output,
/// so comparing recorder logs across runtimes is the routing-parity claim.
struct Recorder {
    id: ModuleId,
    /// committed (origin actor, payload) deliveries — what `root()` and the
    /// query surface commit to.
    committed: Vec<(String, Vec<u8>)>,
    /// this block's staged deliveries, published at `commit_block`.
    staged: Vec<(String, Vec<u8>)>,
}

impl Recorder {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            committed: Vec::new(),
            staged: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Recorder {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        if self.committed.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        for (who, payload) in &self.committed {
            h.update((who.len() as u64).to_le_bytes());
            h.update(who.as_bytes());
            h.update((payload.len() as u64).to_le_bytes());
            h.update(payload);
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged
            .push((ctx.env().origin.actor_string(), msg.payload.clone()));
        Ok(())
    }

    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.committed).expect("serializable"))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.committed.append(&mut self.staged);
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

/// the sibling set both hosts carry: "chat" (a content source the Subscribe
/// registry check resolves), "agent" (a scope subscriber the plane routes to),
/// "runs" (the genesis-configured direct owner).
fn siblings() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(Recorder::new("chat")),
        Box::new(Recorder::new("agent")),
        Box::new(Recorder::new("runs")),
    ]
}

fn native_host() -> Host {
    let mut modules: Vec<Box<dyn Module>> = vec![Box::new(native_tagging())];
    modules.extend(siblings());
    Host::genesis(modules).expect("genesis")
}

fn wasm_host_() -> Host {
    let mut modules: Vec<Box<dyn Module>> = vec![Box::new(wasm_tagging())];
    modules.extend(siblings());
    Host::genesis(modules).expect("genesis")
}

fn op(m: &TaggingMsg) -> Msg {
    Msg {
        target: "tagging".into(),
        payload: encode_msg(m),
    }
}

fn from_module(id: &str) -> Origin {
    Origin::Module(id.into())
}

fn subscribe(source: &str, container: &str) -> TaggingMsg {
    TaggingMsg::Subscribe {
        source: source.into(),
        container: container.into(),
    }
}

fn unsubscribe(source: &str, container: &str) -> TaggingMsg {
    TaggingMsg::Unsubscribe {
        source: source.into(),
        container: container.into(),
    }
}

fn user_tag(container: &str, seq: u64, tags: Vec<EntityRef>) -> TaggingMsg {
    TaggingMsg::Tag(TagEvent {
        container: container.into(),
        content_seq: seq,
        author: Author::User(b"human".to_vec()),
        tags,
    })
}

fn runs_ref(entity: &str) -> EntityRef {
    EntityRef {
        module: "runs".into(),
        entity: entity.into(),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
/// tagging ops are module-origin only, so the origin is always a parameter.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

/// the read matrix: the plane has no query surface of its own, so its
/// observable reads are the DELIVERED streams of every sibling recorder.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for target in ["chat", "agent", "runs"] {
        out.push(h.query(target, &[]).await.expect("recorder query"));
    }
    out
}

/// one recorder's committed delivery log, decoded.
async fn delivered(h: &Host, target: &str) -> Vec<(String, Vec<u8>)> {
    let reply = h.query(target, &[]).await.expect("recorder query");
    serde_json::from_slice(&reply).expect("recorder log decodes")
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("tagging").expect("tagging registered")
}

fn event_tuples(events: &[Event]) -> Vec<(String, Vec<u8>)> {
    events
        .iter()
        .map(|e| (e.source.clone(), e.payload.clone()))
        .collect()
}

#[test]
fn same_ops_same_routing_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();

    // at GENESIS the roots COINCIDE by construction: tagging's canonical
    // encoding of empty state is a lone zero count — byte-identical to the
    // empty host-KV store's encoding, hashed by the same sha256. the declared
    // schema break manifests on the FIRST WRITE (asserted per block below),
    // which is what the revision-2 fence actually guards.
    let genesis_root = root_of(&native);
    assert_eq!(
        genesis_root,
        root_of(&wasm),
        "empty-state roots coincide by construction"
    );

    // every op family, in one deterministic sequence. `moves` says whether the
    // op changes committed tagging state — root movement must agree on BOTH
    // sides. tag intakes NEVER move the root (the plane stages nothing for
    // them; their observable effect is routing, asserted via the recorders).
    let ops: Vec<(Origin, Msg, bool)> = vec![
        // h1: acceptance DEPENDS on the sibling read: module_root("chat") must
        // resolve Some through the wasm runtime's memoized replay.
        (
            from_module("agent"),
            op(&subscribe("chat", "general")),
            true,
        ),
        // h2: idempotent re-subscribe stages nothing.
        (
            from_module("agent"),
            op(&subscribe("chat", "general")),
            false,
        ),
        // h3: a second subscriber on the same scope.
        (from_module("runs"), op(&subscribe("chat", "general")), true),
        // h4: user-authored tagged content — fan-out to both subscribers, the
        // direct-owner mention deduped against the scope subscription. the
        // direct-owner route DEPENDS on module_root("runs") resolving through
        // the wasm runtime.
        (
            from_module("chat"),
            op(&user_tag("general", 7, vec![runs_ref("qa-luna")])),
            false,
        ),
        // h5: THE LOOP RULE — an entity-authored event must never fire.
        (
            from_module("chat"),
            op(&TaggingMsg::Tag(TagEvent {
                container: "general".into(),
                content_seq: 8,
                author: Author::Entity(runs_ref("qa-luna")),
                tags: vec![runs_ref("qa-luna")],
            })),
            false,
        ),
        // h6: an unsubscribed container delivers nothing.
        (
            from_module("chat"),
            op(&user_tag("other", 1, vec![])),
            false,
        ),
        // h7: a DIFFERENT source module, no scope subscription — the explicit
        // entity mention still reaches the direct owner (and only it).
        (
            from_module("pages"),
            op(&user_tag("thread-1", 2, vec![runs_ref("qa-luna")])),
            false,
        ),
        // h8: an overlong tag list truncates deterministically, never rejects.
        (
            from_module("chat"),
            op(&user_tag(
                "general",
                9,
                (0..MAX_TAGS_PER_EVENT + 4)
                    .map(|i| EntityRef {
                        module: "agent".into(),
                        entity: format!("bot{i}"),
                    })
                    .collect(),
            )),
            false,
        ),
        // h9: malformed tags are filtered (with an observability note);
        // well-formed ones still deliver.
        (
            from_module("chat"),
            op(&user_tag(
                "general",
                10,
                vec![
                    EntityRef {
                        module: String::new(),
                        entity: "x".into(),
                    },
                    runs_ref("qa-luna"),
                ],
            )),
            false,
        ),
        // h10: a malformed container is dropped (note), never an error — the
        // no-fail intake rides the content's block.
        (from_module("chat"), op(&user_tag("", 11, vec![])), false),
        // h11: a MODULE's undecodable bytes are a staged no-op (someone else's
        // block), never an abort.
        (
            from_module("chat"),
            Msg {
                target: "tagging".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            false,
        ),
        // h12/h13: unsubscribe moves the root once; repeating it stages nothing.
        (
            from_module("agent"),
            op(&unsubscribe("chat", "general")),
            true,
        ),
        (
            from_module("agent"),
            op(&unsubscribe("chat", "general")),
            false,
        ),
        // h14: the remaining subscriber still receives.
        (
            from_module("chat"),
            op(&user_tag("general", 12, vec![])),
            false,
        ),
        // h15: the last subscriber's departure removes the scope — the native
        // root returns to its empty value.
        (
            from_module("runs"),
            op(&unsubscribe("chat", "general")),
            true,
        ),
        // h16: nothing subscribed, nothing mentioned — silence.
        (
            from_module("chat"),
            op(&user_tag("general", 13, vec![])),
            false,
        ),
    ];

    for (height, (origin, msg, moves)) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        let n_out = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect("native submit");
        let w_out = wasm
            .submit_at(block(height, origin), msg)
            .await
            .expect("wasm submit");

        // the deterministic dispatch trace is identical: same dispatches, same
        // follow-up fan-out counts, same observability events — the strongest
        // per-block equivalence short of the (deliberately different) roots.
        assert_eq!(
            n_out.dispatches, w_out.dispatches,
            "dispatch traces diverge at block {height}"
        );
        assert_eq!(
            event_tuples(&n_out.events),
            event_tuples(&w_out.events),
            "events diverge at block {height}"
        );
        // the delivered streams are identical after every block.
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "deliveries diverge after block {height}"
        );
        // roots move in LOCKSTEP: a state-changing op moves both commit
        // boundaries, a no-op holds both...
        if moves {
            assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
            assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        } else {
            assert_eq!(root_of(&native), n_before, "native root moved at {height}");
            assert_eq!(root_of(&wasm), w_before, "wasm root moved at {height}");
        }
        // ...and from the first committed write on, the roots themselves
        // always differ (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // the emptied plane pins the schema-break asymmetry: the native root is
    // back at its genesis value, while the wasm store now carries an EXPLICIT
    // empty snapshot under its reserved keys — never the genesis store again.
    assert_eq!(root_of(&native), genesis_root, "native root back to empty");
    assert_ne!(
        root_of(&wasm),
        genesis_root,
        "the wasm store never returns to its pre-first-write shape"
    );

    // decoded spot checks on the wasm side: every delivery came from the
    // plane (Origin::Module("tagging")), with the source verified by origin.
    let agent_log = delivered(&wasm, "agent").await;
    assert_eq!(agent_log.len(), 3, "agent: h4, h8, h9");
    assert!(agent_log.iter().all(|(who, _)| who == "tagging"));
    let ev = decode_event(&agent_log[0].1).expect("decode");
    assert_eq!(ev.source, "chat");
    assert_eq!(ev.container, "general");
    assert_eq!(ev.content_seq, 7);
    assert_eq!(ev.author, Author::User(b"human".to_vec()));
    assert_eq!(ev.tags, vec![runs_ref("qa-luna")]);
    let ev = decode_event(&agent_log[1].1).expect("decode");
    assert_eq!(ev.content_seq, 9);
    assert_eq!(ev.tags.len(), MAX_TAGS_PER_EVENT, "list truncated");
    assert_eq!(ev.tags[0].entity, "bot0");
    let ev = decode_event(&agent_log[2].1).expect("decode");
    assert_eq!(ev.content_seq, 10);
    assert_eq!(ev.tags, vec![runs_ref("qa-luna")], "malformed tag filtered");

    let runs_log = delivered(&wasm, "runs").await;
    assert_eq!(runs_log.len(), 5, "runs: h4, h7, h8, h9, h14");
    let ev = decode_event(&runs_log[1].1).expect("decode");
    assert_eq!(ev.source, "pages", "the SOURCE is the dispatch origin");
    assert_eq!(ev.container, "thread-1");
    assert_eq!(ev.content_seq, 2);
    let ev = decode_event(&runs_log[4].1).expect("decode");
    assert_eq!(ev.content_seq, 12);
    assert!(ev.tags.is_empty());

    assert!(
        delivered(&wasm, "chat").await.is_empty(),
        "a source module receives nothing"
    );

    // the plane has no query surface: both runtimes refuse a direct read, the
    // wasm side carrying the native refusal through the wit rendering.
    assert!(native.query("tagging", b"{}").await.is_err());
    let w_err = wasm.query("tagging", b"{}").await.expect_err("unsupported");
    assert!(
        w_err.to_string().contains("QueryUnsupported"),
        "got {w_err:?}"
    );

    // queries are read-only on the wasm side too: the root is STABLE across
    // the whole read matrix (and the refused direct read).
    let settled = root_of(&wasm);
    let _ = replies(&wasm).await;
    let _ = wasm.query("tagging", b"{}").await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = vec![0xA1u8; 32];

    // seed one committed subscription, then fill a second scope to its cap so
    // the cap rejection is reachable (the "m{i}" subscribers are origins only
    // — nothing ever tags "crowded", so they never need to be registered).
    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, from_module("agent")),
            op(&subscribe("chat", "general")),
        )
        .await
        .expect("seed subscribe");
        for i in 0..MAX_SUBSCRIBERS_PER_SCOPE {
            host.submit_at(
                block(2 + i as u64, from_module(&format!("m{i}"))),
                op(&subscribe("chat", "crowded")),
            )
            .await
            .expect("cap-fill subscribe");
        }
    }

    // the rejection matrix: every distinct refusal family the native module
    // implements — the origin gate, the sibling-read registry check (resolved
    // through the wasm runtime's memoized replay), every id-shape violation,
    // the subscriber cap, and an EXTERNAL submitter's undecodable bytes (a
    // module's undecodable bytes are a no-op, asserted in the accept matrix).
    // each rejected block must leave BOTH roots byte-identical (the abort
    // path: staged writes discarded, no trace).
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            Origin::External(alice.clone()),
            op(&subscribe("chat", "x")),
            "module-origin only",
        ),
        (
            Origin::System,
            op(&subscribe("chat", "x")),
            "module-origin only",
        ),
        (
            Origin::External(alice.clone()),
            op(&user_tag("general", 1, vec![])),
            "module-origin only",
        ),
        (
            from_module("agent"),
            op(&subscribe("ghost", "c")),
            "not a registered module",
        ),
        (
            from_module("agent"),
            op(&subscribe("", "c")),
            "source must be non-empty",
        ),
        (
            from_module("agent"),
            op(&subscribe("chat", "")),
            "container must be non-empty",
        ),
        (
            from_module("agent"),
            op(&subscribe("chat", "a\u{1f}b")),
            "reserved separator",
        ),
        (
            from_module("agent"),
            op(&subscribe("chat", &"c".repeat(129))),
            "the cap is 128",
        ),
        (
            from_module("one-too-many"),
            op(&subscribe("chat", "crowded")),
            "subscribers already",
        ),
        (
            Origin::External(alice.clone()),
            Msg {
                target: "tagging".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (i, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = i as u64 + 2 + MAX_SUBSCRIBERS_PER_SCOPE as u64;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), msg)
            .await
            .expect_err("wasm must reject");

        // both reject DETERMINISTICALLY with the native module's reason. the
        // wasm runtime wraps the reason in its wit-error rendering, so the
        // parity claim is containment, not string equality.
        let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
            panic!("native rejection shape: {n_err:?}");
        };
        let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
            panic!("wasm rejection shape: {w_err:?}");
        };
        assert!(n_msg.contains(needle), "native reason: {n_msg}");
        assert!(
            w_msg.contains(needle),
            "wasm reason must carry the native reason: {w_msg}"
        );

        // abort leaves no trace: both roots byte-identical to pre-block.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(replies(&native).await, replies(&wasm).await);
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();

    // ONE block, two ops: the tag's fan-out READS the subscription the first
    // op staged (the scope only exists in this block's overlay). on the wasm
    // side that is the outer staged `__state` being reloaded by the second
    // dispatch — the read-your-writes seam the adapter relies on — WHILE the
    // dispatch also resolves its sibling reads through the memoized replay.
    let batch = vec![
        (from_module("agent"), op(&subscribe("chat", "hot"))),
        (from_module("chat"), op(&user_tag("hot", 1, vec![]))),
    ];
    let n_out = native
        .submit_block(block(1, from_module("chat")), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, from_module("chat")), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "both members must apply: {:?}",
            out.members
        );
    }
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        assert_eq!(
            delivered(host, "agent").await.len(),
            1,
            "the tag saw the SAME block's staged subscription"
        );
    }

    // the mirror image: a staged UNSUBSCRIBE is visible to a later dispatch of
    // the same block — the second tag delivers nothing.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (from_module("agent"), op(&unsubscribe("chat", "hot"))),
        (from_module("chat"), op(&user_tag("hot", 2, vec![]))),
    ];
    let n_out = native
        .submit_block(block(2, from_module("chat")), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, from_module("chat")), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "both members must apply: {:?}",
            out.members
        );
    }
    assert_ne!(root_of(&native), n_before, "the unsubscribe committed");
    assert_ne!(root_of(&wasm), w_before, "the unsubscribe committed");
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        assert_eq!(
            delivered(host, "agent").await.len(),
            1,
            "the staged unsubscribe silenced the same block's tag"
        );
    }

    // ONE block where the SECOND member rejects — and the rejection itself is
    // decided by a SIBLING READ (module_root("ghost") resolves None through
    // the wasm runtime): the runtime aborts the staged overlay and replays the
    // accepted member — committed state must equal the accepted subset alone,
    // on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (from_module("agent"), op(&subscribe("chat", "warm"))),
        (from_module("agent"), op(&subscribe("ghost", "x"))),
    ];
    let n_out = native
        .submit_block(block(3, from_module("agent")), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(3, from_module("agent")), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    // the accepted member landed (roots moved), the rejected one left nothing.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(replies(&native).await, replies(&wasm).await);

    // and the accepted subscription is LIVE: a tag on "warm" delivers.
    for (h, host) in [(4u64, &mut native), (4u64, &mut wasm)] {
        host.submit_at(
            block(h, from_module("chat")),
            op(&user_tag("warm", 3, vec![])),
        )
            .await
            .expect("tag on the accepted subscription");
    }
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        assert_eq!(
            delivered(host, "agent").await.len(),
            2,
            "the accepted member's subscription survived the batch replay"
        );
    }
}
