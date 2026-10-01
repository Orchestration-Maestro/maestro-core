//! Scoped local overlays and replaceable activation policy and commit ports.
mod manifest;
mod storage;
mod writer;
pub use manifest::{
    ActivationAuthority, AtomicCommit, Change, Commit, ConfigurationWriter, HeldAuthority, Notify,
    Proposal, WriteError,
};
pub use writer::{LocalWriter, WriterContext};
