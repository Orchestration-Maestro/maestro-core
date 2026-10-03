//! Public factory for lease-bound producers and immutable, guarded readers.

use super::{
    adapter::{Session, reader_open},
    build::ProjectionBuild,
    cancellation::ProjectionCancellation,
    handle::ProjectionHandle,
    port::{ProjectionError, ProjectionScope},
    settings::{EngineSettings, ProjectionConfiguration, ProjectionEngine},
};
use maestro_canonicalization::FileLock;
use maestro_kernel::{scope::ScopeSet, store::Database};
use std::{fmt, path::Path, time::SystemTime};

/// Application-facing backend factory; callers never construct native databases.
/// Direct engine-file access bypassing the permanent guard protocol is unsupported.
pub struct ProjectionFactory<'a> {
    /// Explicit immutable root/settings/lock inputs shared by reader and producer adapters.
    configuration: ProjectionConfiguration<'a>,
    /// Runtime backend choice.
    engine: ProjectionEngine,
}

impl fmt::Debug for ProjectionFactory<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionFactory")
            .field("configuration", &self.configuration)
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

impl<'a> ProjectionFactory<'a> {
    /// Retain application configuration; do no filesystem or kernel work.
    #[must_use]
    pub fn new(
        path: &Path,
        engine: ProjectionEngine,
        settings: EngineSettings,
        locks: &'a dyn FileLock,
    ) -> Self {
        Self {
            configuration: ProjectionConfiguration {
                path: path.to_owned(),
                settings,
                locks,
            },
            engine,
        }
    }

    /// Reserve a private staging session under shared access and an exclusive writer guard.
    /// `clock` is the caller's live clock, consulted before installation and after it.
    ///
    /// # Errors
    /// Refuses disabled/absent engines, missing/unsafe guards, contention, unsupported
    /// locks, denied scope, invalid project leases or native write failures.
    pub fn producer<'k>(
        &self,
        kernel: &'k Database,
        scopes: &ScopeSet,
        build: ProjectionBuild,
        clock: &'k dyn Fn() -> SystemTime,
    ) -> Result<ProjectionProducer<'k>, ProjectionError> {
        self.enabled()?;
        Session::create(&self.configuration, kernel, scopes, build, clock)
            .map(|session| ProjectionProducer { session })
    }

    /// Take shared access before looking up the exact kernel readiness receipt.
    ///
    /// # Errors
    /// Refuses disabled/absent engines, missing/unsafe guards, unsupported locks,
    /// denied scope, absent readiness, settings conflicts or mismatching physical content.
    pub fn reader(
        &self,
        kernel: &Database,
        scopes: &ScopeSet,
        scope: ProjectionScope,
    ) -> Result<ProjectionHandle, ProjectionError> {
        self.enabled()?;
        reader_open(&self.configuration, kernel, scopes, scope, None)
    }

    /// Open a reader whose queries can be interrupted by this explicit token.
    ///
    /// # Errors
    /// Returns the ordinary reader refusals, or cancellation without exposing rows.
    pub fn reader_cancellable(
        &self,
        kernel: &Database,
        scopes: &ScopeSet,
        scope: ProjectionScope,
        cancellation: ProjectionCancellation,
    ) -> Result<ProjectionHandle, ProjectionError> {
        self.enabled()?;
        reader_open(
            &self.configuration,
            kernel,
            scopes,
            scope,
            Some(cancellation),
        )
    }

    /// Runtime disablement precedes every delegated side effect in both feature modes.
    fn enabled(&self) -> Result<(), ProjectionError> {
        if self.engine == ProjectionEngine::None {
            return Err(ProjectionError::Backend(
                "the graph is off (graph.engine = none)".into(),
            ));
        }
        Ok(())
    }
}

/// A fresh lease-bound unpublished session. Drop closes it and preserves staging;
/// explicit cancellation additionally cancels its matching kernel job.
#[derive(Debug)]
pub struct ProjectionProducer<'a> {
    /// Feature-selected opaque session; no native type appears in a public signature.
    pub(super) session: Session<'a>,
}
