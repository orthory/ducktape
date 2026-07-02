//! finalized snapshot capture: the host can expose the registry roots and
//! per-module state-sync handles for the exact app-hash a finalized block
//! produced. capture must not serve stale heights after the registry advances,
//! and it must not expose staged writes from an aborted block.

use commonware_runtime::{Runner as _, deterministic};
use host::{BlockContext, FinalizedBlock, Host, SnapshotError};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};

const BYTES_ID: &str = "bytes";
const RESOLVER_ID: &str = "resolver";

struct BytesModule {
    committed: u8,
    staged: Option<u8>,
}

impl BytesModule {
    fn new(committed: u8) -> Self {
        Self {
            committed,
            staged: None,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for BytesModule {
    fn id(&self) -> ModuleId {
        BYTES_ID.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot([self.committed; sdk::ROOT_LEN])
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(vec![self.committed]))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let Some((&next, rest)) = msg.payload.split_first() else {
            return Err(Error::Module("missing staged byte".into()));
        };
        self.staged = Some(next);
        if rest == [b'!'] {
            ctx.emit_msg(Msg {
                target: "missing".into(),
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

struct ResolverBackedModule;

#[async_trait::async_trait(?Send)]
impl Module for ResolverBackedModule {
    fn id(&self) -> ModuleId {
        RESOLVER_ID.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot([2u8; sdk::ROOT_LEN])
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "requires a module-specific sync target plus DbResolver".into(),
        })
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Ok(())
    }
}

fn block(height: u64, app_hash: StateRoot) -> FinalizedBlock {
    FinalizedBlock { height, app_hash }
}

fn ctx(height: u64) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height * 10,
        origin: Origin::System,
    }
}

#[test]
fn snapshot_capture_uses_the_finalized_app_hash_boundary() {
    deterministic::Runner::default().start(|_| async move {
        let mut host = Host::genesis(vec![
            Box::new(BytesModule::new(1)),
            Box::new(ResolverBackedModule),
        ])
        .expect("genesis");
        let start_hash = host.app_hash();

        let err = host
            .submit_at(
                ctx(7),
                Msg {
                    target: BYTES_ID.into(),
                    payload: vec![9, b'!'],
                },
            )
            .await
            .expect_err("unknown follow-up aborts the block");
        assert_eq!(err, Error::UnknownModule("missing".into()));

        let after_abort = host
            .capture_finalized_snapshot(block(6, start_hash))
            .expect("unchanged app-hash can still be served");
        let bytes = after_abort.module(BYTES_ID).expect("bytes module");
        assert_eq!(bytes.root, StateRoot([1u8; sdk::ROOT_LEN]));
        assert_eq!(
            bytes.state_sync,
            StateSyncHandle::SnapshotBytes(vec![1]),
            "aborted staged byte must not leak into the snapshot handle",
        );

        let committed = host
            .submit_at(
                ctx(8),
                Msg {
                    target: BYTES_ID.into(),
                    payload: vec![7],
                },
            )
            .await
            .expect("committed block");

        let snapshot = host
            .capture_finalized_snapshot(block(8, committed.app_hash))
            .expect("current finalized app-hash must capture");
        assert_eq!(snapshot.height, 8);
        assert_eq!(snapshot.app_hash, committed.app_hash);

        let bytes = snapshot.module(BYTES_ID).expect("bytes module");
        assert_eq!(bytes.root, StateRoot([7u8; sdk::ROOT_LEN]));
        assert_eq!(bytes.state_sync, StateSyncHandle::SnapshotBytes(vec![7]));

        let resolver = snapshot.module(RESOLVER_ID).expect("resolver module");
        assert_eq!(resolver.root, StateRoot([2u8; sdk::ROOT_LEN]));
        assert_eq!(
            resolver.state_sync,
            StateSyncHandle::ResolverBacked {
                backend: "qmdb".into(),
                detail: "requires a module-specific sync target plus DbResolver".into(),
            },
        );
        assert!(
            !snapshot.has_all_snapshot_bytes(),
            "resolver-backed modules must prevent this from being advertised as a full byte snapshot",
        );

        let moved = host
            .submit_at(
                ctx(9),
                Msg {
                    target: BYTES_ID.into(),
                    payload: vec![8],
                },
            )
            .await
            .expect("second committed block");
        let stale = host
            .capture_finalized_snapshot(block(8, committed.app_hash))
            .expect_err("old boundary must not be served from new registry state");
        assert_eq!(
            stale,
            SnapshotError::AppHashMismatch {
                expected: committed.app_hash,
                actual: moved.app_hash,
            },
        );
    });
}
