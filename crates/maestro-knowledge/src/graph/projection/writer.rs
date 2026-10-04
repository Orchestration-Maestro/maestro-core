//! Atomic backend-neutral writes and verification of unpublished projections.
// Default builds retain this backend-neutral contract; the optional native lifecycle
// and the shared fake/native suites use the same writer.
#![allow(
    dead_code,
    reason = "default builds and fake-only reader adapters retain the shared backend contract"
)]

use super::{
    port::{
        EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope,
        TypedEdgeProjection,
    },
    schema::{REQUIRED_INDEXES, SCHEMA_VERSION},
};
use maestro_kernel::facts::ProjectionReceiptIdentity;
use maestro_kernel::{
    artifact::Digest,
    facts::{Object, Predicate, ProjectionReceipt},
    scope::{Scope, ScopeSet, collection_path},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Formatter, Result as FmtResult},
};

/// Durable edge/fact counts and content identity read back after close/reopen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildVerification {
    /// Schema version read back from the unpublished projection.
    pub schema: String,
    /// Expected or independently verified count for each edge family.
    pub family_counts: BTreeMap<EdgeFamily, usize>,
    /// Number of subject fact records, distinct from all edge families.
    pub fact_count: usize,
    /// Digest of canonical application-ID content.
    pub content_digest: Digest,
    /// Expected logical access paths, or paths proved by the reopened native catalog.
    pub indexes: BTreeSet<String>,
}

