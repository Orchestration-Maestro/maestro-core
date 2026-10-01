//! Scoped local overlays and replaceable activation policy and commit ports.
#[cfg(test)]
mod golden;
mod lineage;
mod manifest;
mod recovery;
#[cfg(test)]
mod recovery_tests;
mod storage;
mod writer;
pub use manifest::{
    ActivationAuthority, AtomicCommit, Change, Commit, ConfigurationWriter, CurrentGrants,
    HeldAuthority, Notify, Proposal, WriteError,
};
pub use writer::{LocalWriter, WriterContext};
