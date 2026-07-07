//! snapshot/install round-trip for the package registry: committed state
//! covering both installer origin shapes and the Installing/Active/Suspended/
//! Inactive statuses — built through the real ordered-op path — crosses to a
//! fresh module as canonical bytes and re-derives the identical root, with
//! query parity. the bytes arrive UNTRUSTED (a byzantine peer serves them), so
//! the flip side is exercised too: tampered, truncated, padded, misordered,
//! and bad-discriminant snapshots are rejected and the target module is left
//! byte-identical to before the call. because `install` authenticates the
//! BYTES first, the strict-decode cases are driven under a COLLUDING root
//! (sha256 of the evil bytes): even that must not smuggle in an
//! execute-unreachable state.

use futures::executor::block_on;
use package::{
    ActionRoute, InstallSpec, MANIFEST_HASH_LEN, ModuleBinding, PackageModule, PackageMsg,
    PackageQuery, PackageReply, PackageStatus, PromptSeed, UninstallPolicy, decode_reply,
    encode_msg, encode_query,
};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

/// a minimal `Ctx`: controllable env/origin plus a known-module set for the
/// install binding checks; emitted follow-ups are dropped (the harness half
/// is not under test).
struct TestCtx {
    env: Env,
    known_modules: Vec<String>,
}

impl TestCtx {
    fn new(height: u64, origin: Origin) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height,
                consensus_time: height,
                origin,
                me: "package".into(),
            },
            known_modules: vec![
                "docs-harness".into(),
                "notes-harness".into(),
                "old-harness".into(),
                "hmod".into(),
                // 7 bytes, matching "package" (this module's own id, `me`
                // above) — a strict-decode test splices this placeholder to
                // the registry's own id in place, without touching any
                // length prefix.
                "hmodxyz".into(),
            ],
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
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
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
    fn request_effect(&mut self, _eff: Effect) {}
}

fn module() -> PackageModule {
    PackageModule::new(
        "package",
        "memory",
        vec![
            ("tasks.create".into(), "tasks".into()),
            ("tasks.update_status".into(), "tasks".into()),
        ],
    )
}

