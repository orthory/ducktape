pub use uuid::Uuid as Uid;

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UidError {
    #[error("uid is unassigned (nil) — likely forcibly removed or never set")]
    Unassigned,
}

/// Capability trait — types that carry a stable, system-unique identity expose
/// it here. `try_uid` is the canonical accessor: it returns
/// `Err(UidError::Unassigned)` when the underlying value is the nil uuid (e.g.
/// a section parsed from a legacy on-disk format that didn't carry a uid, or a
/// section whose uid was forcibly cleared).
///
/// Impls typically check `Uid::is_nil()` on a stored field and return
/// `Err(Unassigned)` for nil, `Ok(uid)` otherwise.
pub trait Identify {
    fn try_uid(&self) -> Result<Uid, UidError>;
}

/// Mint a fresh Uid. v7 is timestamp-prefixed + random-suffixed, so ids sort
/// lexicographically by creation time — handy for event-ordered systems.
/// Centralized here so the version choice lives in one place.
pub fn new() -> Uid {
    Uid::now_v7()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_mints_v7() {
        let id = new();
        assert_eq!(id.get_version_num(), 7);
    }

    #[test]
    fn new_is_unique_per_call() {
        let a = new();
        let b = new();
        assert_ne!(a, b);
    }

    #[test]
    fn try_uid_ok_when_assigned() {
        struct Thing { id: Uid }
        impl Identify for Thing {
            fn try_uid(&self) -> Result<Uid, UidError> {
                if self.id.is_nil() { Err(UidError::Unassigned) } else { Ok(self.id) }
            }
        }

        let id = new();
        let t = Thing { id };
        assert_eq!(t.try_uid(), Ok(id));
    }

    #[test]
    fn try_uid_err_when_nil() {
        struct Thing { id: Uid }
        impl Identify for Thing {
            fn try_uid(&self) -> Result<Uid, UidError> {
                if self.id.is_nil() { Err(UidError::Unassigned) } else { Ok(self.id) }
            }
        }

        let t = Thing { id: Uid::default() };
        assert_eq!(t.try_uid(), Err(UidError::Unassigned));
    }
}
