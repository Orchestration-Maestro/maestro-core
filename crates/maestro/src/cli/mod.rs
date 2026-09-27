//! The commands, a module each, and what they share: the grammar, the kernel
//! opened for the local principal, how a command prints, why it stops short,
//! and a job run in the foreground, its loop and its lease.

mod args;
mod backup;
mod collection;
mod foreground;
mod health;
mod import;
mod lease;
mod output;
mod prepare;
mod publish;
mod quality;
mod retrieve;
mod run;
mod setup;
mod status;
#[cfg(test)]
mod tests;
mod verify;
mod wait;

pub(crate) use run::main;