fn exec(m: &mut PackageModule, ctx: TestCtx, op: &PackageMsg) {
    let mut ctx = ctx;
    let msg = Msg {
        target: "package".into(),
        payload: encode_msg(op),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
}

fn commit(m: &mut PackageModule) {
    block_on(m.commit_block()).unwrap();
}

fn query_reply(m: &PackageModule, q: &PackageQuery) -> PackageReply {
    decode_reply(&block_on(m.query(&encode_query(q))).unwrap()).unwrap()
}

/// the adversary's best case: a consensus root that MATCHES the evil bytes.
/// install's hash check passes and only strict decode stands between the
/// bytes and the module.
fn colluding_root(bytes: &[u8]) -> StateRoot {
    let mut h = Sha256::new();
    h.update(bytes);
    StateRoot(h.finalize().into())
}

fn spec(package: &str, harness_module: &str, tags: &[&str], prompts: bool) -> InstallSpec {
    let content = "be terse";
    InstallSpec {
        package: package.into(),
        version: "1.0.0".into(),
        manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
        modules: vec![ModuleBinding {
            logical: "h".into(),
            module_id: harness_module.into(),
        }],
        harness: "h".into(),
        prompts: if prompts {
            vec![PromptSeed {
                logical: "editor".into(),
                path: format!("/packages/{package}/prompts/editor.md"),
                content: content.into(),
                sha256: Sha256::digest(content.as_bytes()).to_vec(),
            }]
        } else {
            Vec::new()
        },
        agents: Vec::new(),
        actions: tags
            .iter()
            .map(|tag| ActionRoute {
                tag: (*tag).into(),
                owner: "h".into(),
            })
            .collect(),
        engagements: Vec::new(),
        uninstall: UninstallPolicy {
            pending_runs: "drain".into(),
            user_data: "preserve".into(),
        },
    }
}

fn install(m: &mut PackageModule, height: u64, origin: Origin, s: InstallSpec) {
    let harness = "h";
    let harness_module = s
        .modules
        .iter()
        .find(|b| b.logical == harness)
        .unwrap()
        .module_id
        .clone();
    let package = s.package.clone();
    exec(m, TestCtx::new(height, origin), &PackageMsg::Install(s));
    exec(
        m,
        TestCtx::new(height, Origin::Module(harness_module)),
        &PackageMsg::MarkActive { package },
    );
}

/// a source holding packages under both installer shapes and three statuses —
/// Active, Suspended, and an unplugged Inactive tombstone — all built through
/// the real execute path, never by poking internals.
fn source() -> PackageModule {
    let alice = Origin::External(b"alice".to_vec());
    let orchestrator = Origin::Module("orchestrator".into());
    let mut m = module();

    install(
        &mut m,
        1,
        alice.clone(),
        spec("aaa.docs", "docs-harness", &["pages.comment.add"], true),
    );
    commit(&mut m);

    install(
        &mut m,
        2,
        orchestrator.clone(),
        spec("bbb.notes", "notes-harness", &["notes.add"], false),
    );
    exec(
        &mut m,
        TestCtx::new(2, orchestrator),
        &PackageMsg::Suspend {
            package: "bbb.notes".into(),
        },
    );
    commit(&mut m);

    install(
        &mut m,
        3,
        alice.clone(),
        spec("ccc.old", "old-harness", &["old.act"], false),
    );
    exec(
        &mut m,
        TestCtx::new(3, alice),
        &PackageMsg::Unplug {
            package: "ccc.old".into(),
        },
    );
    commit(&mut m);
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the source really covers the space: three packages across three
    // statuses, and the tombstone's routes are gone while builtins survive.
    let PackageReply::Packages(list) = query_reply(&src, &PackageQuery::List) else {
        panic!("packages reply expected");
    };
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].status, PackageStatus::Active);
    assert_eq!(list[1].status, PackageStatus::Suspended);
    assert_eq!(list[2].status, PackageStatus::Inactive);

    // the joiner has UNCOMMITTED staged work of its own: install must drop it
    // — a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = module();
    exec(
        &mut dst,
        TestCtx::new(0, Origin::External(b"bob".to_vec())),
        &PackageMsg::Install(spec("zzz.staged", "docs-harness", &["zzz.act"], false)),
    );

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(dst.root(), src_root, "installed root must equal the source");

    // query parity, rows and routes both.
    assert_eq!(
        query_reply(&dst, &PackageQuery::List),
        query_reply(&src, &PackageQuery::List)
    );
    for tag in ["tasks.create", "pages.comment.add", "notes.add", "old.act"] {
        assert_eq!(
            query_reply(&dst, &PackageQuery::ActionOwner { tag: tag.into() }),
            query_reply(&src, &PackageQuery::ActionOwner { tag: tag.into() }),
            "owner parity for {tag}"
        );
    }
    let PackageReply::Package(staged) = query_reply(
        &dst,
        &PackageQuery::Get {
            package: "zzz.staged".into(),
        },
    ) else {
        panic!("package reply expected");
    };
    assert_eq!(staged, None, "install must clear the staged overlay");
}

