//! Qualified measurements, not card invocation claims, constrain S1 adaptation.
use super::{n30_support::Fixture, n57_processing_artifacts::initial};
use maestro_acquisition::{
    Ref,
    adaptation::{artifacts::S1ChunkStrategy, storage},
};
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        ModelCard, Role,
        card_v2::{Capability, Dimensions, EmbeddingFormat, Observation},
    },
};

#[test]
fn n57_forged_and_unavailable_model_measurements_hold() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let effective = &snapshot.effective;
    let mut strategy: S1ChunkStrategy = storage::artifact(
        &fixture.db,
        "synthetic-reader",
        &effective.sources["notes"].2.chunk,
    )
    .unwrap();
    let model = &effective.protected_resources["s1_embedding_model"];
    let tokenizer = &effective.protected_resources["s1_tokenizer_qualification"];
    let card = ModelCard::from_json_bytes(&fixture.catalog.0[&model.id].bytes).unwrap();
    assert!(strategy.validate(&card, model, tokenizer).is_ok());
    let store = Store::new(fixture.root.join("synthetic-models"));
    for unavailable in [false, true] {
        let mut identity = card.identity().unwrap().clone();
        identity.resources.qualified_limits.context_tokens = if unavailable {
            Observation::Unavailable {
                reason: "not measured".into(),
            }
        } else {
            Observation::Measured {
                value: 8192.try_into().unwrap(),
                provenance: "synthetic overclaim".into(),
            }
        };
        let forged = ModelCard::record_v2(&store, &identity).unwrap();
        let model = Ref {
            id: "model".into(),
            digest: forged.digest().clone(),
        };
        strategy.model = model.clone();
        assert!(strategy.validate(&forged, &model, tokenizer).is_err());
    }
}

#[test]
fn n57_embedder_role_and_complete_prepared_budget_are_required() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let effective = &snapshot.effective;
    let strategy: S1ChunkStrategy = storage::artifact(
        &fixture.db,
        "synthetic-reader",
        &effective.sources["notes"].2.chunk,
    )
    .unwrap();
    let model = &effective.protected_resources["s1_embedding_model"];
    let tokenizer = &effective.protected_resources["s1_tokenizer_qualification"];
    let card = ModelCard::from_json_bytes(&fixture.catalog.0[&model.id].bytes).unwrap();
    let store = Store::new(fixture.root.join("synthetic-models"));
    for fault in ["role", "context", "tokenizer", "digest"] {
        let mut identity = card.identity().unwrap().clone();
        match fault {
            "role" => {
                identity.role = Role::Reranker;
                identity.invocation.dimensions = Dimensions::NotApplicable;
                identity.formats.document = Capability::NotApplicable;
                identity.formats.query = Capability::NotApplicable;
                identity.formats.embedding = EmbeddingFormat::NotApplicable;
            }
            "context" => {
                identity.invocation.limits.context_tokens = 699.try_into().unwrap();
                identity.resources.qualified_limits.context_tokens = Observation::Measured {
                    value: 699.try_into().unwrap(),
                    provenance: "synthetic".into(),
                };
            }
            "tokenizer" => identity.formats.qualification_digest = Digest::of(b"wrong evidence"),
            _ => {}
        }
        let card = ModelCard::record_v2(&store, &identity).unwrap();
        let mut model = Ref {
            id: "model".into(),
            digest: card.digest().clone(),
        };
        if fault == "digest" {
            model.digest = Digest::of(b"wrong card pin");
        }
        let mut invalid = strategy.clone();
        invalid.model = model.clone();
        invalid.model_limits = card.fields().limits;
        assert!(
            invalid.validate(&card, &model, tokenizer).is_err(),
            "{fault}"
        );
    }
}
