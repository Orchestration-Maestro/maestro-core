//! Public application-ID boundary for disposable typed-edge projections.

use maestro_kernel::{artifact::Digest, facts::ClaimRecord, scope::ScopeSet};
use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
};

/// A pinned collection generation, never an engine-specific identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectionScope {
    /// Collection that owns this projection.
    pub collection_id: String,
    /// Immutable kernel generation pin.
    pub generation_id: i64,
}

/// An authoritative family discriminator. Families are never inferred from edge labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeFamily {
    /// Evidence-backed entity-to-entity knowledge claims.
    KnowledgeClaim,
    /// Catalog dependencies, which are not evidence claims.
    CatalogDependency,
}

/// A directed typed edge identified only by application IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionEdge {
    /// Application ID of the authoritative edge record.
    pub id: Digest,
    /// Collection and generation that scope the edge.
    pub scope: ProjectionScope,
    /// Distinct authoritative edge family.
    pub family: EdgeFamily,
    /// Source entity application ID.
    pub source: Digest,
    /// Target entity application ID.
    pub target: Digest,
    /// Closed-vocabulary relation spelling from the owning family.
    pub relation: String,
}

/// Literal-valued claim data attached to its subject, never a node or edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFact {
    /// Original kernel claim record, including qualifiers and support references.
    pub claim: ClaimRecord,
    /// The subject application ID.
    pub subject: Digest,
    /// The pinned collection and generation.
    pub scope: ProjectionScope,
}

/// Read and write operations exposed to projection consumers and producers.
pub trait TypedEdgeProjection {
    /// Write application-ID edges in the selected family and generation.
    ///
    /// # Errors
    /// Refuses a scope mismatch, invalid family record, or backend failure.
    fn write_edges(
        &mut self,
        scopes: &ScopeSet,
        edges: &[ProjectionEdge],
    ) -> Result<(), ProjectionError>;

    /// Read scoped edges adjacent to `entity` without crossing family or pin.
    ///
    /// # Errors
    /// Refuses an unreadable or unpublished projection.
    fn neighbors(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, ProjectionError>;

    /// Read literal claim records for one subject at the pinned generation.
    ///
    /// # Errors
    /// Refuses an unreadable or unpublished projection.
    fn entity_facts(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        subject: &Digest,
    ) -> Result<Vec<EntityFact>, ProjectionError>;
}

/// A safe refusal from the typed-edge boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// The caller's current scopes do not cover the collection.
    Unauthorized,
    /// The requested build is not published or is inconsistent.
    NotReady,
    /// Input does not match the pinned collection/generation/family.
    Invalid(String),
    /// Projection backend refused an operation.
    Backend(String),
}

impl Display for ProjectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            Self::Unauthorized => "projection access is not authorized",
            Self::NotReady => "projection is not ready",
            Self::Invalid(_) => "projection input is invalid",
            Self::Backend(_) => "projection backend operation failed",
        })
    }
}

impl StdError for ProjectionError {}