impl BuildVerification {
    /// Compute the expectation from authoritative knowledge-edge/fact inputs.
    /// This is not native catalog evidence; `producer.verify()` reads that evidence
    /// back from the closed/reopened engine. The frozen encoder stays private.
    ///
    /// # Errors
    /// Refuses invalid records or an unregistered catalog vocabulary before native I/O.
    pub fn expected(
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<Self, ProjectionError> {
        Self::expectation(edges, facts, None)
    }

    /// Compute the same expectation using the catalog owner's registered relation vocabulary.
    /// This is authoritative-input expectation, not evidence of a native catalog or index.
    ///
    /// # Errors
    /// Refuses invalid records and unregistered relations before native I/O.
    pub fn expected_with_catalog_vocabulary(
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
        vocabulary: &dyn CatalogRelationVocabulary,
    ) -> Result<Self, ProjectionError> {
        Self::expectation(edges, facts, Some(vocabulary))
    }

    /// Reuse the writer's validation and one frozen encoder for both entry points.
    fn expectation(
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
        vocabulary: Option<&dyn CatalogRelationVocabulary>,
    ) -> Result<Self, ProjectionError> {
        if let Some(scope) = edges
            .first()
            .map(|edge| &edge.scope)
            .or_else(|| facts.first().map(|fact| &fact.scope))
        {
            validate_batch(scope, edges, facts, vocabulary)?;
        }
        let mut family_counts = BTreeMap::new();
        for edge in edges {
            *family_counts.entry(edge.family).or_default() += 1;
        }
        Ok(Self {
            schema: SCHEMA_VERSION.into(),
            family_counts,
            fact_count: facts.len(),
            content_digest: super::content::digest(edges, facts)
                .map_err(ProjectionError::Invalid)?,
            indexes: REQUIRED_INDEXES
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        })
    }
}

/// Read-only durable rows in one already-open immutable projection.
pub(crate) trait ProjectionBackendReader: Send + Sync + 'static {
    /// Verify reopened immutable content before exposing its rows.
    fn verification(&self) -> Result<BuildVerification, String>;
    /// Return edges adjacent to an entity, without applying authorization or pin filtering.
    fn edges_adjacent(
        &self,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, String>;
    /// Return literal claim records for a subject.
    fn facts_for(&self, subject: &Digest) -> Result<Vec<EntityFact>, String>;
}

/// Replaceable private storage contract. Writes are transactional; published builds are immutable.
pub(crate) trait ProjectionBackend {
    /// The handle for one open immutable projection.
    type Reader: ProjectionBackendReader;
    /// Fail the next batch deterministically for the shared adapter contract suite.
    #[cfg(test)]
    fn inject_batch_failure(&mut self) -> Result<(), String>;
    /// Create a new file for this unpublished scope; never reuse a published file.
    fn create_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String>;
    /// Write edges and facts in one transaction, or leave the file unchanged.
    fn write_batch(
        &mut self,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), String>;
    /// Close, reopen and verify the latest unpublished content; leave the session writable.
    ///
    /// A subsequent batch invalidates this verification, so publication verifies again.
    fn verify_unpublished(&mut self, scope: &ProjectionScope) -> Result<BuildVerification, String>;
    /// Atomically install this verified file under the exact receipt basename.
    fn publish_unpublished(
        &mut self,
        scope: &ProjectionScope,
        file_name: &str,
    ) -> Result<(), String>;
    /// Open only the immutable file named by this exact readiness receipt.
    fn open_published(
        &self,
        scope: &ProjectionScope,
        receipt: &ProjectionReceipt,
    ) -> Result<Self::Reader, String>;
}

/// Catalog-owned closed vocabulary; absence of a port fails closed.
pub trait CatalogRelationVocabulary {
    /// Whether the catalog recognizes this dependency relation spelling.
    fn accepts(&self, relation: &str) -> bool;
}

/// Readiness lookup supplied by the kernel adapter; tests may provide a fake.
pub(crate) trait ProjectionReadiness {
    /// Get the kernel receipt visible for this exact scope.
    fn projection_ready(
        &self,
        scopes: &ScopeSet,
        scope: &ProjectionScope,
    ) -> Result<Option<ProjectionReceipt>, String>;
}

/// Writes one fresh generation without changing existing pins.
pub(crate) struct ProjectionWriter<'a, B: ProjectionBackend> {
    /// Backend receiving the atomic writes.
    pub(crate) backend: &'a mut B,
    /// Exact generation being built.
    scope: ProjectionScope,
}
impl<B: ProjectionBackend> Debug for ProjectionWriter<'_, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ProjectionWriter")
            .field("scope", &self.scope)
            .finish()
    }
}
impl<'a, B: ProjectionBackend> ProjectionWriter<'a, B> {
    /// Start a fresh unpublished build.
    pub(crate) fn create(
        backend: &'a mut B,
        scope: ProjectionScope,
    ) -> Result<Self, ProjectionError> {
        backend
            .create_unpublished(&scope)
            .map_err(ProjectionError::Backend)?;
        Ok(Self { backend, scope })
    }
    /// Borrow the already-created lifecycle session without recreating its native file.
    pub(crate) fn resume(backend: &'a mut B, scope: ProjectionScope) -> Self {
        Self { backend, scope }
    }

    /// Commit one fully validated edge/fact batch atomically.
    pub(crate) fn write_batch(
        &mut self,
        scopes: &ScopeSet,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), ProjectionError> {
        let target = collection_path(&self.scope.collection_id)
            .parse()
            .map_err(|_| ProjectionError::Invalid("invalid collection scope".to_owned()))?;
        if !scopes.covers(&target) {
            return Err(ProjectionError::Unauthorized);
        }
        validate_batch(&self.scope, edges, facts, None)?;
        self.backend
            .write_batch(&self.scope, edges, facts)
            .map_err(ProjectionError::Backend)
    }
    /// Commit a batch using the caller's catalog relation vocabulary.
    pub(crate) fn write_batch_with_catalog_vocabulary(
        &mut self,
        scopes: &ScopeSet,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
        vocabulary: &dyn CatalogRelationVocabulary,
    ) -> Result<(), ProjectionError> {
        let target = collection_path(&self.scope.collection_id)
            .parse()
            .map_err(|_| ProjectionError::Invalid("invalid collection scope".to_owned()))?;
        if !scopes.covers(&target) {
            return Err(ProjectionError::Unauthorized);
        }
        validate_batch(&self.scope, edges, facts, Some(vocabulary))?;
        self.backend
            .write_batch(&self.scope, edges, facts)
            .map_err(ProjectionError::Backend)
    }
    /// Verify the closed/reopened build and then publish it immutably.
    pub(crate) fn verify_and_publish(
        &mut self,
        expected: &BuildVerification,
        claim_set_id: &Digest,
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
        let file_name = super::content::basename(&self.scope, claim_set_id)
            .map_err(ProjectionError::Backend)?;
        self.backend
            .publish_unpublished(&self.scope, &file_name)
            .map_err(ProjectionError::Backend)?;
        Ok(found)
    }
}

