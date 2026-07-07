//! module-level behavior of the package registry: install validation +
//! staging (row, routes, prompt-seed publishes, the harness follow-up), the
//! origin-gated lifecycle (`MarkActive` from the recorded harness only;
//! suspend/resume/unplug from installer or harness), unplug's audit-preserving
//! tombstone, route resolution over builtin + installed tags, and commit/abort
//! staging discipline.

use futures::executor::block_on;
use memory::{MemoryMsg, PublishBody, decode_msg as memory_decode_msg};
use package::{
    ActionRoute, AgentSeed, EngagementRule, HarnessMsg, InstallSpec, MANIFEST_HASH_LEN,
    ModuleBinding, PackageModule, PackageMsg, PackageQuery, PackageReply, PackageStatus,
    PromptSeed, UninstallPolicy, decode_harness_msg, decode_reply, encode_msg, encode_query,
};
use saga::SagaOrigin;
use sdk::{Ctx, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

const PACKAGE: &str = "package";
const MEMORY: &str = "memory";
const HARNESS: &str = "docs-harness";
const PKG: &str = "org.example.docs";

/// a minimal `Ctx`: controllable env/origin, a known-module set for the
/// binding existence checks, and captured follow-up msgs.
struct TestCtx {
    env: sdk::Env,
    /// module ids `module_root` reports as registered (binding targets).
    known_modules: Vec<String>,
    /// follow-up msgs emitted during execute, in order.
    emitted: Vec<Msg>,
}

impl TestCtx {
    fn with_origin(height: u64, origin: Origin) -> Self {
        Self {
            env: sdk::Env {
                protocol_version: 0,
                height,
                consensus_time: 0,
                origin,
                me: PACKAGE.into(),
            },
            known_modules: Vec::new(),
            emitted: Vec::new(),
        }
    }

    fn knowing(mut self, module_id: &str) -> Self {
        self.known_modules.push(module_id.to_string());
        self
    }

    /// the default full mesh a valid install needs.
    fn installing(height: u64, origin: Origin) -> Self {
        Self::with_origin(height, origin)
            .knowing(HARNESS)
            .knowing("pages")
            .knowing(MEMORY)
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.known_modules
            .iter()
            .any(|m| m == target)
            .then_some(StateRoot::ZERO)
    }

    async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::UnknownModule(target.into()))
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push(msg);
    }
    fn emit_event(&mut self, _ev: Event) {}
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

fn module() -> PackageModule {
    PackageModule::new(
        PACKAGE,
        MEMORY,
        vec![
            ("tasks.create".into(), "tasks".into()),
            ("tasks.update_status".into(), "tasks".into()),
        ],
    )
}

fn installer() -> Origin {
    Origin::External(b"alice".to_vec())
}

fn harness_origin() -> Origin {
    Origin::Module(HARNESS.into())
}

fn module_msg(payload: &PackageMsg) -> Msg {
    Msg {
        target: PACKAGE.into(),
        payload: encode_msg(payload),
    }
}

fn prompt_seed(logical: &str, path: &str, content: &str) -> PromptSeed {
    PromptSeed {
        logical: logical.into(),
        path: path.into(),
        content: content.into(),
        sha256: Sha256::digest(content.as_bytes()).to_vec(),
    }
}

