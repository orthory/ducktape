//! the root-hash composition contract (moved here with `global_root` when the
//! `state` crate merged into host): order-independent, id-boundary-unambiguous,
//! sensitive to every module root.

use host::global_root;
use sdk::{Module, ModuleId, StateRoot};

/// a stand-in module with a fixed root, so the composition can be tested
/// without standing up a real qmdb/git substrate.
struct StubModule {
    id: &'static str,
    root: StateRoot,
}

#[async_trait::async_trait(?Send)]
impl Module for StubModule {
    fn id(&self) -> ModuleId {
        self.id.to_string()
    }
    fn root(&self) -> StateRoot {
        self.root
    }
    async fn execute(
        &mut self,
        _ctx: &mut dyn sdk::Ctx,
        _msg: &sdk::Msg,
    ) -> Result<(), sdk::Error> {
        Ok(())
    }
}

fn m(id: &'static str, fill: u8) -> StubModule {
    StubModule {
        id,
        root: StateRoot([fill; 32]),
    }
}

#[test]
fn order_independent() {
    let a = m("documents", 1);
    let b = m("forge", 2);
    let c = m("validators", 3);
    let one = global_root(&[&a, &b, &c]);
    let two = global_root(&[&c, &a, &b]);
    assert_eq!(one, two, "global root must not depend on module ordering");
}

#[test]
fn sensitive_to_any_module_root() {
    let a = m("documents", 1);
    let b = m("forge", 2);
    let before = global_root(&[&a, &b]);
    let b2 = m("forge", 9);
    let after = global_root(&[&a, &b2]);
    assert_ne!(
        before, after,
        "changing a module root must change the global root"
    );
}

#[test]
fn id_boundary_is_unambiguous() {
    // ("ab", r) vs ("a", r) must not collide — the length-prefix guards the
    // concatenation boundary between id and the next field.
    let x = m("ab", 7);
    let y = m("a", 7);
    assert_ne!(global_root(&[&x]), global_root(&[&y]));
}

#[test]
fn empty_app_has_a_stable_root() {
    assert_eq!(global_root(&[]), global_root(&[]));
}
