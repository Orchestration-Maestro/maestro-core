//! Duplicate claim identity and post-review source rejection checks.

use super::{DeterministicExtractor, GraphExtractor, ModelExtractor, policy, source, test_card};
use maestro_kernel::{
    artifact::Digest,
    facts::{EntityKind, EntityName, Object, Predicate},
    gateway::Candidate,
};
use std::fs;

#[test]
fn duplicate_model_candidates_produce_one_claim_and_a_successful_extraction() {
    let text = "Command launch requires component core.\n";
    let source = source(text);
    let candidate = Candidate {
        subject: EntityName {
            kind: EntityKind::Command,
            name: "launch".into(),
        },
        predicate: Predicate::Requires,
        object: Object::Entity(EntityName {
            kind: EntityKind::Component,
            name: "core".into(),
        }),
        quote: "Command launch requires component core".into(),
    };
    let (path, card) = test_card();
    let extractor = ModelExtractor::new(
        DeterministicExtractor {
            candidates: vec![candidate.clone(), candidate],
            refuse: false,
        },
        card,
        policy(128, 0, 8),
        Digest::of(b"policy"),
        2048,
    )
    .expect("model extraction runtime");

    let extraction = GraphExtractor::extract(&extractor, &source);

    assert_eq!(extraction.claims.len(), 1);
    assert!(extraction.rejections.is_empty());
    fs::remove_dir_all(path).expect("remove synthetic card");
}

#[test]
fn overlap_quote_produces_one_claim_and_a_successful_batch() {
    let source = source("0123456789abcdefghij");
    let candidate = Candidate {
        subject: EntityName {
            kind: EntityKind::Command,
            name: "launch".into(),
        },
        predicate: Predicate::Requires,
        object: Object::Entity(EntityName {
            kind: EntityKind::Component,
            name: "core".into(),
        }),
        quote: "89ab".into(),
    };
    let (path, card) = test_card();
    let extractor = ModelExtractor::new(
        DeterministicExtractor {
            candidates: vec![candidate],
            refuse: false,
        },
        card,
        policy(12, 4, 8),
        Digest::of(b"policy"),
        2048,
    )
    .expect("model extraction runtime");

    let extraction = GraphExtractor::extract(&extractor, &source);

    assert_eq!(extraction.claims.len(), 1);
    assert!(extraction.rejections.is_empty());
    fs::remove_dir_all(path).expect("remove synthetic card");
}

#[test]
fn model_reply_claiming_alias_of_is_refused() {
    let source = source("Command launch has alias name run.\n");
    let candidate = Candidate {
        subject: EntityName {
            kind: EntityKind::Command,
            name: "launch".into(),
        },
        predicate: Predicate::AliasOf,
        object: Object::Entity(EntityName {
            kind: EntityKind::Command,
            name: "run".into(),
        }),
        quote: "Command launch has alias name run".into(),
    };
    let (path, card) = test_card();
    let extractor = ModelExtractor::new(
        DeterministicExtractor {
            candidates: vec![candidate],
            refuse: false,
        },
        card,
        policy(128, 0, 8),
        Digest::of(b"policy"),
        2048,
    )
    .expect("model extraction runtime");

    let extraction = GraphExtractor::extract(&extractor, &source);

    assert!(extraction.claims.is_empty());
    assert_eq!(extraction.rejections.len(), 1);
    assert_eq!(
        extraction.rejections[0].reason,
        "predicate is not claimable"
    );
    fs::remove_dir_all(path).expect("remove synthetic card");
}

#[test]
fn model_extraction_refuses_canonical_markdown_mismatch() {
    let original = source("Trusted source text.\n");
    let mismatched = super::Source::new(
        original.revision_id().to_owned(),
        original.canonical().clone(),
        "Changed source text.\n".to_owned(),
    );
    let (path, card) = test_card();
    let extractor = ModelExtractor::new(
        DeterministicExtractor {
            candidates: Vec::new(),
            refuse: false,
        },
        card,
        policy(128, 0, 8),
        Digest::of(b"policy"),
        2048,
    )
    .expect("model extraction runtime");

    let extraction = GraphExtractor::extract(&extractor, &mismatched);

    assert!(extraction.claims.is_empty());
    assert_eq!(extraction.rejections.len(), 1);
    assert_eq!(
        extraction.rejections[0].reason,
        "source verification failed"
    );
    fs::remove_dir_all(path).expect("remove synthetic card");
}