#[test]
fn tampered_snapshot_is_rejected_and_leaves_state_untouched() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();

    // the target already has COMMITTED state of its own, so "untouched" is
    // observable through both root and query.
    let mut dst = module();
    install(
        &mut dst,
        1,
        Origin::External(b"bob".to_vec()),
        spec("local.pkg", "docs-harness", &["local.act"], false),
    );
    commit(&mut dst);
    let before_root = dst.root();
    let before_view = query_reply(&dst, &PackageQuery::List);

    // flip one byte inside the first package's manifest hash: the bytes still
    // DECODE, but they no longer hash to the agreed root. layout: count 8 |
    // id 8+8 ("aaa.docs") | version 8+5 | hash len 8 -> first hash byte at 45.
    let mut forged = snap.clone();
    forged[45] ^= 0xff;
    assert!(
        dst.install(&forged, src_root).is_err(),
        "a forged payload must be rejected"
    );
    assert_eq!(dst.root(), before_root, "failed install must not move root");
    assert_eq!(query_reply(&dst, &PackageQuery::List), before_view);

    // honest bytes against the WRONG agreed root are equally rejected.
    assert!(dst.install(&snap, StateRoot::ZERO).is_err());
    assert_eq!(dst.root(), before_root);

    // and the failures left the module fully usable: the honest snapshot
    // under the honest root still lands.
    dst.install(&snap, src_root).unwrap();
    assert_eq!(dst.root(), src_root);
}

#[test]
fn truncated_or_padded_snapshot_is_rejected() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();
    let empty_root = module().root();

    // EVERY strict prefix must fail — under the honest root (hash mismatch)
    // AND under a colluding root (strict decode: truncation) — and none of
    // the failures may move the fresh module's root.
    for cut in 0..snap.len() {
        let mut dst = module();
        assert!(
            dst.install(&snap[..cut], src_root).is_err(),
            "a {cut}-byte prefix must fail the root check"
        );
        assert!(
            dst.install(&snap[..cut], colluding_root(&snap[..cut]))
                .is_err(),
            "a {cut}-byte prefix must fail strict decode"
        );
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected prefix ({cut} bytes) must not move the root"
        );
    }

    // trailing bytes after a complete snapshot are equally malformed.
    let mut padded = snap.clone();
    padded.push(0);
    let mut dst = module();
    assert!(dst.install(&padded, colluding_root(&padded)).is_err());
    assert_eq!(dst.root(), empty_root);

    // a count field claiming more entries than committed is caught by the
    // decode walk even when the root colludes: the package count is at
    // offset 0.
    let mut inflated = snap.clone();
    inflated[0] = inflated[0].wrapping_add(1);
    assert!(
        dst.install(&inflated, colluding_root(&inflated)).is_err(),
        "an inflated package count must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

// the minimal one-package state, built through the real op path against a
// module with NO builtin routes, so the byte layout is small enough to pin:
// packages: count 8 | id 8+1 | version 8+5 | hash 8+32 | STATUS 1
//           | bindings count 8 | logical 8+1 | module 8+4 | harness 8+4
//           | INSTALLER DISC 1 + key 8+1 | drain 8+5 | preserve 8+8
//           | times 16
// routes:   count 8
const MINIMAL_LEN: usize = 175;
const STATUS_OFFSET: usize = 70;
const INSTALLER_DISC_OFFSET: usize = 112;
const ROUTES_COUNT_OFFSET: usize = 167;

fn bare_module() -> PackageModule {
    PackageModule::new("package", "memory", Vec::new())
}

fn minimal_snapshot() -> Vec<u8> {
    let mut m = bare_module();
    exec(
        &mut m,
        TestCtx::new(0, Origin::External(vec![5])),
        &PackageMsg::Install(spec("a", "hmod", &[], false)),
    );
    commit(&mut m);
    let snap = m.snapshot();
    assert_eq!(
        snap.len(),
        MINIMAL_LEN,
        "the minimal layout this test indexes into"
    );
    assert_eq!(snap[STATUS_OFFSET], 0, "Installing at the pinned offset");
    assert_eq!(
        snap[INSTALLER_DISC_OFFSET], 0,
        "External at the pinned offset"
    );
    snap
}

#[test]
fn unknown_discriminants_and_unreachable_states_are_rejected() {
    let empty_root = bare_module().root();
    let snap = minimal_snapshot();

    // each discriminant admits exactly the execute-reachable values — a state
    // has ONE valid encoding, even under a colluding root: an unknown status
    // (9), the never-committed Unplugging (3), an unknown installer origin
    // (9), and the install-rejected System installer (2).
    for (index, value, what) in [
        (STATUS_OFFSET, 9u8, "unknown status"),
        (STATUS_OFFSET, 3, "Unplugging status"),
        (INSTALLER_DISC_OFFSET, 9, "unknown installer origin"),
        (INSTALLER_DISC_OFFSET, 2, "system installer origin"),
    ] {
        let mut bad = snap.clone();
        bad[index] = value;
        let mut dst = bare_module();
        let err = dst.install(&bad, colluding_root(&bad)).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected {what} must not move the root"
        );
    }
}

