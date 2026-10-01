//! Audited identities stay held for review or cross the projection unchanged.

use super::{
    Descriptor, DescriptorEmbedder, DescriptorQuery, build, qdrant, tests_backend::Backend,
    tests_embedding::card, tests_source::Authority,
};
use crate::graph::resolve::{EXACT_RESOLVER_VERSION, validate_snapshot};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        ClaimSet, Decision, DecisionKind, Endpoint, Mention, ResolutionInput, ResolutionSnapshot,
    },
    gateway::FakeModels,
};
use serde_json::json;
use std::time::Duration;

/// Record a reviewer-identified collision using supported mentions and real authority.
fn record_separation(fixture: &Authority) -> (ResolutionSnapshot, ResolutionInput) {
    let original = fixture.read();
    let mut claim = original.claims[0].claim.clone();
    claim
        .conditions
        .insert("context".into(), "other documentary namespace".into());
    let set = fixture
        .database
        .record_claim_set(
            &fixture.scopes,
            &ClaimSet {
                collection_id: "graph".into(),
                claims: vec![claim],
            },
        )
        .unwrap();
    let decision = Decision {
        left: Mention {
            claim: original.claims[0].id.clone(),
            endpoint: Endpoint::Subject,
        },
        right: Mention {
            claim: set.claims[0].id.clone(),
            endpoint: Endpoint::Subject,
        },
        kind: DecisionKind::Separate,
        reason: "source-backed namespace collision reviewed by a person".into(),
    };
    let input = ResolutionInput {
        resolver_version: EXACT_RESOLVER_VERSION.into(),
        sets: vec![original.claim_set.clone(), set.id],
        previous: Some(fixture.resolution.clone()),
        decisions: vec![decision.clone()],
    };
    let held =
        fixture
            .database
            .record_resolution(&fixture.scopes, "builder", &input, &validate_snapshot);
    assert!(held.is_ok(), "collision review must be retained: {held:?}");
    (held.unwrap(), input)
}

#[tokio::test]
async fn reviewed_namespace_identity_survives_authority_embedding_and_projection_handoff() {
    let fixture = Authority::new();
    let original = fixture.read();
    let (held, mut input) = record_separation(&fixture);
    let read = super::DescriptorInput::read(
        &fixture.database,
        &fixture.scopes,
        "builder",
        (&fixture.pin, &held.id),
    )
    .unwrap();
    assert!(build(&read).is_err());
    input.previous = Some(held.id.clone());
    input.decisions[0].kind = DecisionKind::Alias;
    let resolved = fixture
        .database
        .record_resolution(&fixture.scopes, "builder", &input, &validate_snapshot)
        .unwrap();
    assert_eq!(resolved.history.len(), 2);
    let read = super::DescriptorInput::read(
        &fixture.database,
        &fixture.scopes,
        "builder",
        (&fixture.pin, &resolved.id),
    )
    .unwrap();
    let documents = build(&read).unwrap();
    let alpha = documents
        .iter()
        .find(|document| document.kind == "entity" && document.text.starts_with("Alpha\n"))
        .unwrap();
    assert_eq!(
        alpha.target.as_str(),
        "d5c16926bdd336139829d71d709c9c15d28ffc596fbe1efef5442325370075c7"
    );
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let output = embedder
        .prepare(&fixture.pin, &documents, None)
        .await
        .unwrap();
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    let hits = qdrant::lookup(
        &backend,
        output.receipt(),
        DescriptorQuery {
            kind: "entity".into(),
            vector: vec![1.0; 4],
            limit: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(hits.len(), 2);
    for hit in hits {
        let projected: Descriptor = serde_json::from_value(json!(hit.payload)).unwrap();
        assert_eq!(
            &projected,
            documents
                .iter()
                .find(|document| document.id == projected.id)
                .unwrap()
        );
        assert_eq!(projected.resolution, resolved.id);
    }
    assert_eq!(
        fixture
            .database
            .resolution(&fixture.scopes, "builder", &held.id)
            .unwrap(),
        Some(held)
    );
    assert_eq!(build(&fixture.read()).unwrap(), build(&original).unwrap());
}
