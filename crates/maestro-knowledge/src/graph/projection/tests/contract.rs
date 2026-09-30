//! Backend-generic projection writer contract; adapters call this unchanged.

use super::projection_writer::BackendContract;
use crate::graph::projection::{
    EdgeFamily, ProjectionError, ProjectionScope, TypedEdgeProjection, content,
    writer::{
        BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionReader,
        ProjectionReadiness, ProjectionWriter,
    },
};
use maestro_kernel::{artifact::Digest, facts::ProjectionReceipt, scope::ScopeSet};
use std::slice;

pub(super) fn run<B: ProjectionBackend>(backend: &mut B, contract: &BackendContract<'_>) {
    let claim_set_id = Digest::of(b"set");
    let initial_receipt = receipt(contract.scope, contract.expected, &claim_set_id);
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
        *contract.expected
    );
    let mut later_edge = contract.edges[0].clone();
    later_edge.id = Digest::of(b"later edge");
    later_edge.source = Digest::of(b"later source");
    writer
        .write_batch(contract.scopes, slice::from_ref(&later_edge), &[])
        .unwrap();
    let after_first_addition = writer.backend.verify_unpublished(contract.scope).unwrap();
    assert_ne!(after_first_addition, *contract.expected);
    later_edge.id = Digest::of(b"latest edge");
    later_edge.source = Digest::of(b"latest source");
    writer
        .write_batch(contract.scopes, slice::from_ref(&later_edge), &[])
        .unwrap();
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