/// a valid spec: the harness plus pages bound, one prompt, one agent, two
/// owned actions, one engagement rule.
fn spec() -> InstallSpec {
    InstallSpec {
        package: PKG.into(),
        version: "1.0.0".into(),
        manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
        modules: vec![
            ModuleBinding {
                logical: "harness".into(),
                module_id: HARNESS.into(),
            },
            ModuleBinding {
                logical: "pages".into(),
                module_id: "pages".into(),
            },
        ],
        harness: "harness".into(),
        prompts: vec![prompt_seed(
            "editor",
            "/packages/org.example.docs/prompts/editor.md",
            "be terse",
        )],
        agents: vec![AgentSeed {
            agent_id: "docs.editor".into(),
            display_name: "Docs Editor".into(),
            capability: "claude".into(),
            prompt: "editor".into(),
            actions: vec!["pages.comment.add".into()],
            active: true,
        }],
        actions: vec![
            ActionRoute {
                tag: "pages.comment.add".into(),
                owner: "harness".into(),
            },
            ActionRoute {
                tag: "pages.block.update_text".into(),
                owner: "harness".into(),
            },
        ],
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

fn exec(m: &mut PackageModule, ctx: &mut TestCtx, op: &PackageMsg) -> Result<(), Error> {
    block_on(m.execute(ctx, &module_msg(op)))
}

fn commit(m: &mut PackageModule) {
    block_on(m.commit_block()).unwrap();
}

fn abort(m: &mut PackageModule) {
    block_on(m.abort_block()).unwrap();
}

fn query(m: &PackageModule, q: &PackageQuery) -> PackageReply {
    decode_reply(&block_on(m.query(&encode_query(q))).unwrap()).unwrap()
}

fn action_owner(m: &PackageModule, tag: &str) -> Option<String> {
    match query(m, &PackageQuery::ActionOwner { tag: tag.into() }) {
        PackageReply::Owner(owner) => owner,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn get(m: &PackageModule, package: &str) -> Option<package::PackageView> {
    match query(
        m,
        &PackageQuery::Get {
            package: package.into(),
        },
    ) {
        PackageReply::Package(view) => view,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn routes_for(m: &PackageModule, module: &str) -> Vec<String> {
    match query(
        m,
        &PackageQuery::RoutesForOwner {
            module: module.into(),
        },
    ) {
        PackageReply::Routes(routes) => routes,
        other => panic!("unexpected reply: {other:?}"),
    }
}

/// install + MarkActive committed as one block — the healthy path every
/// lifecycle test starts from.
fn installed_active(m: &mut PackageModule) {
    let mut ctx = TestCtx::installing(1, installer());
    exec(m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    let mut ack = TestCtx::with_origin(1, harness_origin());
    exec(
        m,
        &mut ack,
        &PackageMsg::MarkActive {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(m);
}

// ---- install ---------------------------------------------------------------

#[test]
fn install_stages_row_and_routes_and_emits_seeds_then_harness_msg() {
    let mut m = module();
    let mut ctx = TestCtx::installing(5, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    commit(&mut m);

    // the row: Installing, installer recorded, harness mapped to its module id.
    let view = get(&m, PKG).expect("row staged");
    assert_eq!(view.status, PackageStatus::Installing);
    assert_eq!(view.installer, SagaOrigin::External(b"alice".to_vec()));
    assert_eq!(view.harness, HARNESS);
    assert_eq!(view.modules.get("pages"), Some(&"pages".to_string()));
    assert_eq!(view.installed_at, 5);
    assert_eq!(view.updated_at, 5);

    // the routes: staged and inventoried under the owner module either way,
    // but NOT yet live — the row is still Installing, so the registry itself
    // withholds resolution until MarkActive (requirement: registry-side
    // route liveness). `RoutesForOwner` is an inventory query (unaffected);
    // `ActionOwner` is the live-resolution one that now gates on `Active`.
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert_eq!(action_owner(&m, "pages.block.update_text"), None);
    assert_eq!(
        routes_for(&m, HARNESS),
        vec![
            "pages.block.update_text".to_string(),
            "pages.comment.add".to_string()
        ]
    );

    // emissions, in order: one memory publish per prompt seed, then the
    // harness install follow-up — all in the installer's block.
    assert_eq!(ctx.emitted.len(), 2);
    assert_eq!(ctx.emitted[0].target, MEMORY);
    match memory_decode_msg(&ctx.emitted[0].payload).unwrap() {
        MemoryMsg::Publish { path, body, .. } => {
            assert_eq!(path, "/packages/org.example.docs/prompts/editor.md");
            assert_eq!(body, PublishBody::Inline("be terse".into()));
        }
        other => panic!("expected a publish, got {other:?}"),
    }
    assert_eq!(ctx.emitted[1].target, HARNESS);
    match decode_harness_msg(&ctx.emitted[1].payload).unwrap() {
        HarnessMsg::InstallPackage { package, spec: fwd } => {
            assert_eq!(package, PKG);
            assert_eq!(fwd, spec());
        }
        other => panic!("expected InstallPackage, got {other:?}"),
    }
}

#[test]
fn install_requires_an_authenticated_external_or_module_origin() {
    let mut m = module();
    // the empty pre-consensus external origin never writes.
    let mut ctx = TestCtx::installing(1, Origin::External(Vec::new()));
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).is_err());
    // system is not an installer either (v1: authenticated members only).
    let mut ctx = TestCtx::installing(1, Origin::System);
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).is_err());
    // a module origin is a legitimate installer.
    let mut ctx = TestCtx::installing(1, Origin::Module("orchestrator".into()));
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    commit(&mut m);
    assert_eq!(
        get(&m, PKG).unwrap().installer,
        SagaOrigin::Module("orchestrator".into())
    );
}

#[test]
fn install_rejects_an_unknown_bound_module_id() {
    let mut m = module();
    // "pages" is NOT in the known set here, so its binding must reject.
    let mut ctx = TestCtx::with_origin(1, installer()).knowing(HARNESS);
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).is_err());
    assert!(
        ctx.emitted.is_empty(),
        "a rejected install must emit nothing"
    );
    commit(&mut m);
    assert_eq!(get(&m, PKG), None);
}

#[test]
fn install_rejects_a_route_collision_with_a_builtin_tag() {
    let mut m = module();
    let mut colliding = spec();
    colliding.actions.push(ActionRoute {
        tag: "tasks.create".into(),
        owner: "harness".into(),
    });
    let mut ctx = TestCtx::installing(1, installer());
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(colliding)).is_err());
    assert!(ctx.emitted.is_empty());
    commit(&mut m);
    // the builtin route is untouched and the row never landed.
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));
    assert_eq!(get(&m, PKG), None);
}

