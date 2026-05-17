pub use uuid::Uuid as Uid;

/// Capability trait — types that carry a stable, system-unique identity expose
/// it here. Identity is **always present** for types implementing this trait;
/// uids are minted at construction (or at parse time, by the parser) and never
/// observed in an unassigned state.
pub trait Identify {
    fn uid(&self) -> Uid;
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
    fn uid_returns_stored_value() {
        struct Thing { id: Uid }
        impl Identify for Thing {
            fn uid(&self) -> Uid { self.id }
        }

        let id = new();
        let t = Thing { id };
        assert_eq!(t.uid(), id);
    }
}
