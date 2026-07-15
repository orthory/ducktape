//! The hash-gated loader for packaged module views.
//!
//! A packaged view is pinned in consensus as a manifest sha256 (gateway
//! `RouteTarget::DuckFs`). The shell polls that pin, asks [`ViewHost::needs`]
//! whether it changed, fetches the bytes through its own (async, fallible)
//! transport, and hands them to [`ViewHost::install`]; this crate owns the
//! swap decision, not the fetching or the rendering. Fetching stays outside
//! on purpose — the real byte path (`blob_fetch`/DuckFS) is async and
//! distinguishes transport failure from corruption, and neither concern
//! belongs in the swap state machine. The contract mirrors
//! `realize_module_swaps` on the module rail: fail closed, and on failure the
//! PREVIOUS artifact keeps running — a tampered or broken publish must never
//! blank a live UI.
//!
//! The runtime is a trait so the swap logic tests without wasmtime; the real
//! wasmtime-backed runtime (widget-tree ABI) is a later, separate concern.

use sha2::{Digest, Sha256};

/// A view's pinned content hash — sha256 of its manifest/component bytes.
pub type ViewHash = [u8; 32];

#[derive(Debug, PartialEq, Eq)]
pub enum ViewError {
    /// Fetched bytes do not hash to the consensus-pinned value.
    Integrity { expected: ViewHash, got: ViewHash },
    /// Bytes are intact but the runtime refused them.
    Instantiate(String),
    /// `render` was called before any view ever loaded successfully.
    NoView,
    /// The loaded view failed while rendering.
    Render(String),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integrity { expected, got } => write!(
                f,
                "view bytes do not match pinned hash (expected {}, got {})",
                hex8(expected),
                hex8(got)
            ),
            Self::Instantiate(e) => write!(f, "view failed to instantiate: {e}"),
            Self::NoView => write!(f, "no view loaded"),
            Self::Render(e) => write!(f, "view failed to render: {e}"),
        }
    }
}

impl std::error::Error for ViewError {}

