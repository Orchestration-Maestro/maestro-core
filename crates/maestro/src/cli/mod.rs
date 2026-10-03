//! The commands, a module each, and what they share: the grammar, the kernel
//! opened for the local principal, how a command prints, why it stops short,
//! and a job run in the foreground, its loop and its lease.

mod args;
mod ask;
mod backup;
mod catalog;
mod collection;
mod config;
mod eval;
mod foreground;
pub(crate) mod health;
mod import;
mod init;
mod lease;
mod model;
mod output;
mod policy;
mod prepare;
/// Explicit replacement of a lost published projection.
mod publish;
/// Reconciled output for historical publication and current projection state.
mod publish_report;
mod quality;
mod retrieve;
mod run;
mod search;
pub(crate) mod session;
mod setup;
mod status;
#[cfg(test)]
mod tests;
mod trust;
mod trust_path;
mod verify;
mod wait;

pub(crate) use run::main;
