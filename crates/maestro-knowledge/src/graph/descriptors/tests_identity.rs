//! Frozen /1 preimages remain typed and independent of the JSON map backend.

use super::{
    BUILDER_VERSION, Descriptor, DescriptorEmbedder, DescriptorReceipt, build, qdrant,
    tests::fixture, tests_backend::Backend, tests_embedding::card,
};
use maestro_kernel::{artifact::Digest, facts::Validity, gateway::FakeModels};
use std::time::Duration;

/// Original default-build receipt, including its sorted nested profile.
const RECEIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/synthetic/graph/descriptor-receipt-preimage.json"
));

/// Original default-build content, including every pointer and qualifier key.
const CONTENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/synthetic/graph/descriptor-content-preimage.json"
));

#[test]
fn typed_descriptor_identity_preimages_keep_the_original_bytes_and_ids() {
    let input = fixture();
    let documents = build(&input).unwrap();
    let preimages: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/synthetic/graph/descriptor-identity-preimages.json"
    )))
    .unwrap();
    assert_eq!(BUILDER_VERSION, "maestro-source-descriptors/1");
    for (document, golden) in documents.iter().zip(&preimages) {
        let bytes = serde_json::to_vec(&(
            BUILDER_VERSION,
            &document.target,
            &document.kind,
            &document.text,
            &document.pointers,
            &document.pin,
            &document.claim_set,
            &document.resolution,
            &document.qualifiers,
            document.eligible,
        ))
        .unwrap();
        assert_eq!(bytes, serde_json::to_vec(golden).unwrap());
        assert_eq!(document.id, Digest::of(&bytes));
    }
}

#[test]
fn original_descriptor_payloads_decode_and_reserialize_to_identical_bytes() {
    for golden in [
        CONTENT,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/synthetic/graph/descriptor-bounded-content-preimage.json"
        )),
    ] {
        let documents: Vec<Descriptor> = serde_json::from_str(golden).unwrap();
        assert_eq!(
            serde_json::to_string(&documents).unwrap(),
            golden.trim_end()
        );
        for document in documents {
            assert_eq!(document.id, document.identity());
        }
    }
}

#[test]
fn bounded_qualifiers_keep_sorted_conditions_and_half_open_bound_bytes() {
    let mut input = fixture();
    input.claims[0]
        .claim
        .conditions
        .insert("zeta".into(), "safe".into());
    input.claims[0]
        .claim
        .conditions
        .insert("alpha".into(), "café \"quoted\"".into());
    input.claims[0].claim.version = Validity::Bounded {
        start: Some("1.0".into()),
        end: Some("2.0".into()),
    };
    input.claims[0].claim.world = Validity::Bounded {
        start: None,
        end: Some("2030".into()),
    };
    let documents = build(&input).unwrap();
    let golden = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/synthetic/graph/descriptor-bounded-content-preimage.json"
    ));
    assert_eq!(
        serde_json::to_string(&documents).unwrap(),
        golden.trim_end()
    );
}

#[tokio::test]
async fn receipt_content_and_digest_preimages_keep_the_original_bytes() {
    let input = fixture();
    let documents = build(&input).unwrap();
    let card = card();
    let output = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    }
    .prepare(&input.pin, &documents, None)
    .await
    .unwrap();

    assert_eq!(
        output.receipt().content,
        Digest::of(CONTENT.trim_end().as_bytes())
    );
    assert_eq!(
        serde_json::to_string(&output.documents).unwrap(),
        CONTENT.trim_end()
    );
    let backend = Backend::default();
    qdrant::rebuild(&backend, &output).await.unwrap();
    assert_eq!(
        serde_json::to_string(output.receipt()).unwrap(),
        RECEIPT.trim_end()
    );
    let digest = Digest::of(RECEIPT.trim_end().as_bytes());
    let layout = backend.layout.lock().unwrap();
    assert_eq!(
        layout.as_ref().unwrap().0,
        format!("maestro-descriptors-{}", digest.as_str())
    );
    for point in backend.points.lock().unwrap().values() {
        assert_eq!(point.payload["receipt_digest"], serde_json::json!(digest));
    }
}

#[test]
fn typed_receipt_preimage_decodes_and_reserializes_to_original_bytes() {
    let receipt: DescriptorReceipt = serde_json::from_str(RECEIPT).unwrap();
    assert_eq!(serde_json::to_string(&receipt).unwrap(), RECEIPT.trim_end());
}
