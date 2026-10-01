//! Rebuild equality and backend-scoped deterministic lookup fixtures.

use super::{
    Descriptor, DescriptorEmbedder, DescriptorQuery, EmbeddedDescriptors, build, qdrant,
    tests_backend::Backend, tests_embedding::card, tests_source::Authority,
};
use crate::index::ProjectionFilter;
use maestro_kernel::{artifact::Digest, facts::ReviewState, gateway::FakeModels};
use serde_json::json;
use std::time::Duration;

/// Build exclusively from the real kernel and original artifacts, using fake models.
async fn embedded(fixture: &Authority) -> EmbeddedDescriptors {
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let documents = build(&fixture.read(), &fixture.contexts).unwrap();
    embedder
        .prepare(&fixture.pin, &documents, None)
        .await
        .unwrap()
}

/// The deterministic claim-only lookup fixture; no question classifier or ANN oracle.
fn claim_query() -> DescriptorQuery {
    DescriptorQuery {
        kind: "claim".into(),
        vector: vec![1.0; 4],
        limit: 1,
    }
}

#[tokio::test]
async fn deleting_projection_recreates_equal_canonical_payloads_from_kernel_and_artifacts() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    let result = qdrant::rebuild(&backend, &output).await;
    assert!(result.is_ok(), "{result:?}");
    let before = backend.points.lock().unwrap().clone();
    assert_eq!(before.len(), 3);
    backend.delete();
    assert!(backend.points.lock().unwrap().is_empty());
    assert!(qdrant::verify(&backend, &output).await.is_err());
    let rebuilt = embedded(&fixture).await;
    assert_eq!(rebuilt.descriptors(), output.descriptors());
    assert_eq!(rebuilt.receipt(), output.receipt());
    qdrant::rebuild(&backend, &rebuilt).await.unwrap();
    assert_eq!(*backend.points.lock().unwrap(), before);
    let hit = qdrant::lookup(&backend, rebuilt.receipt(), claim_query())
        .await
        .unwrap();
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].payload["kind"], "claim");
    assert_eq!(hit[0].payload["collection_id"], "graph");
    assert_eq!(
        hit[0].payload["text"],
        "Alpha\nCommand\nAlpha is a command.\nREQUIRES\nBeta\nComponent\nBeta is a component."
    );
}

#[tokio::test]
async fn lookup_filters_collection_pin_version_eligibility_and_kind_before_caps() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    let claim = backend
        .points
        .lock()
        .unwrap()
        .values()
        .find(|point| point.payload["kind"] == "claim")
        .unwrap()
        .clone();
    for (index, (field, value)) in [
        ("collection_id", "hidden"),
        ("generation", "999"),
        ("version", "\"2.0\""),
        ("eligibility", "false"),
        ("kind", "entity"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut decoy = claim.clone();
        decoy.id = format!("000-decoy-{index}");
        decoy.payload.insert(field.into(), json!(value));
        backend
            .points
            .lock()
            .unwrap()
            .insert(decoy.id.clone(), decoy);
    }
    let hit = qdrant::lookup(&backend, output.receipt(), claim_query())
        .await
        .unwrap();
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].id, claim.id);
    let filter = backend.filters.lock().unwrap().pop().unwrap();
    let ProjectionFilter::All(fields) = filter else {
        panic!("scope must be a conjunction")
    };
    for (field, value) in [
        ("collection_id", "graph".to_owned()),
        ("generation", fixture.pin.generation_id.to_string()),
        ("version", "null".into()),
        ("eligibility", "true".into()),
        ("kind", "claim".into()),
    ] {
        assert!(fields.contains(&ProjectionFilter::ExactString {
            field: field.into(),
            value
        }));
    }
}

