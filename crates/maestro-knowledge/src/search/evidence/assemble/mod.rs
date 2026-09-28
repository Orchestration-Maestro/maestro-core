//! Runs evidence assembly under the handoff's absolute deadline.

mod candidates;
pub(super) mod deadline;
mod engine;
pub(super) mod ledger;
mod output;
mod types;
mod validate;

#[cfg(test)]
pub(super) use engine::{assemble_blocking, assemble_blocking_sequential};
