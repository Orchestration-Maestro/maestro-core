//! Backend-generic projection writer contract; adapters call this unchanged.

use crate::graph::projection::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
    content,
    writer::{
        BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionReader,
        ProjectionReadiness, ProjectionWriter,
    },
};
use maestro_kernel::{
    artifact::Digest,
    facts::{Predicate, ProjectionReceipt},
    scope::ScopeSet,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

pub(super) struct BackendContract<'a> {
    pub(super) scopes: &'a ScopeSet,
    pub(super) denied_scopes: &'a ScopeSet,
    pub(super) scope: &'a ProjectionScope,
    pub(super) edges: &'a [ProjectionEdge],
    pub(super) facts: &'a [EntityFact],
    pub(super) expected: BuildVerification,
}

pub(super) fn fixture<'a>(
    scopes: &'a ScopeSet,
    denied_scopes: &'a ScopeSet,
    scope: &'a ProjectionScope,
    edges: &'a [ProjectionEdge],
    facts: &'a [EntityFact],
) -> BackendContract<'a> {
    let mut family_counts = BTreeMap::new();
    for edge in edges {
        *family_counts.entry(edge.family).or_insert(0) += 1;
    }
    BackendContract {
        scopes,
        denied_scopes,
        scope,
        edges,
        facts,
        expected: BuildVerification {
            schema: "maestro-typed-edges/1".to_owned(),
            family_counts,
            fact_count: facts.len(),
            content_digest: content::digest(edges, facts).unwrap(),
            indexes: BTreeSet::from([
                "edge_by_scope_family_source".to_owned(),
                "fact_by_scope_subject".to_owned(),
            ]),
        },
    }
}

pub(super) fn edge(generation_id: i64, family: EdgeFamily) -> ProjectionEdge {
    ProjectionEdge {
        id: Digest::of(format!("edge-{generation_id}-{family:?}").as_bytes()),
        scope: ProjectionScope {
            collection_id: "c".to_owned(),
            generation_id,
        },
        family,
        source: Digest::of(b"source"),
        target: Digest::of(b"target"),
        relation: Predicate::Requires.as_str().to_owned(),
    }
}

pub(super) fn fact(scope: &ProjectionScope) -> EntityFact {
    use maestro_kernel::facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        Provenance, ReviewState, Validity,
    };
    use std::collections::BTreeMap;
    EntityFact {
        claim: ClaimRecord {
            id: Digest::of(b"claim"),
            collection_id: scope.collection_id.clone(),
            claim: Claim {
                subject: EntityName {
                    kind: EntityKind::Parameter,
                    name: "mode".to_owned(),
                },
                predicate: Predicate::DefaultsTo,
                object: Object::Literal(Literal {
                    kind: LiteralKind::Text,
                    lexeme: "fast".to_owned(),
                }),
                conditions: BTreeMap::new(),
                version: Validity::Unknown,
                world: Validity::Unknown,
                provenance: Provenance {
                    extractor: "test".to_owned(),
                    profile: Digest::of(b"profile"),
                },
                supports: Vec::new(),
            },
            review: ReviewState::Unreviewed,
            recorded_at: "2026-01-01T00:00:00Z".to_owned(),
        },
        subject: Digest::of(b"entity"),
        scope: scope.clone(),
    }
}

pub(super) fn run<B: ProjectionBackend>(backend: &mut B, contract: &BackendContract<'_>) {
    let claim_set_id = Digest::of(b"set");
    let initial_receipt = receipt(contract.scope, &contract.expected, &claim_set_id);
    assert_unpublished(backend, contract, &initial_receipt);
    let mut writer = ProjectionWriter::create(backend, contract.scope.clone()).unwrap();
    assert_invalid_batches(&mut writer, contract);
    assert_batch_rollback(&mut writer, contract);
    let published_build =
        verify_and_publish(&mut writer, contract, &claim_set_id, &initial_receipt);
    drop(writer);
    let receipt = receipt(contract.scope, &published_build, &claim_set_id);
    assert_wrong_name(backend, contract, &receipt, &claim_set_id);
    assert_read_contract(backend, contract, &receipt);
}

fn assert_unpublished<B: ProjectionBackend>(
    backend: &B,
    contract: &BackendContract<'_>,
    receipt: &ProjectionReceipt,
) {
    assert!(
        ProjectionReader::open(
            backend,
            &Ready(receipt.clone()),
            contract.scopes,
            contract.scope.clone(),
        )
        .is_err()
    );
}