#[tokio::test]
async fn canonical_payload_round_trips_and_disagreeing_eligibility_is_refused() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    let point = backend
        .points
        .lock()
        .unwrap()
        .values()
        .find(|point| point.payload["kind"] == "claim")
        .unwrap()
        .clone();
    let canonical: Result<Descriptor, _> = serde_json::from_value(json!(point.payload));
    assert!(
        canonical.is_ok(),
        "canonical fields must retain their original types"
    );
    let canonical = canonical.unwrap();
    let expected = output
        .descriptors()
        .iter()
        .find(|document| document.id == canonical.id)
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&canonical).unwrap(),
        serde_json::to_vec(expected).unwrap()
    );
    assert_eq!(point.payload["eligible"], json!(true));
    assert_eq!(point.payload["eligibility"], "true");
    backend
        .points
        .lock()
        .unwrap()
        .get_mut(&point.id)
        .unwrap()
        .payload
        .insert("eligible".into(), json!(false));
    assert!(qdrant::verify(&backend, &output).await.is_err());
    assert!(
        qdrant::lookup(&backend, output.receipt(), claim_query())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn review_ineligibility_is_encoded_from_the_canonical_bool_before_caps() {
    let fixture = Authority::new();
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let backend = Backend::default();
    for review in [ReviewState::Rejected, ReviewState::Flagged] {
        let mut input = fixture.read();
        for record in &mut input.claims {
            record.review = review;
        }
        let documents = build(&input, &fixture.contexts).unwrap();
        let output = embedder
            .prepare(&fixture.pin, &documents, None)
            .await
            .unwrap();
        qdrant::rebuild(&backend, &output).await.unwrap();
        for point in backend.points.lock().unwrap().values() {
            assert_eq!(point.payload["eligible"], json!(false));
            assert_eq!(point.payload["eligibility"], "false");
        }
        let result = qdrant::lookup(&backend, output.receipt(), claim_query()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}

#[tokio::test]
async fn lookup_refuses_malformed_canonical_fields_despite_a_matching_keyword() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    let point = backend
        .points
        .lock()
        .unwrap()
        .values()
        .find(|point| point.payload["kind"] == "claim")
        .unwrap()
        .clone();
    for (field, invalid) in [
        ("target", json!(null)),
        ("eligible", json!("true")),
        ("pointers", json!(null)),
    ] {
        let mut malformed = point.clone();
        malformed.payload.insert(field.into(), invalid);
        backend
            .points
            .lock()
            .unwrap()
            .insert(point.id.clone(), malformed);
        let result = qdrant::lookup(&backend, output.receipt(), claim_query()).await;
        assert!(result.is_err(), "malformed {field} was read as canonical");
    }
}

#[tokio::test]
async fn version_a_projection_never_returns_descriptors_for_version_b() {
    let mut fixture = Authority::new();
    fixture.pin.version = Some("1.0".into());
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    let mut other_version = output.receipt().clone();
    other_version.pin.version = Some("2.0".into());
    let result = qdrant::lookup(&backend, &other_version, claim_query()).await;
    assert!(
        result.is_err(),
        "version B must not open version A's receipt-bound collection"
    );
}

#[tokio::test]
async fn invalid_lookup_scope_is_refused_without_backend_calls() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    for (kind, limit) in [("claim", 0), ("invented", 1)] {
        let query = DescriptorQuery {
            kind: kind.into(),
            limit,
            ..claim_query()
        };
        assert!(
            qdrant::lookup(&backend, output.receipt(), query)
                .await
                .is_err()
        );
    }
    assert!(backend.filters.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mismatched_builder_or_content_cannot_create_a_ready_projection() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    for change in 0..2 {
        let mut invalid = output.clone();
        if change == 0 {
            invalid.receipt.profile.builder = Digest::of(b"other builder");
        } else {
            invalid.receipt.content = Digest::of(b"other content");
        }
        let backend = Backend::default();
        assert!(qdrant::rebuild(&backend, &invalid).await.is_err());
    }
}

#[tokio::test]
async fn readiness_refuses_count_payload_layout_and_receipt_mismatches() {
    let fixture = Authority::new();
    let output = embedded(&fixture).await;
    for change in 0..5 {
        let backend = Backend::default();
        qdrant::rebuild(&backend, &output).await.unwrap();
        match change {
            0 => {
                let mut extra = backend
                    .points
                    .lock()
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .clone();
                extra.id = "unexpected-extra-point".into();
                backend
                    .points
                    .lock()
                    .unwrap()
                    .insert(extra.id.clone(), extra);
            }
            1 => {
                backend
                    .points
                    .lock()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()
                    .payload
                    .insert("text".into(), json!("invented"));
            }
            2 => {
                backend
                    .layout
                    .lock()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .1
                    .dense_dimensions = 999;
            }
            3 => {
                backend.points.lock().unwrap().pop_first();
            }
            _ => {
                backend
                    .points
                    .lock()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()
                    .payload
                    .insert("receipt".into(), json!({}));
            }
        }
        assert!(
            qdrant::verify(&backend, &output).await.is_err(),
            "change {change} accepted invalid readiness"
        );
    }
}
