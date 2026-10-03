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

#[test]
fn g10_review_multiple_held_endpoints_choose_canonical_entity() {
    use super::{DescriptorError, DescriptorInput};
    use crate::graph::resolve::resolve_snapshot;

    let fixture = Authority::new();
    let (held, mut input) = record_separation(&fixture);
    input.previous = Some(held.id);
    let mut object = input.decisions[0].clone();
    object.left.endpoint = Endpoint::Object;
    object.right.endpoint = Endpoint::Object;
    input.decisions.push(object);
    let snapshot = fixture
        .database
        .record_resolution(&fixture.scopes, "builder", &input, &validate_snapshot)
        .unwrap();
    let read = DescriptorInput::read(
        &fixture.database,
        &fixture.scopes,
        "builder",
        (&fixture.pin, &snapshot.id),
    )
    .unwrap();
    let entities = resolve_snapshot(&read.snapshot).unwrap();
    let held: Vec<_> = entities
        .iter()
        .filter(|entity| {
            !entity.colliding.is_empty()
                && entity
                    .mentions
                    .iter()
                    .any(|mention| read.claims.iter().any(|record| record.id == mention.claim))
        })
        .map(|entity| entity.id.clone())
        .collect();
    assert_eq!(held.len(), 2);
    assert_eq!(
        held[0].as_str(),
        "9dad7d3c60690eb52f1f1944134592751d29e05cbd2edd8d6169c1155f59a6a6"
    );
    let expected = DescriptorError::HeldForReview(held[0].clone());
    let actual = build(&read).unwrap_err();
    eprintln!("R1 held attached IDs = {held:?}");
    eprintln!("R1 expected = {expected:?}");
    eprintln!("R1 actual = {actual:?}");
    assert_eq!(actual, expected);
}
