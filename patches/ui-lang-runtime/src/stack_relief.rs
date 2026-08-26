//! Stack headroom for outlined component methods.
//!
//! The codegen outlines component uses into per-use methods so rustc's
//! front-end checks one item at a time — but at runtime, unoptimized debug
//! frames of a deep component chain stack up instead of sharing one
//! function's colored frame, and a full view render can exhaust a small
//! thread stack (the ducktape app's render tripled past its 4 MiB budget).
//! Every outlined call goes through [`grow_stack`], which allocates a fresh
//! heap segment when the current stack is nearly spent — the same scheme
//! rustc itself uses for deep recursion. The check is one comparison; the
//! segment only materializes under real pressure.

/// Remaining-stack threshold below which a new segment is allocated.
const RED_ZONE: usize = 512 * 1024;

/// Size of each additional heap-allocated stack segment.
const SEGMENT: usize = 8 * 1024 * 1024;

/// Runs `f`, growing the stack first if less than [`RED_ZONE`] remains.
#[inline]
pub fn grow_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, SEGMENT, f)
}
