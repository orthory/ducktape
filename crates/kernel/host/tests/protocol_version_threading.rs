//! Phase 2 of the no-downtime node-upgrade plan: `protocol_version` threaded
//! read-only through `BlockContext` -> `Env`.
//!
//! The load-bearing property under test is the NEVER-HASHED invariant: the
//! block's `protocol_version` is a pure dispatch input, copied verbatim into
//! every `Env` (root op AND every FIFO follow-up), branchable inside
//! `execute`/`query`, but folded into NO `root()` preimage and NO app-hash. Two
//! blocks that differ ONLY in `protocol_version` must produce byte-identical
//! module roots and app-hashes. Also covers the `BASELINE_VERSION` default (so
//! the legacy `submit()` path is unchanged) and `Host::effective_version`'s pure
//! derivation + graceful fallback when the `upgrade` module is absent.

use std::cell::RefCell;
use std::rc::Rc;

use commonware_runtime::{Runner as _, deterministic};
use host::{BASELINE_VERSION, BlockContext, Host, UPGRADE_MODULE_ID};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use upgrade_interface::{Upgrade, UpgradeReply, UpgradeStatus, encode_reply};

/// a probe module that records the `protocol_version` it observed on EVERY
/// dispatch, and whose `root()` deliberately IGNORES the version — it commits
/// only a monotonically increasing counter. this is the never-hashed harness:
/// the version reaches `execute` but never the root preimage.
struct ProbeModule {
    id: ModuleId,
    /// emit one follow-up to this sibling when dispatched as the ROOT op.
    sibling: Option<ModuleId>,
    /// shared per-dispatch record of the observed `env.protocol_version`.
    seen: Rc<RefCell<Vec<u32>>>,
    committed: u64,
    staged: Option<u64>,
}

impl ProbeModule {
    fn new(id: &str, sibling: Option<&str>, seen: Rc<RefCell<Vec<u32>>>) -> Self {
        Self {
            id: id.into(),
            sibling: sibling.map(Into::into),
            seen,
            committed: 0,
            staged: None,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for ProbeModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        // ONLY the committed counter — never `protocol_version`.
        StateRoot([self.committed as u8; sdk::ROOT_LEN])
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        self.seen.borrow_mut().push(ctx.env().protocol_version);
        self.staged = Some(self.committed + 1);
        // fan out exactly once, only from the root op (External/System origin) —
        // a follow-up carries `Origin::Module`, so this never loops.
        if !matches!(ctx.env().origin, Origin::Module(_))
            && let Some(sibling) = &self.sibling
        {
            ctx.emit_msg(Msg {
                target: sibling.clone(),
                payload: Vec::new(),
            });
        }
        Ok(())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(next) = self.staged.take() {
            self.committed = next;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        Ok(())
    }
}

/// a stub standing in for the real `upgrade` module: it answers
/// `UpgradeQuery::Status` with a fixed, configurable status so
/// `Host::effective_version` can be exercised on its own derivation +
/// fallback, decoupled from valset/governance mechanics.
struct StatusModule {
    status: UpgradeStatus,
}

#[async_trait::async_trait(?Send)]
impl Module for StatusModule {
    fn id(&self) -> ModuleId {
        UPGRADE_MODULE_ID.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Ok(())
    }

    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(encode_reply(&UpgradeReply::Status(self.status.clone())))
    }
}

fn ctx_v(height: u64, protocol_version: u32) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin: Origin::System,
        protocol_version,
    }
}

/// the drain stamps `BlockContext.protocol_version` into `Env.protocol_version`
/// identically on the root op AND on every emitted follow-up.
#[test]
fn drain_copies_protocol_version_to_root_and_every_followup() {
    deterministic::Runner::default().start(|_| async move {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut host = Host::genesis(vec![
            Box::new(ProbeModule::new("a", Some("b"), seen.clone())),
            Box::new(ProbeModule::new("b", None, seen.clone())),
        ])
        .expect("genesis");

        host.submit_at(
            ctx_v(5, 7),
            Msg {
                target: "a".into(),
                payload: Vec::new(),
            },
        )
        .await
        .expect("block applies");

        // "a" (root op) then "b" (follow-up) — both saw the SAME stamped version.
        assert_eq!(
            *seen.borrow(),
            vec![7, 7],
            "root op and follow-up must both observe BlockContext.protocol_version",
        );
    });
}

