//! S1 capabilities and scope inheritance, with no real model qualification.
use super::{n30_support::Fixture, n57_processing_artifacts::initial, n57_support as support};
use maestro_acquisition::adaptation::change::selection_key;
use maestro_acquisition::{
    Ref,
    adaptation::{
        artifacts::{DedupKeys, S1ChunkStrategy},
        storage,
    },
    extraction::outcome::ProfileSelection,
};
use maestro_kernel::{acquisition::Receipts, artifact::Digest, gateway::ModelCard, scope::Right};
#[test]
fn n57_s1_supported_limits_and_keys() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let effective = &snapshot.effective;
    let pin = &effective.sources["notes"].2.chunk;
    let strategy: S1ChunkStrategy =
        storage::artifact(&fixture.db, "synthetic-reader", pin).unwrap();
    let model = &effective.protected_resources["s1_embedding_model"];
    let tokenizer = &effective.protected_resources["s1_tokenizer_qualification"];
    let card = ModelCard::from_json_bytes(&fixture.catalog.0[&model.id].bytes).unwrap();
    assert!(strategy.validate(&card, model, tokenizer).is_ok());
    for fault in [
        "version",
        "preparation",
        "target",
        "hard",
        "model",
        "tokenizer",
        "context",
        "output",
    ] {
        let mut invalid = strategy.clone();
        match fault {
            "version" => invalid.chunker_version = "new/1".into(),
            "preparation" => invalid.preparation_profile = "wrong/1".into(),
            "target" => invalid.target_tokens = 499.try_into().unwrap(),
            "hard" => invalid.hard_max_tokens = 701.try_into().unwrap(),
            "model" => invalid.model.digest = Digest::of(b"other model"),
            "tokenizer" => invalid.tokenizer_qualification.digest = Digest::of(b"other tokenizer"),
            "context" => invalid.model_limits.context_tokens = 8192.try_into().unwrap(),
            _ => invalid.model_limits.output_tokens = Some(1.try_into().unwrap()),
        }
        assert!(
            invalid.validate(&card, model, tokenizer).is_err(),
            "{fault}"
        );
    }
    let pin = &effective.sources["notes"].2.dedup;
    let keys: DedupKeys = storage::artifact(&fixture.db, "synthetic-reader", pin).unwrap();
    assert!(keys.validate().is_ok());
    for fault in ["order", "duplicate", "lossy", "prepared"] {
        let mut invalid = keys.clone();
        match fault {
            "order" => invalid.exact.reverse(),
            "duplicate" => invalid.exact.push(invalid.exact[0]),
            "lossy" => {
                invalid.exact.pop();
            }
            _ => invalid.prepared.reverse(),
        }
        assert!(invalid.validate().is_err(), "{fault}");
    }
}
#[test]
fn n57_snapshot_inherits_cross_collection_evidence_scope() {
    let fixture = Fixture::new();
    let private = "workspace/default/collection/other".parse().unwrap();
    fixture
        .db
        .grant("synthetic-reader", &private, Right::Read, "owner")
        .unwrap();
    let foreign = fixture
        .db
        .retain(&private, b"synthetic cross-collection evidence", &[])
        .unwrap();
    let foreign = Ref {
        id: foreign.to_string(),
        digest: Digest::of(b"synthetic cross-collection evidence"),
    };
    let mut snapshot = initial(&fixture);
    let selection = support::retain(
        &fixture.db,
        &ProfileSelection::Selected {
            profile: snapshot.effective.sources["notes"].0.clone(),
            evidence: vec![foreign],
        },
        &[],
    );
    snapshot
        .effective
        .protected_resources
        .insert(selection_key("notes"), selection);
    let pin = support::retain_snapshot(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &snapshot,
    );
    fixture
        .db
        .revoke("synthetic-reader", &private, Right::Read, "owner")
        .unwrap();
    assert!(
        fixture
            .db
            .read("synthetic-reader", pin.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
    let scopes = fixture.db.visible("synthetic-reader").unwrap();
    let principal = super::support::principal(&scopes);
    assert!(
        support::reader(
            &fixture.db,
            &fixture.collection,
            &fixture.catalog,
            &principal
        )
        .read(&pin)
        .is_err()
    );
}