/// a length-prefixed string, exactly as the module's canonical encoding
/// writes one — for splicing hand-built route rows onto a real snapshot.
fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

#[test]
fn routes_must_reference_a_live_package_and_its_own_binding() {
    let empty_root = bare_module().root();
    let snap = minimal_snapshot();

    // a route row: tag | owner | package option | schema option.
    let route = |owner: &str, package: &str| {
        let mut out = Vec::new();
        push_str(&mut out, "t.x");
        push_str(&mut out, owner);
        out.push(1);
        push_str(&mut out, package);
        out.push(0);
        out
    };

    // a route naming a MISSING package, and one whose owner is not among the
    // named package's own bindings — both execute-unreachable.
    for (row, what) in [
        (route("hmod", "zzz"), "missing package"),
        (route("ghost", "a"), "unbound owner"),
    ] {
        let mut bad = snap[..ROUTES_COUNT_OFFSET].to_vec();
        bad.extend_from_slice(&1u64.to_le_bytes());
        bad.extend_from_slice(&row);
        let mut dst = bare_module();
        let err = dst.install(&bad, colluding_root(&bad)).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), empty_root);
    }

    // a route hanging off a TOMBSTONED row: unplug removes routes with the
    // tombstone, so this pair can never have been committed together.
    let mut bad = snap[..ROUTES_COUNT_OFFSET].to_vec();
    bad[STATUS_OFFSET] = 4; // Inactive — valid on its own
    bad.extend_from_slice(&1u64.to_le_bytes());
    bad.extend_from_slice(&route("hmod", "a"));
    let mut dst = bare_module();
    let err = dst.install(&bad, colluding_root(&bad)).unwrap_err();
    assert!(
        matches!(err, Error::Module(_)),
        "a tombstoned package's route must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

#[test]
fn non_ascending_or_duplicate_keys_are_rejected() {
    // two same-shape packages "aaa" and "bbb": their encoded bodies have
    // identical lengths, so swapping the body slices yields a descending-id
    // stream and copying one over the other a duplicate-id stream — both must
    // reject, since sorted-unique keys are what make the encoding canonical.
    let mut m = bare_module();
    for id in ["aaa", "bbb"] {
        exec(
            &mut m,
            TestCtx::new(0, Origin::External(vec![5])),
            &PackageMsg::Install(spec(id, "hmod", &[], false)),
        );
    }
    commit(&mut m);
    let snap = m.snapshot();
    let good_root = m.root();
    // packages section: count 8, then two equal bodies, then routes count 8.
    let body_len = (snap.len() - 16) / 2;
    assert_eq!(snap.len(), 16 + body_len * 2);
    let body_a = snap[8..8 + body_len].to_vec();
    let body_b = snap[8 + body_len..8 + 2 * body_len].to_vec();

    for (first, second, what) in [
        (&body_b, &body_a, "descending ids"),
        (&body_a, &body_a, "duplicate ids"),
    ] {
        let mut bytes = snap.clone();
        bytes[8..8 + body_len].copy_from_slice(first);
        bytes[8 + body_len..8 + 2 * body_len].copy_from_slice(second);
        let mut dst = bare_module();
        let err = dst.install(&bytes, colluding_root(&bytes)).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), bare_module().root());
    }

    // the untouched stream still installs — the rejection above is the
    // ordering check, not an artifact of the splicing.
    let mut dst = bare_module();
    dst.install(&snap, good_root).unwrap();
    assert_eq!(dst.root(), good_root);
}

