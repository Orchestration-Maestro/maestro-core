//! The commands, a module each, and what they share: the grammar, the kernel
//! opened for the local principal, how a command prints, why it stops short,
//! and the lease of a job run in the foreground.

mod args;
mod collection;
mod failure;
mod health;
mod import;
mod kernel;
mod lease;
mod output;
mod run;
mod setup;
mod status;
#[cfg(test)]
mod tests;
mod wait;

pub(crate) use run::main;
