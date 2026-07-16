//! Agent driver for the native iced shell — a dev-only loopback bridge and
//! semantic-tree layer giving tauri-agent parity (tree/find/click/type/…) plus
//! real OS AccessKit, fed from one source through the vendored `iced_winit`
//! fork's seam API.
//!
//! The entire crate is gated on `debug_assertions`: the fork's `agent` seam
//! module only exists in debug builds, so in release this crate compiles to an
//! empty shell (and the app's own wiring is cfg'd out anyway). No feature
//! combination can put the bridge, the seams, or an adapter in a release
//! binary.

#[cfg(debug_assertions)]
pub mod bridge;
#[cfg(debug_assertions)]
pub mod collect;
#[cfg(debug_assertions)]
pub mod logs;
#[cfg(debug_assertions)]
pub mod protocol;
#[cfg(debug_assertions)]
pub mod sem;
#[cfg(debug_assertions)]
pub mod tools;

#[cfg(debug_assertions)]
pub use bridge::{AgentHandle, Shared, UiCommand};
#[cfg(debug_assertions)]
pub use collect::{Collector, FlatNode, SnapshotSlot, WindowSnapshot, to_accesskit};
#[cfg(debug_assertions)]
pub use logs::{LogLine, LogsHandle, RingLayer, ring_layer};
#[cfg(debug_assertions)]
pub use protocol::{Cmd, Cond, Intent, Rect, Request, Response, Role, SemNode, Target};
#[cfg(debug_assertions)]
pub use sem::{Sem, SemProbe, sem};