/// NEVER-HASHED invariance: two blocks that differ ONLY in `protocol_version`,
/// with identical ops against a probe whose `root()` ignores the version,
/// produce byte-identical module roots AND app-hashes — yet the probe provably
/// observed the two different versions.
#[test]
fn protocol_version_is_never_folded_into_any_root_or_app_hash() {
    deterministic::Runner::default().start(|_| async move {
        async fn run_block(version: u32) -> (StateRoot, StateRoot, Vec<u32>) {
            let seen = Rc::new(RefCell::new(Vec::new()));
            let mut host =
                Host::genesis(vec![Box::new(ProbeModule::new("a", None, seen.clone()))])
                    .expect("genesis");
            host.submit_at(
                ctx_v(9, version),
                Msg {
                    target: "a".into(),
                    payload: Vec::new(),
                },
            )
            .await
            .expect("block applies");
            let root = host.module_root("a").expect("probe root");
            (host.app_hash(), root, seen.borrow().clone())
        }

        let (hash_lo, root_lo, seen_lo) = run_block(1).await;
        let (hash_hi, root_hi, seen_hi) = run_block(999).await;

        assert_eq!(seen_lo, vec![1], "low block dispatched under v1");
        assert_eq!(seen_hi, vec![999], "high block dispatched under v999");
        assert_ne!(seen_lo, seen_hi, "the two blocks really differ in version");

        assert_eq!(
            root_lo, root_hi,
            "module root must not depend on protocol_version",
        );
        assert_eq!(
            hash_lo, hash_hi,
            "app-hash must not depend on protocol_version",
        );
    });
}

/// the default (`BlockContext::default()`) is the baseline version, so the
/// legacy `Host::submit` path dispatches under `BASELINE_VERSION` unchanged.
#[test]
fn default_context_and_legacy_submit_are_baseline() {
    deterministic::Runner::default().start(|_| async move {
        assert_eq!(BlockContext::default().protocol_version, BASELINE_VERSION);

        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut host =
            Host::genesis(vec![Box::new(ProbeModule::new("a", None, seen.clone()))])
                .expect("genesis");
        host.submit(Msg {
            target: "a".into(),
            payload: Vec::new(),
        })
        .await
        .expect("legacy submit applies");

        assert_eq!(
            *seen.borrow(),
            vec![BASELINE_VERSION],
            "legacy submit() dispatches under the baseline version",
        );
    });
}

/// `Host::effective_version` is the pure derivation over committed upgrade-module
/// state: `to_version` at/after an armed pending `H`, `current_version` below `H`
/// (or when not armed).
#[test]
fn effective_version_derivation_over_committed_state() {
    deterministic::Runner::default().start(|_| async move {
        let armed = UpgradeStatus {
            current_version: 1,
            pending: Some(Upgrade {
                name: "forge-multi-repo".into(),
                activation_height: 10,
                to_version: 2,
            }),
            members: vec![vec![1u8; 32]],
            ready: vec![vec![1u8; 32]],
            member_count: 1,
            ready_count: 1,
            armed: true,
        };
        let host = Host::genesis(vec![Box::new(StatusModule {
            status: armed.clone(),
        })])
        .expect("genesis");

        // below H -> stored current_version; at/after H -> to_version.
        assert_eq!(host.effective_version(9).await, 1, "below H runs OLD version");
        assert_eq!(host.effective_version(10).await, 2, "at H runs to_version");
        assert_eq!(host.effective_version(11).await, 2, "after H runs to_version");

        // not armed (a straggler unready): a boundary member missing from ready,
        // so the shared predicate the host runs never flips — even past H. (the
        // `armed` bool is a derived convenience; the host recomputes from
        // members+ready, so this must model genuine incomplete readiness.)
        let not_armed = UpgradeStatus {
            members: vec![vec![1u8; 32], vec![2u8; 32]],
            ready: vec![vec![1u8; 32]],
            member_count: 2,
            ready_count: 1,
            armed: false,
            ..armed
        };
        let host = Host::genesis(vec![Box::new(StatusModule { status: not_armed })])
            .expect("genesis");
        assert_eq!(
            host.effective_version(100).await,
            1,
            "unarmed pending never flips the dispatch version",
        );
    });
}

/// with NO upgrade module registered (pre-retrofit nets), the derivation falls
/// back to `BASELINE_VERSION` rather than erroring or panicking.
#[test]
fn effective_version_falls_back_to_baseline_when_module_absent() {
    deterministic::Runner::default().start(|_| async move {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let host = Host::genesis(vec![Box::new(ProbeModule::new("a", None, seen))])
            .expect("genesis");
        assert_eq!(
            host.effective_version(12345).await,
            BASELINE_VERSION,
            "absent upgrade module must degrade to baseline, never panic",
        );
    });
}
