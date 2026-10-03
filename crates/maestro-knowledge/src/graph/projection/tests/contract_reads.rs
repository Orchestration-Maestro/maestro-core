//! Ordered application-ID reads and exact scope/family pin contract for every backend.

use super::contract::{BackendContract, Ready};
use crate::graph::projection::{
    ProjectionError, ProjectionScope, TypedEdgeProjection,
    writer::{ProjectionBackend, ProjectionReader},
};
use maestro_kernel::{artifact::Digest, facts::ProjectionReceipt};
#[cfg(not(windows))]
use {
    super::contract,
    crate::graph::projection::{
        EdgeFamily, content,
        writer::{ProjectionWriter, receipt_from_verification},
    },
    maestro_kernel::scope::ScopeSet,
    std::slice,
};

/// A later published generation never changes an already-held physical reader.
#[cfg(not(windows))]
pub(in crate::graph::projection) fn pinned_generations<B: ProjectionBackend>(
    first: &mut B,
    second: &mut B,
    scopes: &ScopeSet,
) {
    let mut readers = Vec::new();
    for (backend, generation) in [(first, 1), (second, 2)] {
        let edge = contract::edge(generation, EdgeFamily::KnowledgeClaim);
        let fact = contract::fact(&edge.scope);
        let fixture = contract::fixture(
            scopes,
            scopes,
            &edge.scope,
            slice::from_ref(&edge),
            slice::from_ref(&fact),
        );
        let set = Digest::of(format!("set-{generation}").as_bytes());
        let name = content::basename(&edge.scope, &set).unwrap();
        let mut writer = ProjectionWriter::create(backend, edge.scope.clone()).unwrap();
        writer
            .write_batch(scopes, slice::from_ref(&edge), slice::from_ref(&fact))
            .unwrap();
        writer.verify_and_publish(&fixture.expected, &set).unwrap();
        let receipt =
            receipt_from_verification(&edge.scope, set, name, &fixture.expected, &contract::pins())
                .unwrap();
        readers.push((
            ProjectionReader::open(backend, &Ready(receipt), scopes, edge.scope.clone()).unwrap(),
            edge,
            fact,
        ));
    }
    for (reader, edge, fact) in readers {
        assert_eq!(
            reader
                .neighbors(scopes, &edge.scope, edge.family, &edge.source)
                .unwrap(),
            slice::from_ref(&edge)
        );
        assert_eq!(
            reader
                .entity_facts(scopes, &edge.scope, &fact.subject)
                .unwrap(),
            [fact]
        );
    }
}

pub(super) fn assert_read_contract<B: ProjectionBackend>(
    backend: &mut B,
    contract: &BackendContract<'_>,
    receipt: &ProjectionReceipt,
) {
    assert_reads(backend, contract, receipt);
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

/// Windows immutable fixtures use exactly these shared reader assertions too.
pub(in crate::graph::projection) fn assert_reads<B: ProjectionBackend>(
    backend: &B,
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
            contract.scope.clone()
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
    let other_pin = ProjectionScope {
        generation_id: contract.scope.generation_id + 1,
        ..contract.scope.clone()
    };
    assert!(
        reader
            .entity_facts(contract.scopes, &other_pin, &Digest::of(b"entity"))
            .is_err()
    );
    assert!(
        reader
            .entity_facts(contract.scopes, contract.scope, &Digest::of(b"absent"))
            .unwrap()
            .is_empty()
    );
    for fact in contract.facts {
        assert!(
            reader
                .entity_facts(contract.denied_scopes, contract.scope, &fact.subject)
                .is_err()
        );
        let mut expected: Vec<_> = contract
            .facts
            .iter()
            .filter(|row| row.subject == fact.subject)
            .cloned()
            .collect();
        expected.sort_by(|left, right| left.claim.id.cmp(&right.claim.id));
        assert_eq!(
            reader
                .entity_facts(contract.scopes, contract.scope, &fact.subject)
                .unwrap(),
            expected
        );
    }
    assert_edges(&reader, contract, &other_pin);
}

fn assert_edges(
    reader: &ProjectionReader,
    contract: &BackendContract<'_>,
    other_pin: &ProjectionScope,
) {
    for edge in contract.edges {
        for entity in [&edge.source, &edge.target] {
            let mut expected: Vec<_> = contract
                .edges
                .iter()
                .filter(|row| {
                    row.family == edge.family && (row.source == *entity || row.target == *entity)
                })
                .cloned()
                .collect();
            expected.sort_by(|left, right| left.id.cmp(&right.id));
            assert_eq!(
                reader
                    .neighbors(contract.scopes, contract.scope, edge.family, entity)
                    .unwrap(),
                expected
            );
        }
        assert!(
            reader
                .neighbors(
                    contract.scopes,
                    contract.scope,
                    edge.family,
                    &Digest::of(b"absent")
                )
                .unwrap()
                .is_empty()
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
        assert!(
            reader
                .neighbors(contract.scopes, other_pin, edge.family, &edge.source)
                .is_err()
        );
    }
}