fn assert_invalid_batches<B: ProjectionBackend>(
    writer: &mut ProjectionWriter<'_, B>,
    contract: &BackendContract<'_>,
) {
    assert!(
        writer
            .write_batch(contract.scopes, contract.edges, contract.facts)
            .is_err(),
        "catalog rows require the caller's catalog vocabulary"
    );
    assert!(
        writer
            .write_batch_with_catalog_vocabulary(
                contract.denied_scopes,
                contract.edges,
                contract.facts,
                &ContractVocabulary,
            )
            .is_err(),
        "a denied collection cannot start a backend batch"
    );
    if let Some(edge) = contract.edges.first() {
        assert!(
            writer
                .write_batch_with_catalog_vocabulary(
                    contract.scopes,
                    &[edge.clone(), edge.clone()],
                    &[],
                    &ContractVocabulary,
                )
                .is_err(),
            "duplicate edge IDs are rejected before the backend write"
        );
    }
    if let Some(fact) = contract.facts.first() {
        assert!(
            writer
                .write_batch_with_catalog_vocabulary(
                    contract.scopes,
                    &[],
                    &[fact.clone(), fact.clone()],
                    &ContractVocabulary,
                )
                .is_err(),
            "duplicate fact IDs are rejected before the backend write"
        );
    }
}

fn assert_batch_rollback<B: ProjectionBackend>(
    writer: &mut ProjectionWriter<'_, B>,
    contract: &BackendContract<'_>,
) {
    let before = writer.backend.verify_unpublished(contract.scope).unwrap();
    writer.backend.inject_batch_failure().unwrap();
    assert!(
        writer
            .write_batch_with_catalog_vocabulary(
                contract.scopes,
                contract.edges,
                contract.facts,
                &ContractVocabulary,
            )
            .is_err()
    );
    assert_eq!(
        writer.backend.verify_unpublished(contract.scope).unwrap(),
        before
    );
}

fn verify_and_publish<B: ProjectionBackend>(
    writer: &mut ProjectionWriter<'_, B>,
    contract: &BackendContract<'_>,
    claim_set_id: &Digest,
    receipt: &ProjectionReceipt,
) -> BuildVerification {
    writer
        .write_batch_with_catalog_vocabulary(
            contract.scopes,
            contract.edges,
            contract.facts,
            &ContractVocabulary,
        )
        .unwrap();
    assert!(
        writer
            .backend
            .publish_unpublished(contract.scope, "g.x.lbdb")
            .is_err()
    );
    assert!(
        writer
            .backend
            .open_published(contract.scope, receipt)
            .is_err()
    );
    assert!(
        writer
            .write_batch_with_catalog_vocabulary(
                contract.scopes,
                contract.edges,
                contract.facts,
                &ContractVocabulary,
            )
            .is_err(),
        "duplicate application IDs across batches are rejected"
    );
    assert_eq!(
        writer.backend.verify_unpublished(contract.scope).unwrap(),
        contract.expected
    );
    let mut final_edges = contract.edges.to_vec();
    let mut later_edge = contract.edges[0].clone();
    later_edge.id = Digest::of(b"later edge");
    later_edge.source = Digest::of(b"later source");
    writer
        .write_batch(contract.scopes, slice::from_ref(&later_edge), &[])
        .unwrap();
    final_edges.push(later_edge.clone());
    let after_first_addition = writer.backend.verify_unpublished(contract.scope).unwrap();
    assert_ne!(after_first_addition, contract.expected);
    later_edge.id = Digest::of(b"latest edge");
    later_edge.source = Digest::of(b"latest source");
    writer
        .write_batch(contract.scopes, slice::from_ref(&later_edge), &[])
        .unwrap();
    final_edges.push(later_edge.clone());
    assert_eq!(
        writer.verify_and_publish(&after_first_addition, claim_set_id),
        Err(ProjectionError::NotReady),
        "publication re-verifies content written after the previous verification"
    );
    assert!(
        writer
            .backend
            .open_published(contract.scope, receipt)
            .is_err()
    );
    let latest = writer.backend.verify_unpublished(contract.scope).unwrap();
    assert_eq!(
        writer.verify_and_publish(&latest, claim_set_id).unwrap(),
        latest
    );
    assert_eq!(
        latest.content_digest,
        content::digest(&final_edges, contract.facts).unwrap(),
        "published digest includes every row written"
    );
    assert!(
        writer
            .backend
            .publish_unpublished(contract.scope, &receipt.file_name)
            .is_err(),
        "a published build cannot be replaced under its canonical name"
    );
    latest
}

