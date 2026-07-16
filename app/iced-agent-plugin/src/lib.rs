//! Agent driver for the native iced shell — a dev-only loopback bridge and
//! semantic-tree layer giving tauri-agent parity (tree/find/click/type/…) plus
//! real OS AccessKit, fed from one source through the vendored `iced_winit`
//! fork's seam API.
//!
//! The crate is inert unless the app calls into it: the app depends on it only
//! under `#[cfg(all(feature = "agent", debug_assertions))]`, so a release
//! binary links it but never boots the server or attaches an adapter.

pub mod bridge;
pub mod collect;
pub mod logs;
pub mod protocol;
pub mod sem;
pub mod tools;

pub use bridge::{AgentHandle, Shared, UiCommand};
pub use collect::{to_accesskit, Collector, FlatNode, SnapshotSlot, WindowSnapshot};
pub use logs::{ring_layer, LogLine, LogsHandle, RingLayer};
pub use protocol::{Cmd, Cond, Intent, Rect, Request, Response, Role, SemNode, Target};
pub use sem::{sem, Sem, SemProbe};