#[test]
fn install_rejects_a_route_collision_with_an_installed_tag() {
    let mut m = module();
    installed_active(&mut m);
    // a second package claiming an already-routed tag rejects, even staged
    // against committed state.
    let mut second = spec();
    second.package = "org.example.other".into();
    second.prompts[0].path = "/packages/org.example.other/prompts/editor.md".into();
    let mut ctx = TestCtx::installing(2, installer());
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(second)).is_err());
    commit(&mut m);
    assert_eq!(get(&m, "org.example.other"), None);
}

#[test]
fn install_rejects_a_duplicate_package_id() {
    let mut m = module();
    installed_active(&mut m);
    // same id, colliding tags stripped — the ID collision itself must reject.
    let mut again = spec();
    again.actions.clear();
    again.agents.clear();
    again.engagements.clear();
    let mut ctx = TestCtx::installing(2, installer());
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(again)).is_err());
}

#[test]
fn install_rejects_a_same_block_duplicate_package_id() {
    // two installs of the SAME package id in the SAME block, with no commit
    // between them: the second must see the first's STAGED (uncommitted)
    // row — `validate_spec` reads `store()`, which is pending-if-present —
    // and collide, exactly like the cross-block case.
    let mut m = module();
    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    assert!(
        exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).is_err(),
        "a same-block second install of the same id must collide"
    );
    commit(&mut m);
    // exactly one row landed, from the first install only.
    let view = get(&m, PKG).expect("the first install's row");
    assert_eq!(view.status, PackageStatus::Installing);
    match query(&m, &PackageQuery::List) {
        PackageReply::Packages(list) => assert_eq!(list.len(), 1),
        other => panic!("unexpected reply: {other:?}"),
    }
}