/// Opens kernel-authorized immutable projection data and enforces pin/scope/family filters.
pub(crate) struct ProjectionReader {
    /// Exact immutable generation held for reads.
    scope: ProjectionScope,
    /// Opened read-only backend handle.
    backend: Box<dyn ProjectionBackendReader>,
}
impl Debug for ProjectionReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ProjectionReader")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
impl ProjectionReader {
    /// The collection and generation held by this read-only handle.
    #[must_use]
    pub(crate) fn scope(&self) -> &ProjectionScope {
        &self.scope
    }
}
impl TypedEdgeProjection for ProjectionReader {
    fn neighbors(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, ProjectionError> {
        check_read_scope(scopes, &self.scope, pin)?;
        self.backend
            .edges_adjacent(family, entity)
            .map_err(ProjectionError::Backend)
            .map(|rows| {
                rows.into_iter()
                    .filter(|edge| edge.scope == self.scope && edge.family == family)
                    .collect()
            })
    }
    fn entity_facts(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        subject: &Digest,
    ) -> Result<Vec<EntityFact>, ProjectionError> {
        check_read_scope(scopes, &self.scope, pin)?;
        self.backend
            .facts_for(subject)
            .map_err(ProjectionError::Backend)
            .map(|rows| {
                rows.into_iter()
                    .filter(|fact| fact.scope == self.scope && fact.subject == *subject)
                    .collect()
            })
    }
}
impl ProjectionReader {
    /// Wrap the already receipt-verified physical handle selected by a lifecycle.
    pub(super) fn from_verified(
        scope: ProjectionScope,
        backend: Box<dyn ProjectionBackendReader>,
    ) -> Self {
        Self { scope, backend }
    }