#[test]
fn snapshot_rejects_a_row_binding_the_registrys_own_id() {
    // "hmodxyz" is 7 bytes, matching "package" (this module's own id in
    // every `TestCtx` here) — the same length lets us splice the STRING
    // CONTENT of an otherwise-legitimate install in place, without touching
    // any length prefix or shifting a single downstream offset.
    let placeholder = "hmodxyz";
    assert_eq!(placeholder.len(), "package".len());

    let mut m = bare_module();
    exec(
        &mut m,
        TestCtx::new(0, Origin::External(vec![5])),
        &PackageMsg::Install(spec("a", placeholder, &[], false)),
    );
    commit(&mut m);
    let snap = m.snapshot();

    // splice EVERY occurrence of the placeholder — the binding's module id
    // and the row's separate `harness` field both name it — to the
    // registry's own id. `validate_spec` refuses a binding naming the
    // registry itself at install time (a HarnessMsg looped back here would
    // be mis-decoded as a PackageMsg and poison the block), so no honest
    // validator ever commits this shape; strict decode must refuse it too.
    let mut bad = snap.clone();
    let needle = placeholder.as_bytes();
    let mut i = 0;
    let mut replaced = 0;
    while i + needle.len() <= bad.len() {
        if &bad[i..i + needle.len()] == needle {
            bad[i..i + needle.len()].copy_from_slice(b"package");
            replaced += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    assert_eq!(
        replaced, 2,
        "expected exactly the binding's module id and the harness field"
    );

    let empty_root = bare_module().root();
    let mut dst = bare_module();
    let err = dst
        .install(&bad, colluding_root(&bad))
        .expect_err("a row binding the registry's own id must be rejected");
    assert!(matches!(err, Error::Module(_)));
    assert_eq!(
        dst.root(),
        empty_root,
        "the rejection must not move the root"
    );

    // the untouched snapshot still installs fine.
    let mut dst = bare_module();
    dst.install(&snap, m.root()).unwrap();
    assert_eq!(dst.root(), m.root());
}

#[test]
fn snapshot_rejects_a_package_less_route_beyond_the_genesis_seed() {
    // a hand-built one-route-row stream: zero packages, one route — the
    // package/schema discriminant bytes here are always 0 (None): a
    // package-less route can only ever be a genesis builtin.
    let route = |tag: &str, owner: &str| {
        let mut out = Vec::new();
        push_str(&mut out, tag);
        push_str(&mut out, owner);
        out.push(0); // package: None
        out.push(0); // schema: None
        out
    };
    let zero_packages_one_route = |route_row: &[u8]| {
        let mut out = Vec::new();
        out.extend_from_slice(&0u64.to_le_bytes()); // packages count
        out.extend_from_slice(&1u64.to_le_bytes()); // routes count
        out.extend_from_slice(route_row);
        out
    };

    // `dst`'s own genesis builtins are exactly {tasks.create, tasks.update_
    // status} -> "tasks" (from `module()`) — nothing else may ever appear as
    // a `package: None` route, whether the tag is foreign or the owner is
    // wrong for a real builtin tag.
    for (bytes, what) in [
        (
            zero_packages_one_route(&route("zzz.ghost", "tasks")),
            "a tag that is not a genesis builtin",
        ),
        (
            zero_packages_one_route(&route("tasks.create", "ghost-own")),
            "a known builtin tag with the wrong owner",
        ),
    ] {
        let empty_root = module().root();
        let mut dst = module();
        let err = dst
            .install(&bytes, colluding_root(&bytes))
            .expect_err(&format!("{what} must be rejected"));
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), empty_root, "{what} must not move the root");
    }

    // the real genesis-only state (no packages, both real builtins, nothing
    // extra) still installs fine under the same decode path.
    let src = module();
    let mut dst = module();
    dst.install(&src.snapshot(), src.root()).unwrap();
    assert_eq!(dst.root(), src.root());
}
