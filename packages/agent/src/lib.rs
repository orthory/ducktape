//! the agentic layer.
//!
//! today this is just the claude-code subprocess [`driver`] — the seam that
//! shells out to the `claude` cli and parses its json envelope. later stages
//! grow a spec-directive parser and an orchestrator on top of it.

pub mod driver;
pub mod spec;
