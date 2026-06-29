//! version control as a content-addressed substrate.
//!
//! two surfaces, split by whether they replicate:
//! - [`cmd`] — local git verbs ([`cmd::Command`]) run via [`cmd::run_local`];
//!   never serialized, never on the wire.
//! - [`op`] — the wire ops ([`op::Op`]) that DO replicate: a [`op::Op::RefUpdate`]
//!   carrying an [`ObjectId`] target, applied on receivers via `update-ref`.
//!
//! everything is content-addressed by [`ObjectId`] (a git oid).

pub mod cmd;
pub mod git;
pub mod object;
pub mod objects;
pub mod odb;
pub mod op;

pub use object::ObjectId;
pub use odb::GitOdb;
