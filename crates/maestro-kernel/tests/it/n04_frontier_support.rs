//! Synthetic fixtures for the N04 replaceable frontier contract.
#![cfg(test)]
use maestro_kernel::{
    acquisition::{DispatchRequest, Frontier, LeaseRequest, NewItem, SourceLease},
    artifact::Digest,
    scope::{Right, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Test-owned directory, removed only after its database connections close.
pub(super) struct Scratch(PathBuf);
impl Scratch {
    /// Allocates a synthetic scratch directory.
    pub(super) fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}
impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Fixed authority time; tests advance it without sleeping for durable expiry.
pub(super) fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_000_000)
}
/// One bounded lease request.
pub(super) fn request(at: SystemTime) -> LeaseRequest<'static> {
    LeaseRequest {
        holder: "worker",
        now: at,
        term: Duration::from_secs(30),
    }
}
/// Dispatch with an explicit finite attempt ceiling.
pub(super) fn dispatch(at: SystemTime) -> DispatchRequest<'static> {
    DispatchRequest {
        lease: request(at),
        max_attempts: 3,
    }
}
/// A synthetic immutable request/context pair.
pub(super) fn item() -> NewItem {
    NewItem {
        fetch_identity: "https://example.test/manual".into(),
        authorization_context: Digest::of(b"account-a"),
        representation_profile: Digest::of(b"wire-body"),
    }
}
/// One source writer, through the replaceable frontier port.
pub(super) fn writer(frontier: &dyn Frontier, at: SystemTime) -> SourceLease {
    frontier
        .lease_source(
            "docs",
            &"workspace/default/collection/docs".parse().unwrap(),
            request(at),
        )
        .unwrap()
}
/// A set covering this synthetic collection only.
pub(super) fn scopes(db: &Database) -> ScopeSet {
    db.grant(
        "reader",
        &"workspace/default/collection/docs".parse().unwrap(),
        Right::Read,
        "owner",
    )
    .unwrap();
    db.visible("reader").unwrap()
}