    /// Open a published backend build only after the kernel reports matching readiness.
    pub(crate) fn open<B: ProjectionBackend, K: ProjectionReadiness>(
        backend: &B,
        readiness: &K,
        scopes: &ScopeSet,
        scope: ProjectionScope,
    ) -> Result<Self, ProjectionError> {
        check_read_scope(scopes, &scope, &scope)?;
        let receipt = readiness
            .projection_ready(scopes, &scope)
            .map_err(ProjectionError::Backend)?
            .ok_or(ProjectionError::NotReady)?;
        if receipt.identity.collection_id != scope.collection_id
            || receipt.identity.generation_id != scope.generation_id
            || receipt.identity.schema_version != SCHEMA_VERSION
        {
            return Err(ProjectionError::NotReady);
        }
        if super::content::basename(&scope, &receipt.identity.claim_set_id)
            != Ok(receipt.identity.file_name.clone())
        {
            return Err(ProjectionError::NotReady);
        }
        let reader = backend
            .open_published(&scope, &receipt)
            .map_err(ProjectionError::Backend)?;
        let verified = reader.verification().map_err(ProjectionError::Backend)?;
        let mapped = receipt_from_verification(
            (&scope, receipt.identity.build_id),
            receipt.identity.claim_set_id.clone(),
            receipt.identity.file_name.clone(),
            &verified,
            &super::binding::receipt_pins(&receipt),
        )?;
        if mapped != receipt {
            return Err(ProjectionError::NotReady);
        }
        Ok(Self::from_verified(scope, Box::new(reader)))
    }
}

/// Convert verified backend counts into the kernel receipt in one tested mapping.
pub(crate) fn receipt_from_verification(
    target: (&ProjectionScope, i64),
    claim_set_id: Digest,
    file_name: String,
    build: &BuildVerification,
    pins: &[String; 4],
) -> Result<ProjectionReceipt, ProjectionError> {
    let (scope, build_id) = target;
    super::binding::validate(pins).map_err(ProjectionError::Backend)?;
    if build.schema != SCHEMA_VERSION
        || REQUIRED_INDEXES
            .iter()
            .any(|name| !build.indexes.contains(*name))
    {
        return Err(ProjectionError::NotReady);
    }
    let known = build
        .family_counts
        .get(&EdgeFamily::KnowledgeClaim)
        .copied()
        .unwrap_or(0);
    Ok(ProjectionReceipt {
        identity: ProjectionReceiptIdentity {
            build_id,
            collection_id: scope.collection_id.clone(),
            generation_id: scope.generation_id,
            claim_set_id,
            file_name,
            schema_version: build.schema.clone(),
            knowledge_edge_count: known,
            catalog_dependency_edge_count: build
                .family_counts
                .get(&EdgeFamily::CatalogDependency)
                .copied()
                .unwrap_or(0),
            entity_fact_count: build.fact_count,
            content_digest: build.content_digest.clone(),
        },
        resolution_id: Digest::parse(&pins[0])
            .map_err(|error| ProjectionError::Invalid(error.to_string()))?,
        resolver_version: pins[1].clone(),
        settings_identity: Digest::parse(&pins[2])
            .map_err(|error| ProjectionError::Invalid(error.to_string()))?,
        frozen_lock: Digest::parse(&pins[3])
            .map_err(|error| ProjectionError::Invalid(error.to_string()))?,
    })
}

/// Enforce exact generation pins and the caller's collection read scope.
pub(super) fn check_read_scope(
    scopes: &ScopeSet,
    held: &ProjectionScope,
    pin: &ProjectionScope,
) -> Result<(), ProjectionError> {
    if held != pin {
        return Err(ProjectionError::NotReady);
    }
    let target = collection_path(&pin.collection_id)
        .parse()
        .map_err(|_| ProjectionError::Invalid("invalid collection scope".to_owned()))?;
    if !scopes.covers(&target) {
        return Err(ProjectionError::Unauthorized);
    }
    Ok(())
}
/// Validate one batch before the backend begins its atomic transaction.
fn validate_batch(
    scope: &ProjectionScope,
    edges: &[ProjectionEdge],
    facts: &[EntityFact],
    vocabulary: Option<&dyn CatalogRelationVocabulary>,
) -> Result<(), ProjectionError> {
    if scope.generation_id <= 0
        || scope.collection_id.is_empty()
        || collection_path(&scope.collection_id)
            .parse::<Scope>()
            .is_err()
    {
        return Err(ProjectionError::Invalid(
            "invalid projection collection or generation".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for edge in edges {
        if edge.scope != *scope || edge.relation.is_empty() || !ids.insert(edge.id.clone()) {
            return Err(ProjectionError::Invalid(
                "invalid or duplicate edge record".to_owned(),
            ));
        }
        match edge.family {
            EdgeFamily::KnowledgeClaim
                if !Predicate::parse(&edge.relation).is_some_and(|predicate| {
                    predicate.is_claimable() && !predicate.takes_literal()
                }) =>
            {
                return Err(ProjectionError::Invalid(
                    "invalid knowledge-claim relation".to_owned(),
                ));
            }
            EdgeFamily::CatalogDependency
                if !vocabulary.is_some_and(|vocabulary| vocabulary.accepts(&edge.relation)) =>
            {
                return Err(ProjectionError::Invalid(
                    "catalog relation vocabulary is not registered or rejects the relation"
                        .to_owned(),
                ));
            }
            _ => {}
        }
    }
    for fact in facts {
        if !fact.claim.claim.predicate.takes_literal()
            || !matches!(&fact.claim.claim.object, Object::Literal(_))
        {
            return Err(ProjectionError::Invalid(
                "fact is not a literal claim".to_owned(),
            ));
        }
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
