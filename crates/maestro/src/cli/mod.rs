//! The commands, a module each, and what they share: the grammar, the kernel
//! opened for the local principal, how a command prints, why it stops short,
//! and a job run in the foreground, its loop and its lease.

mod args;
mod collection;
mod failure;
mod foreground;
mod health;
mod import;
mod kernel;
mod lease;
mod output;
mod quality;
mod run;
mod setup;
mod status;
#[cfg(test)]
mod tests;
mod wait;

pub(crate) use run::main;
