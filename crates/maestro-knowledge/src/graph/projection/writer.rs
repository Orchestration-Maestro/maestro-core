//! Atomic backend-neutral writes and verification of unpublished projections.

use super::{
    port::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope},
    schema::{REQUIRED_INDEXES, SCHEMA_VERSION},
};
use std::fmt::{Debug, Formatter, Result as FmtResult};

use maestro_kernel::{
    artifact::Digest,
    facts::{Object, Predicate},
    scope::{ScopeSet, collection_path},
};
use std::collections::{BTreeMap, BTreeSet};

/// Durable edge/fact counts and content identity read back after close/reopen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildVerification {
    /// Schema version read back from the unpublished projection.
    pub schema: String,
    /// Independently verified count for each edge family.
    pub family_counts: BTreeMap<EdgeFamily, usize>,
    /// Number of subject fact records, distinct from all edge families.
    pub fact_count: usize,
    /// Digest of canonical application-ID content.
    pub content_digest: Digest,
    /// Index names read back from the closed/reopened file.
    pub indexes: BTreeSet<String>,
}

/// Replaceable storage contract. `write_batch` is one native transaction:
/// implementors commit only after all records are bound successfully, and
/// roll back on any error. Published builds are immutable files.
pub trait ProjectionBackend {
    /// Create a new file for this unpublished scope; never reuse a published file.
    ///
    /// # Errors
    /// Returns an adapter-specific creation failure without replacing a file.
    fn create_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String>;

    /// Write edges and facts in one transaction, or leave the file unchanged.
    ///
    /// # Errors
    /// Returns an adapter-specific transaction failure after rolling back all batch rows.
    fn write_batch(
        &mut self,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), String>;

    /// Close, reopen and verify the unpublished file's schema, indexes, counts and digest.
    ///
    /// # Errors
    /// Returns an adapter-specific verification failure; the build stays unpublished.
    fn verify_unpublished(&mut self, scope: &ProjectionScope) -> Result<BuildVerification, String>;

    /// Atomically make this verified file immutable and visible to readers.
    ///
    /// # Errors
    /// Returns an adapter-specific publish failure without replacing an older pin.
    fn publish_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String>;

    /// Open only the immutable published file for this exact collection pin.
    ///
    /// # Errors
    /// Returns an adapter-specific error if no immutable published file exists.
    fn open_published(&self, scope: &ProjectionScope) -> Result<(), String>;
}

/// Writes one fresh generation without changing existing pins.
pub struct ProjectionWriter<'a, B: ProjectionBackend> {
    /// The replaceable backend receiving transactions.
    backend: &'a mut B,
    /// The collection and generation being written.
    scope: ProjectionScope,
}

impl<B: ProjectionBackend> Debug for ProjectionWriter<'_, B> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("ProjectionWriter")
            .field("scope", &self.scope)
            .finish()
    }
}

impl<'a, B: ProjectionBackend> ProjectionWriter<'a, B> {
    /// Start a fresh unpublished build.
    ///
    /// # Errors
    /// Returns a backend error if a new file cannot be created.
    pub fn create(backend: &'a mut B, scope: ProjectionScope) -> Result<Self, ProjectionError> {
        backend
            .create_unpublished(&scope)
            .map_err(ProjectionError::Backend)?;
        Ok(Self { backend, scope })
    }

    /// Commit one fully validated edge/fact batch atomically.
    ///
    /// # Errors
    /// Refuses unauthorized, mixed-scope, duplicate-ID, or backend-rejected batches.
    pub fn write_batch(
        &mut self,
        scopes: &ScopeSet,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), ProjectionError> {
        let collection_scope = collection_path(&self.scope.collection_id)
            .parse()
            .map_err(|_| ProjectionError::Invalid("invalid collection scope".to_owned()))?;
        if !scopes.covers(&collection_scope) {
            return Err(ProjectionError::Unauthorized);
        }
        validate_batch(&self.scope, edges, facts)?;
        self.backend
            .write_batch(&self.scope, edges, facts)
            .map_err(ProjectionError::Backend)
    }

    /// Publish only when the reopened build exactly matches the expected receipt.
    ///
    /// # Errors
    /// Returns `NotReady` when schema, family/fact counts, content digest, or indexes differ.
    pub fn verify_and_publish(
        &mut self,
        expected: &BuildVerification,
    ) -> Result<BuildVerification, ProjectionError> {
        let found = self
            .backend
            .verify_unpublished(&self.scope)
            .map_err(ProjectionError::Backend)?;
        if found != *expected
            || found.schema != SCHEMA_VERSION
            || REQUIRED_INDEXES
                .iter()
                .any(|index| !found.indexes.contains(*index))
        {
            return Err(ProjectionError::NotReady);
        }
        self.backend
            .publish_unpublished(&self.scope)
            .map_err(ProjectionError::Backend)?;
        Ok(found)
    }
}

/// A read-only handle obtained only for a published generation.
#[derive(Debug)]
pub struct ProjectionReader {
    /// The exact immutable generation held for reads.
    scope: ProjectionScope,
}

impl ProjectionReader {
    /// Open the immutable published projection at this exact application pin.
    ///
    /// # Errors
    /// Refuses an unpublished or unavailable projection.
    pub fn open<B: ProjectionBackend>(
        backend: &B,
        scope: ProjectionScope,
    ) -> Result<Self, ProjectionError> {
        backend
            .open_published(&scope)
            .map_err(ProjectionError::Backend)?;
        Ok(Self { scope })
    }

    /// The collection and generation held by this read-only handle.
    #[must_use]
    pub fn scope(&self) -> &ProjectionScope {
        &self.scope
    }
}

/// Check the full input batch before the backend opens a native transaction.
fn validate_batch(
    scope: &ProjectionScope,
    edges: &[ProjectionEdge],
    facts: &[EntityFact],
) -> Result<(), ProjectionError> {
    let mut ids = BTreeSet::new();
    for edge in edges {
        if edge.scope != *scope || edge.relation.is_empty() || !ids.insert(edge.id.clone()) {
            return Err(ProjectionError::Invalid(
                "invalid or duplicate edge record".to_owned(),
            ));
        }
    }
    for fact in facts {
        if fact.claim.claim.predicate != Predicate::DefaultsTo {
            return Err(ProjectionError::Invalid(
                "fact is not a literal default".to_owned(),
            ));
        }
        let Object::Literal(_) = &fact.claim.claim.object else {
            return Err(ProjectionError::Invalid(
                "fact is not a literal default".to_owned(),
            ));
        };
        if fact.scope != *scope
            || fact.claim.collection_id != scope.collection_id
            || !ids.insert(fact.claim.id.clone())
        {
            return Err(ProjectionError::Invalid(
                "invalid or duplicate fact record".to_owned(),
            ));
        }
    }
    Ok(())
}