fn hex8(h: &ViewHash) -> String {
    h[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// Instantiates view bytes into something that can render. The wasmtime
/// runtime implements this later; tests use a fake.
pub trait ViewRuntime {
    type View: LoadedView;
    fn load(&self, bytes: &[u8]) -> Result<Self::View, ViewError>;
}

/// A successfully instantiated view. `state` in, widget tree out — both are
/// opaque bytes to this crate; the widget ABI is the runtime's concern.
pub trait LoadedView {
    fn render(&self, state: &[u8]) -> Result<Vec<u8>, ViewError>;
}

/// One module's view slot: the current hash and the currently-running view.
pub struct ViewHost<R: ViewRuntime> {
    runtime: R,
    current: Option<(ViewHash, R::View)>,
}

impl<R: ViewRuntime> ViewHost<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            runtime,
            current: None,
        }
    }

    /// The hash of the view currently rendering, if any.
    pub fn current_hash(&self) -> Option<&ViewHash> {
        self.current.as_ref().map(|(h, _)| h)
    }

    /// Does the consensus pin differ from what is running? `true` means the
    /// caller should fetch the pinned bytes (its own transport, its own error
    /// domain) and call [`Self::install`]. The thrash guard lives here: an
    /// unchanged pin never triggers a fetch.
    pub fn needs(&self, manifest_hash: &ViewHash) -> bool {
        self.current_hash() != Some(manifest_hash)
    }

    /// Verify `sha256(bytes) == manifest_hash`, instantiate, swap in.
    ///
    /// - Same hash already running → `Ok(())`, no reload (idempotent).
    /// - Hash mismatch → `Err(Integrity)` — the DELIVERED bytes are wrong;
    ///   transport failures never reach here, they stay the caller's.
    /// - Instantiation failure → `Err(Instantiate)`.
    /// - On ANY `Err` the previously loaded view KEEPS RUNNING.
    pub fn install(&mut self, manifest_hash: ViewHash, bytes: &[u8]) -> Result<(), ViewError> {
        if !self.needs(&manifest_hash) {
            return Ok(());
        }
        let got: ViewHash = Sha256::digest(bytes).into();
        if got != manifest_hash {
            return Err(ViewError::Integrity {
                expected: manifest_hash,
                got,
            });
        }
        let view = self.runtime.load(bytes)?;
        self.current = Some((manifest_hash, view));
        Ok(())
    }

    pub fn render(&self, state: &[u8]) -> Result<Vec<u8>, ViewError> {
        match &self.current {
            Some((_, view)) => view.render(state),
            None => Err(ViewError::NoView),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake runtime: bytes starting with `!` refuse to instantiate; otherwise
    /// the "view" echoes its own bytes so tests can see WHICH version renders.
    struct FakeRuntime;

    struct FakeView(Vec<u8>);

    impl ViewRuntime for FakeRuntime {
        type View = FakeView;
        fn load(&self, bytes: &[u8]) -> Result<FakeView, ViewError> {
            if bytes.first() == Some(&b'!') {
                return Err(ViewError::Instantiate("bad component".into()));
            }
            Ok(FakeView(bytes.to_vec()))
        }
    }

    impl LoadedView for FakeView {
        fn render(&self, _state: &[u8]) -> Result<Vec<u8>, ViewError> {
            Ok(self.0.clone())
        }
    }

    fn sha(bytes: &[u8]) -> ViewHash {
        Sha256::digest(bytes).into()
    }

    #[test]
    fn new_hash_installs_and_renders_new_version() {
        let mut host = ViewHost::new(FakeRuntime);
        let (v1, v2) = (b"view-v1".to_vec(), b"view-v2".to_vec());

        assert!(host.needs(&sha(&v1)));
        host.install(sha(&v1), &v1).unwrap();
        assert_eq!(host.render(b"{}").unwrap(), v1);

        assert!(host.needs(&sha(&v2)));
        host.install(sha(&v2), &v2).unwrap();
        assert_eq!(host.render(b"{}").unwrap(), v2);
    }

    #[test]
    fn unchanged_pin_needs_nothing_and_reinstall_is_idempotent() {
        let mut host = ViewHost::new(FakeRuntime);
        let v1 = b"view-v1".to_vec();
        host.install(sha(&v1), &v1).unwrap();

        // the thrash guard: the caller checks `needs` and never fetches...
        assert!(!host.needs(&sha(&v1)));
        // ...and even a redundant install is an idempotent no-op.
        assert_eq!(host.install(sha(&v1), &v1), Ok(()));
        assert_eq!(host.render(b"{}").unwrap(), v1);
    }

    #[test]
    fn tampered_bytes_are_refused_and_old_view_keeps_running() {
        let mut host = ViewHost::new(FakeRuntime);
        let v1 = b"view-v1".to_vec();
        host.install(sha(&v1), &v1).unwrap();

        // Pin says v2, but the delivered bytes are wrong.
        let err = host.install(sha(b"view-v2"), b"evil").unwrap_err();
        assert!(matches!(err, ViewError::Integrity { .. }));
        assert_eq!(host.current_hash(), Some(&sha(&v1)));
        assert_eq!(host.render(b"{}").unwrap(), v1);
    }

    #[test]
    fn broken_component_is_refused_and_old_view_keeps_running() {
        let mut host = ViewHost::new(FakeRuntime);
        let v1 = b"view-v1".to_vec();
        host.install(sha(&v1), &v1).unwrap();

        // Intact bytes (hash matches) that fail instantiation.
        let broken = b"!broken".to_vec();
        let err = host.install(sha(&broken), &broken).unwrap_err();
        assert!(matches!(err, ViewError::Instantiate(_)));
        assert_eq!(host.current_hash(), Some(&sha(&v1)));
        assert_eq!(host.render(b"{}").unwrap(), v1);
    }

    #[test]
    fn render_before_any_load_is_no_view() {
        let host = ViewHost::new(FakeRuntime);
        assert_eq!(host.render(b"{}").unwrap_err(), ViewError::NoView);
    }

    #[test]
    fn failed_first_install_leaves_host_empty_then_recovers() {
        let mut host = ViewHost::new(FakeRuntime);
        let broken = b"!broken".to_vec();
        assert!(host.install(sha(&broken), &broken).is_err());
        assert_eq!(host.render(b"{}").unwrap_err(), ViewError::NoView);

        // A later good publish still installs: the failed hash was not latched.
        let v1 = b"view-v1".to_vec();
        host.install(sha(&v1), &v1).unwrap();
        assert_eq!(host.render(b"{}").unwrap(), v1);
    }
}
