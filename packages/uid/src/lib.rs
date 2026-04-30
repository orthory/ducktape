pub use uuid::Uuid as Uid;

/// Capability trait — types that carry a stable, system-unique identity expose it
/// here. Impls usually return a cached field rather than minting on every call.
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
    fn identify_trait_returns_stored_uid() {
        struct Thing { id: Uid }
        impl Identify for Thing {
            fn uid(&self) -> Uid { self.id }
        }

        let id = new();
        let t = Thing { id };
        assert_eq!(t.uid(), id);
    }
}
