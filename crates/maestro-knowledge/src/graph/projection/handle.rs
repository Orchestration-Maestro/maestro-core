//! Backend-neutral immutable handle retaining the backend's native ownership and guard.
use super::{
    port::{
        EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope,
        TypedEdgeProjection,
    },
    settings::EngineSettings,
};
use maestro_kernel::{artifact::Digest, scope::ScopeSet};
use std::fmt;

/// A pinned immutable reader retaining shared access for its complete lifetime.
pub struct ProjectionHandle {
    /// Backend-neutral reader owns its native handles and guard together.
    pub(super) reader: Box<dyn TypedEdgeProjection + Send + Sync>,
    /// Exact settings used to open/share the physical handle.
    pub(super) settings: EngineSettings,
}

impl fmt::Debug for ProjectionHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionHandle")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl ProjectionHandle {
    /// The caller-owned settings and frozen lock identity bound to this read.
    #[must_use]
    pub fn settings(&self) -> &EngineSettings {
        &self.settings
    }
}
impl TypedEdgeProjection for ProjectionHandle {
    fn neighbors(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, ProjectionError> {
        self.reader.neighbors(scopes, pin, family, entity)
    }
    fn entity_facts(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        subject: &Digest,
    ) -> Result<Vec<EntityFact>, ProjectionError> {
        self.reader.entity_facts(scopes, pin, subject)
    }
}
