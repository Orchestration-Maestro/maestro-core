//! Named feature-absent implementation, owned only by featureless mutation runs.

use super::{
    build::{ProjectionBuild, PublishedProjection},
    cancellation::ProjectionCancellation,
    handle::ProjectionHandle,
    port::{EntityFact, ProjectionEdge, ProjectionError, ProjectionScope},
    settings::{EngineSettings, ProjectionConfiguration},
    writer::{BuildVerification, CatalogRelationVocabulary},
};
use maestro_kernel::{scope::ScopeSet, store::Database};
use std::{convert::Infallible, marker::PhantomData, time::SystemTime};

/// No feature-absent session can exist: only the named factory refusal is reachable.
#[derive(Debug)]
pub(super) struct Session<'a> {
    /// An unconstructible session needs no fake backend or repeated refusal methods.
    never: Infallible,
    /// Preserve the public kernel lifetime in every feature mode.
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> Session<'a> {
    /// Refuse before filesystem, kernel, lock or native calls.
    pub(super) fn create(
        _factory: &ProjectionConfiguration<'_>,
        _kernel: &'a Database,
        _scopes: &ScopeSet,
        _build: ProjectionBuild,
        _clock: &'a dyn Fn() -> SystemTime,
    ) -> Result<Self, ProjectionError> {
        Err(absent())
    }
    /// Unreachable because this build cannot construct a session.
    pub(super) fn write_batch(
        &mut self,
        _edges: &[ProjectionEdge],
        _facts: &[EntityFact],
        _vocabulary: Option<&dyn CatalogRelationVocabulary>,
    ) -> Result<(), ProjectionError> {
        match self.never {}
    }
    /// Unreachable because this build cannot construct a session.
    pub(super) fn verify(&mut self) -> Result<BuildVerification, ProjectionError> {
        match self.never {}
    }
    /// Unreachable because this build cannot construct a session.
    pub(super) fn publish(
        self,
        _expected: &BuildVerification,
    ) -> Result<PublishedProjection, ProjectionError> {
        match self.never {}
    }
    /// Unreachable because this build cannot construct a session.
    pub(super) fn cancel(self) -> Result<(), ProjectionError> {
        match self.never {}
    }
    /// Unreachable because this build cannot construct a session.
    pub(super) fn settings(&self) -> &EngineSettings {
        match self.never {}
    }
}

/// Refuse before receipt lookup, filesystem, lock or native calls.
pub(super) fn reader_open(
    _factory: &ProjectionConfiguration<'_>,
    _kernel: &Database,
    _scopes: &ScopeSet,
    _scope: ProjectionScope,
    _cancellation: Option<ProjectionCancellation>,
) -> Result<ProjectionHandle, ProjectionError> {
    Err(absent())
}

/// Same named repair action as existing CLI setup/status/doctor paths.
fn absent() -> ProjectionError {
    ProjectionError::Backend(
        "the graph was built without the engine; rebuild with the `engine` feature".into(),
    )
}