fn assert_wrong_name<B: ProjectionBackend>(
    backend: &B,
    contract: &BackendContract<'_>,
    receipt: &ProjectionReceipt,
    claim_set_id: &Digest,
) {
    let mut wrong_receipt = receipt.clone();
    wrong_receipt.file_name = content::basename(
        &ProjectionScope {
            generation_id: contract.scope.generation_id + 1,
            ..contract.scope.clone()
        },
        claim_set_id,
    )
    .unwrap();
    assert!(
        backend
            .open_published(contract.scope, &wrong_receipt)
            .is_err()
    );
    assert!(
        ProjectionReader::open(
            backend,
            &Ready(wrong_receipt),
            contract.scopes,
            contract.scope.clone(),
        )
        .is_err(),
        "a receipt for a different basename cannot open this build"
    );
}

fn assert_read_contract<B: ProjectionBackend>(
    backend: &mut B,
    contract: &BackendContract<'_>,
    receipt: &ProjectionReceipt,
) {
    let mut wrong_claim_set = receipt.clone();
    wrong_claim_set.claim_set_id = Digest::of(b"other set");
    assert_eq!(
        ProjectionReader::open(
            backend,
            &Ready(wrong_claim_set),
            contract.scopes,
            contract.scope.clone(),
        )
        .err(),
        Some(ProjectionError::NotReady),
        "the receipt claim set must bind to the same published basename"
    );
    let reader = ProjectionReader::open(
        backend,
        &Ready(receipt.clone()),
        contract.scopes,
        contract.scope.clone(),
    )
    .unwrap();
    assert_eq!(reader.scope(), contract.scope);
    for fact in contract.facts {
        assert!(
            reader
                .entity_facts(contract.denied_scopes, contract.scope, &fact.subject)
                .is_err()
        );
        assert_eq!(
            reader
                .entity_facts(contract.scopes, contract.scope, &fact.subject)
                .unwrap(),
            contract.facts
        );
    }
    for edge in contract.edges {
        assert_eq!(
            reader
                .neighbors(contract.scopes, contract.scope, edge.family, &edge.source)
                .unwrap(),
            vec![edge.clone()]
        );
        assert!(
            reader
                .neighbors(
                    contract.denied_scopes,
                    contract.scope,
                    edge.family,
                    &edge.source
                )
                .is_err()
        );
        let other_pin = ProjectionScope {
            generation_id: contract.scope.generation_id + 1,
            ..contract.scope.clone()
        };
        assert!(
            reader
                .neighbors(contract.scopes, &other_pin, edge.family, &edge.source)
                .is_err()
        );
    }
    assert!(backend.create_unpublished(contract.scope).is_err());
    assert!(
        backend
            .write_batch(contract.scope, contract.edges, contract.facts)
            .is_err()
    );
    assert!(
        backend
            .publish_unpublished(contract.scope, "not-a-receipt-name")
            .is_err()
    );
}

fn receipt(
    scope: &ProjectionScope,
    build: &BuildVerification,
    claim_set_id: &Digest,
) -> ProjectionReceipt {
    ProjectionReceipt {
        collection_id: scope.collection_id.clone(),
        generation_id: scope.generation_id,
        claim_set_id: claim_set_id.clone(),
        file_name: content::basename(scope, claim_set_id).unwrap(),
        schema_version: build.schema.clone(),
        knowledge_edge_count: build
            .family_counts
            .get(&EdgeFamily::KnowledgeClaim)
            .copied()
            .unwrap_or(0),
        catalog_dependency_edge_count: build
            .family_counts
            .get(&EdgeFamily::CatalogDependency)
            .copied()
            .unwrap_or(0),
        entity_fact_count: build.fact_count,
        content_digest: build.content_digest.clone(),
    }
}

struct ContractVocabulary;
impl CatalogRelationVocabulary for ContractVocabulary {
    fn accepts(&self, relation: &str) -> bool {
        relation == "depends_on"
    }
}

struct Ready(ProjectionReceipt);
impl ProjectionReadiness for Ready {
    fn projection_ready(
        &self,
        _scopes: &ScopeSet,
        scope: &ProjectionScope,
    ) -> Result<Option<ProjectionReceipt>, String> {
        Ok((self.0.collection_id == scope.collection_id
            && self.0.generation_id == scope.generation_id)
            .then(|| self.0.clone()))
    }
}
