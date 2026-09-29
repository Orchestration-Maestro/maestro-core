//! The commands, a module each, and what they share: the grammar, the kernel
//! opened for the local principal, how a command prints, why it stops short,
//! and a job run in the foreground, its loop and its lease.

mod args;
mod ask;
mod backup;
mod collection;
mod config;
mod eval;
mod filesystem;
mod foreground;
mod graph;
pub(crate) mod health;
mod import;
mod lease;
mod model;
mod output;
mod prepare;
/// Explicit replacement of a lost published projection.
mod publish;
/// Reconciled output for historical publication and current projection state.
mod publish_report;
mod quality;
mod retrieve;
mod run;
mod search;
mod setup;
mod status;
#[cfg(test)]
mod tests;
mod verify;
mod wait;

pub(crate) use run::main;