/// one labelled way to break an otherwise-valid spec.
type SpecBreaker = (&'static str, Box<dyn Fn(&mut InstallSpec)>);

#[test]
fn install_validates_the_spec_shape() {
    let cases: Vec<SpecBreaker> = vec![
        ("bad package id", Box::new(|s| s.package = "UPPER".into())),
        ("empty version", Box::new(|s| s.version = String::new())),
        (
            "short manifest hash",
            Box::new(|s| s.manifest_hash = vec![7u8; 31]),
        ),
        ("unmapped harness", Box::new(|s| s.harness = "ghost".into())),
        (
            "mispinned prompt",
            Box::new(|s| s.prompts[0].sha256 = vec![9u8; 32]),
        ),
        (
            "relative prompt path",
            Box::new(|s| s.prompts[0].path = "prompts/editor.md".into()),
        ),
        (
            "agent with an unknown prompt",
            Box::new(|s| s.agents[0].prompt = "ghost".into()),
        ),
        (
            "agent granted an undeclared action",
            Box::new(|s| s.agents[0].actions.push("tasks.create".into())),
        ),
        (
            "action owned by an unmapped logical",
            Box::new(|s| s.actions[0].owner = "ghost".into()),
        ),
        (
            "engagement from an unmapped source",
            Box::new(|s| s.engagements[0].source = "ghost".into()),
        ),
        (
            "engagement for an undeclared agent",
            Box::new(|s| s.engagements[0].agent = "ghost".into()),
        ),
        (
            "unknown pending_runs policy",
            Box::new(|s| s.uninstall.pending_runs = "explode".into()),
        ),
        (
            "non-preserve user_data policy",
            Box::new(|s| s.uninstall.user_data = "delete".into()),
        ),
    ];
    for (what, mutate) in cases {
        let mut m = module();
        let mut bad = spec();
        mutate(&mut bad);
        let mut ctx = TestCtx::installing(1, installer());
        assert!(
            exec(&mut m, &mut ctx, &PackageMsg::Install(bad)).is_err(),
            "{what} must reject"
        );
        assert!(ctx.emitted.is_empty(), "{what} must emit nothing");
    }
}

// ---- MarkActive ------------------------------------------------------------

#[test]
fn mark_active_accepts_only_the_recorded_harness_origin() {
    let mut m = module();
    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();

    let op = PackageMsg::MarkActive {
        package: PKG.into(),
    };
    // wrong origins: the installer, an unrelated module, an external key.
    for origin in [
        installer(),
        Origin::Module("pages".into()),
        Origin::External(b"mallory".to_vec()),
        Origin::System,
    ] {
        let mut wrong = TestCtx::with_origin(1, origin.clone());
        assert!(
            exec(&mut m, &mut wrong, &op).is_err(),
            "{origin:?} must not activate"
        );
    }

    // the recorded harness's module origin flips Installing -> Active.
    let mut ack = TestCtx::with_origin(1, harness_origin());
    exec(&mut m, &mut ack, &op).unwrap();
    commit(&mut m);
    assert_eq!(get(&m, PKG).unwrap().status, PackageStatus::Active);
}

#[test]
fn mark_active_requires_an_installing_row() {
    let mut m = module();
    let op = PackageMsg::MarkActive {
        package: PKG.into(),
    };
    // no such package.
    let mut ctx = TestCtx::with_origin(1, harness_origin());
    assert!(exec(&mut m, &mut ctx, &op).is_err());
    // already active.
    installed_active(&mut m);
    let mut ctx = TestCtx::with_origin(2, harness_origin());
    assert!(exec(&mut m, &mut ctx, &op).is_err());
}

// ---- suspend / resume / unplug ----------------------------------------------

#[test]
fn suspend_and_resume_flip_status_and_emit_the_harness_msgs() {
    let mut m = module();
    installed_active(&mut m);

    // a stranger (module or external) may not suspend.
    for origin in [
        Origin::External(b"mallory".to_vec()),
        Origin::Module("pages".into()),
    ] {
        let mut wrong = TestCtx::with_origin(2, origin.clone());
        assert!(
            exec(
                &mut m,
                &mut wrong,
                &PackageMsg::Suspend {
                    package: PKG.into()
                }
            )
            .is_err(),
            "{origin:?} must not suspend"
        );
    }

    // the installer origin suspends; the matching HarnessMsg rides the block.
    let mut ctx = TestCtx::with_origin(2, installer());
    exec(
        &mut m,
        &mut ctx,
        &PackageMsg::Suspend {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    let view = get(&m, PKG).unwrap();
    assert_eq!(view.status, PackageStatus::Suspended);
    assert_eq!(view.updated_at, 2);
    assert_eq!(ctx.emitted.len(), 1);
    assert_eq!(ctx.emitted[0].target, HARNESS);
    assert_eq!(
        decode_harness_msg(&ctx.emitted[0].payload).unwrap(),
        HarnessMsg::SuspendPackage {
            package: PKG.into()
        }
    );

    // the route entry itself stays registered (suspend disables activity,
    // not audit) — `RoutesForOwner` still lists it — but `ActionOwner` no
    // longer resolves it while suspended: the registry enforces the suspend
    // guarantee itself now, rather than trusting every owner module to
    // self-gate on phase.
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert!(routes_for(&m, HARNESS).contains(&"pages.comment.add".to_string()));
    // re-suspending a suspended package rejects.
    let mut again = TestCtx::with_origin(3, installer());
    assert!(
        exec(
            &mut m,
            &mut again,
            &PackageMsg::Suspend {
                package: PKG.into()
            }
        )
        .is_err()
    );

    // the harness origin resumes (either owner works).
    let mut ctx = TestCtx::with_origin(3, harness_origin());
    exec(
        &mut m,
        &mut ctx,
        &PackageMsg::Resume {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    assert_eq!(get(&m, PKG).unwrap().status, PackageStatus::Active);
    assert_eq!(ctx.emitted.len(), 1);
    assert_eq!(
        decode_harness_msg(&ctx.emitted[0].payload).unwrap(),
        HarnessMsg::ResumePackage {
            package: PKG.into()
        }
    );
    // Active again: the route resolves once more.
    assert_eq!(action_owner(&m, "pages.comment.add"), Some(HARNESS.into()));

    // resuming an already-active package rejects.
    let mut again = TestCtx::with_origin(4, installer());
    assert!(
        exec(
            &mut m,
            &mut again,
            &PackageMsg::Resume {
                package: PKG.into()
            }
        )
        .is_err()
    );
}

#[test]
fn unplug_tombstones_the_row_and_removes_only_its_routes() {
    let mut m = module();
    installed_active(&mut m);

    let mut ctx = TestCtx::with_origin(7, installer());
    exec(
        &mut m,
        &mut ctx,
        &PackageMsg::Unplug {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);

    // the harness follow-up rides the block.
    assert_eq!(ctx.emitted.len(), 1);
    assert_eq!(ctx.emitted[0].target, HARNESS);
    assert_eq!(
        decode_harness_msg(&ctx.emitted[0].payload).unwrap(),
        HarnessMsg::UnplugPackage {
            package: PKG.into()
        }
    );

    // the row is tombstoned, not deleted (audit-preserving)...
    let view = get(&m, PKG).expect("tombstone preserved");
    assert_eq!(view.status, PackageStatus::Inactive);
    assert_eq!(view.updated_at, 7);

    // ...but its routes are gone, and the builtins survive.
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert_eq!(action_owner(&m, "pages.block.update_text"), None);
    assert!(routes_for(&m, HARNESS).is_empty());
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));

    // an unplugged package accepts no further lifecycle ops.
    for op in [
        PackageMsg::Suspend {
            package: PKG.into(),
        },
        PackageMsg::Resume {
            package: PKG.into(),
        },
        PackageMsg::Unplug {
            package: PKG.into(),
        },
    ] {
        let mut again = TestCtx::with_origin(8, installer());
        assert!(exec(&mut m, &mut again, &op).is_err(), "{op:?} must reject");
    }

    // and its id stays claimed: a reinstall under the tombstoned id rejects.
    let mut ctx = TestCtx::installing(9, installer());
    assert!(exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).is_err());
}

#[test]
fn unplug_recovers_a_row_wedged_in_installing() {
    // a future no-fail intake / wasm harness could silently drop its
    // `InstallPackage` follow-up and never send `MarkActive`, wedging the row
    // in `Installing` forever — `Suspend` (requires Active) and `Resume`
    // (requires Suspended) both refuse it. `Unplug` is the escape hatch.
    let mut m = module();
    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    commit(&mut m);
    assert_eq!(get(&m, PKG).unwrap().status, PackageStatus::Installing);

    // Suspend/Resume both still refuse an Installing row.
    let mut wrong = TestCtx::with_origin(2, installer());
    assert!(
        exec(
            &mut m,
            &mut wrong,
            &PackageMsg::Suspend {
                package: PKG.into()
            }
        )
        .is_err()
    );
    let mut wrong = TestCtx::with_origin(2, installer());
    assert!(
        exec(
            &mut m,
            &mut wrong,
            &PackageMsg::Resume {
                package: PKG.into()
            }
        )
        .is_err()
    );

    // Unplug succeeds from Installing: tombstoned, routes gone.
    let mut ctx = TestCtx::with_origin(2, installer());
    exec(
        &mut m,
        &mut ctx,
        &PackageMsg::Unplug {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    let view = get(&m, PKG).expect("tombstone preserved");
    assert_eq!(view.status, PackageStatus::Inactive);
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert!(routes_for(&m, HARNESS).is_empty());
}

#[test]
fn empty_external_origin_never_authorizes_a_lifecycle_op() {
    // the empty pre-consensus external key can never be a recorded installer
    // (install rejects it), and it is not the harness origin either — so it
    // must never authorize Suspend/Resume/Unplug, even by accident.
    let empty = Origin::External(Vec::new());

    let mut m = module();
    installed_active(&mut m);
    let mut ctx = TestCtx::with_origin(2, empty.clone());
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &PackageMsg::Suspend {
                package: PKG.into()
            }
        )
        .is_err(),
        "the empty external origin must not suspend"
    );

    let mut suspended = module();
    installed_active(&mut suspended);
    let mut ctx = TestCtx::with_origin(2, installer());
    exec(
        &mut suspended,
        &mut ctx,
        &PackageMsg::Suspend {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut suspended);
    let mut ctx = TestCtx::with_origin(3, empty.clone());
    assert!(
        exec(
            &mut suspended,
            &mut ctx,
            &PackageMsg::Resume {
                package: PKG.into()
            }
        )
        .is_err(),
        "the empty external origin must not resume"
    );

    let mut ctx = TestCtx::with_origin(3, empty);
    assert!(
        exec(
            &mut suspended,
            &mut ctx,
            &PackageMsg::Unplug {
                package: PKG.into()
            }
        )
        .is_err(),
        "the empty external origin must not unplug"
    );
}

// ---- queries ----------------------------------------------------------------

#[test]
fn action_owner_only_resolves_active_rows_and_always_resolves_builtins() {
    let mut m = module();
    // builtins resolve before any install exists.
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));

    // Installing: staged and routed, but not yet LIVE.
    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    commit(&mut m);
    assert_eq!(get(&m, PKG).unwrap().status, PackageStatus::Installing);
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));

    // Active: now it resolves.
    let mut ack = TestCtx::with_origin(1, harness_origin());
    exec(
        &mut m,
        &mut ack,
        &PackageMsg::MarkActive {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    assert_eq!(action_owner(&m, "pages.comment.add"), Some(HARNESS.into()));

    // Suspended: withheld again.
    let mut suspend = TestCtx::with_origin(2, installer());
    exec(
        &mut m,
        &mut suspend,
        &PackageMsg::Suspend {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));

    // Resumed: resolves once more, and the builtin never wavered.
    let mut resume = TestCtx::with_origin(3, installer());
    exec(
        &mut m,
        &mut resume,
        &PackageMsg::Resume {
            package: PKG.into(),
        },
    )
    .unwrap();
    commit(&mut m);
    assert_eq!(action_owner(&m, "pages.comment.add"), Some(HARNESS.into()));
}

#[test]
fn action_owner_resolves_builtin_and_installed_tags() {
    let mut m = module();
    // builtins resolve from genesis, before any install.
    assert_eq!(action_owner(&m, "tasks.create"), Some("tasks".into()));
    assert_eq!(
        action_owner(&m, "tasks.update_status"),
        Some("tasks".into())
    );
    assert_eq!(action_owner(&m, "pages.comment.add"), None);
    assert_eq!(routes_for(&m, "tasks").len(), 2);

    installed_active(&mut m);
    assert_eq!(action_owner(&m, "pages.comment.add"), Some(HARNESS.into()));

    // List serves every row.
    match query(&m, &PackageQuery::List) {
        PackageReply::Packages(list) => {
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].package, PKG);
            assert_eq!(list[0].status, PackageStatus::Active);
        }
        other => panic!("unexpected reply: {other:?}"),
    }
}

// ---- staging discipline -------------------------------------------------------

#[test]
fn abort_discards_a_staged_install_and_queries_serve_committed_state() {
    let mut m = module();
    let genesis_root = m.root();
    assert_ne!(
        genesis_root,
        StateRoot::ZERO,
        "builtin routes seed the root"
    );

    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    // queries observe COMMITTED state only: the staged row is not visible.
    assert_eq!(get(&m, PKG), None);
    abort(&mut m);
    assert_eq!(m.root(), genesis_root, "abort must leave no trace");
    assert_eq!(get(&m, PKG), None);

    // the same install lands cleanly afterwards.
    let mut ctx = TestCtx::installing(1, installer());
    exec(&mut m, &mut ctx, &PackageMsg::Install(spec())).unwrap();
    commit(&mut m);
    assert_ne!(m.root(), genesis_root);
    assert!(get(&m, PKG).is_some());
}
