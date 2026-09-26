//! Scopes and grants: who may see what (docs/architecture/04 §3, building
//! block B1; plan D4).
//!
//! A scope is a node of the kernel's access tree, written as a path of kind
//! and name pairs: `workspace/<name>`, then optionally `collection/<name>`,
//! then optionally `source/<name>`. Those are the kinds, in that order, and a
//! name follows the rule of [`check_name`]. Collection and source IDs follow
//! the same rule: the kernel refuses to record a collection or a source whose
//! ID breaks it, so each ID forms one segment of its scope's path. The
//! kernel's records live in the workspace `default`: a collection's scope is
//! `workspace/default/collection/<id>`, a source's `…/source/<id>`; a
//! document and its revisions have their source's, a generation its
//! collection's, and an event the scope it was recorded in, which the journal
//! refuses unless it is a scope path.
//!
//! A grant gives a principal a right on a scope and every scope below it,
//! compared by whole segments: `…/collection/ct` never covers
//! `…/collection/ctm`. There is one right, [`Right::Read`], and a row for
//! each principal, scope and right. Nothing is granted implicitly: a
//! principal with no grant sees nothing, and a scope no grant covers is no
//! access. Each grant and each revocation is journaled, with its actor, in
//! the write that makes it. The CLI and the MCP server run as the local
//! user's principal, [`LOCAL`], whose grants come from [`CONFIG_FILE`] in the
//! kernel's configuration directory.
//!
//! [`Database::visible`](crate::store::Database::visible) reads a
//! principal's grants anew each time, into a [`ScopeSet`] kept for one
//! request and never longer, so a revocation applies to the next read, even
//! of an older generation. Every public reader of scoped data takes a
//! `&ScopeSet` and filters inside its query, and nothing outside the kernel
//! can build a set that covers every scope. The readers of unscoped
//! bookkeeping take none:
//!
//! - [`Database::artifact`](crate::store::Database::artifact),
//!   [`Database::get`](crate::store::Database::get),
//!   [`artifact::Store::get`](crate::artifact::Store::get) and
//!   [`ModelCard::load`](crate::gateway::ModelCard::load): an artifact, or a
//!   model card, by its digest. A digest is a capability: whoever holds one
//!   can read the bytes it names, so a digest is handed out only with a
//!   record its reader may read;
//! - [`Database::garbage`](crate::store::Database::garbage) and
//!   [`Database::collect_garbage`](crate::store::Database::collect_garbage):
//!   the artifacts no record refers to, whatever their scope, which serve the
//!   garbage collector only, never a tool;
//! - [`Database::cursor`](crate::store::Database::cursor): how far a
//!   consumer has read a stream, a position rather than an event;
//! - [`Database::ack`](crate::store::Database::ack): its refusal names a
//!   stream's last sequence, a position rather than an event;
//! - [`Database::visible`](crate::store::Database::visible): a principal's
//!   grants, which are what a set is made of.
//!
//! The migrations have no public reader: the database applies them when it
//! opens.

mod config;
mod grant;
mod path;
mod right;
mod set;
#[cfg(test)]
mod tests;

pub use config::{CONFIG_FILE, Config, ConfigError, LOCAL};
pub use path::{InvalidName, InvalidScope, Scope, check_name};
pub use right::Right;
pub use set::ScopeSet;
