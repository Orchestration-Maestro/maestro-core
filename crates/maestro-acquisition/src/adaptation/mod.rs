//! Scoped local overlays and replaceable activation policy and commit ports.
pub mod artifacts;
pub mod change;
#[cfg(test)]
mod golden;
mod lineage;
mod manifest;
mod recovery;
#[cfg(test)]
mod recovery_tests;
pub mod snapshot;
pub mod storage;
mod writer;
pub use manifest::{
    Activation, ActivationAuthority, AtomicCommit, Change, Commit, ConfigurationWriter,
    CurrentGrants, HeldAuthority, Notify, Proposal, WriteError,
};
pub use writer::{EffectivePreimage, LocalWriter, WriterContext};
