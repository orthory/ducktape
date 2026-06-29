//! the one content-addressed object-store abstraction, generic over the id space.
//!
//! the substrate has two distinct id spaces but ONE storage contract:
//!
//! | impl                  | `Id`                        | holds                       |
//! |-----------------------|-----------------------------|-----------------------------|
//! | `vcs::GitOdb`         | `vcs::ObjectId` (git oid)   | git blobs / trees / commits |
//! | `net::ContentStore`   | sha256 `Digest`             | consensus op-batch payloads |
//!
//! keeping them behind one trait means the blob-transport layer (A3) can be
//! written once against `ObjectStore<Id>` rather than twice. the two id spaces
//! never interoperate — that separation is enforced by the distinct `Id` types,
//! not by this trait.
//!
//! this crate is a zero-dependency leaf on purpose: it must not pull `vcs`,
//! `net`, or any git/crypto crate into its dependents' graphs.

/// a content-addressed store: `put` returns the address *derived from* the
/// bytes, and `get` fetches them back by that address. implementations are
/// content-addressed, so the caller never supplies an id on write.
///
/// all methods take `&self`: a real store mutates through interior state (an
/// `Arc<Mutex<_>>` cache, or git's on-disk ODB), and is typically shared
/// (cloned into several components), so an exclusive `&mut self` would make it
/// unusable. write exclusivity, where it matters, is the implementation's job.
pub trait ObjectStore<Id> {
    /// how this store reports failure. an in-memory store may use
    /// [`std::convert::Infallible`]; a shell-out store uses a real error.
    type Error;

    /// store `bytes` and return the content address derived from them. storing
    /// the same bytes twice yields the same id and is idempotent.
    fn put(&self, bytes: Vec<u8>) -> Result<Id, Self::Error>;

    /// fetch the bytes for `id`.
    ///
    /// `Ok(None)` means "not present" — a normal answer, e.g. a finalized ref
    /// whose objects haven't been fetched yet. `Err` means the store itself
    /// faulted. the two are kept distinct so a caller can fetch-on-absent
    /// without masking a real fault as a cache miss.
    fn get(&self, id: &Id) -> Result<Option<Vec<u8>>, Self::Error>;

    /// whether `id` is present. defaults to a `get`; impls that can answer more
    /// cheaply (e.g. `git cat-file -e`) should override.
    fn has(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.get(id)?.is_some())
    }
}
