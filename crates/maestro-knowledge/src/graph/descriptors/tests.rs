//! Source-only composition and fail-closed pointer checks.

use super::{DescriptorInput, DescriptorPin, build};
use crate::graph::{resolve::EXACT_RESOLVER_VERSION, verify::Source};
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        Provenance, ResolutionSnapshot, ReviewState, Support, Validity,
    },
};
use std::collections::BTreeMap;

/// Exact source paragraphs separated by an unquoted gap.
pub(super) const MARKDOWN: &str = "Alpha is a command.\n\nUnrelated gap.\n\nBeta is a component.\n";

/// Synthetic frozen authority, with one support per endpoint.
pub(super) fn fixture() -> DescriptorInput {
    fixture_with_source(MARKDOWN, ["Alpha is a command.", "Beta is a component."])
}

/// Synthetic original bytes with one explicitly sourced sentence per endpoint.
fn fixture_with_source(markdown: &str, sentences: [&str; 2]) -> DescriptorInput {
    let canonical =
        canonicalize(CanonicalizeInput::new(markdown, "synthetic:descriptors")).unwrap();
    let supports: Vec<_> = sentences
        .into_iter()
        .map(|text| {
            let start = markdown.find(text).unwrap();
            let span = Span {
                start,
                end: start + text.len(),
            };
            let block = canonical
                .blocks
                .iter()
                .find(|block| {
                    block
                        .source_spans
                        .iter()
                        .any(|outer| outer.start <= start && span.end <= outer.end)
                })
                .unwrap();
            Support {
                revision_id: canonical.revision_id.clone(),
                block_id: block.block_id.clone(),
                span,
                quote_digest: Digest::of(text.as_bytes()),
            }
        })
        .collect();
    let id = Digest::of(b"claim");
    let record = ClaimRecord {
        id,
        collection_id: "graph".into(),
        claim: Claim {
            subject: EntityName {
                kind: EntityKind::Command,
                name: "Alpha".into(),
            },
            predicate: Predicate::Requires,
            object: Object::Entity(EntityName {
                kind: EntityKind::Component,
                name: "Beta".into(),
            }),
            conditions: BTreeMap::new(),
            version: Validity::Unknown,
            world: Validity::Unknown,
            provenance: Provenance {
                extractor: "synthetic/1".into(),
                profile: Digest::of(b"rule"),
            },
            supports,
        },
        review: ReviewState::Accepted,
        recorded_at: "ignored".into(),
    };
    let revision = canonical.revision_id.clone();
    let set = Digest::of(b"set");
    DescriptorInput {
        pin: DescriptorPin {
            collection_id: "graph".into(),
            generation_id: 7,
            version: None,
        },
        claim_set: set.clone(),
        claims: vec![record.clone()],
        snapshot: ResolutionSnapshot {
            resolver_version: EXACT_RESOLVER_VERSION.into(),
            id: Digest::of(b"resolution"),
            previous: None,
            sets: vec![set],
            claims: vec![record],
            history: vec![],
        },
        sources: BTreeMap::from([(
            revision.clone(),
            Source::new(revision, canonical, markdown.into()),
        )]),
    }
}

#[test]
fn non_utf8_context_boundaries_are_refused_without_a_whole_source_fallback() {
    let markdown = "Alpha is a café command.\n\nUnrelated gap.\n\nBeta is a component.\n";
    let mut input = fixture_with_source(
        markdown,
        ["Alpha is a café command.", "Beta is a component."],
    );
    let support = &mut input.claims[0].claim.supports[0];
    support.span.start = markdown.find('é').unwrap() + 1;
    support.quote_digest = Digest::of(markdown.as_bytes());
    let result = build(&input);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().0, "invalid UTF-8 span");
}

#[test]
fn canonical_claim_text_preserves_both_disjoint_endpoint_pointers() {
    let input = fixture();
    let result = build(&input);
    assert!(result.is_ok(), "{result:?}");
    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 3);
    let golden: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/synthetic/graph/descriptors.json"
    )))
    .unwrap();
    assert_eq!(serde_json::json!(descriptors), golden);
    let claim = descriptors
        .iter()
        .find(|item| item.kind == "claim")
        .unwrap();
    assert_eq!(
        claim.text,
        "Alpha\nCommand\nAlpha is a command.\nREQUIRES\nBeta\nComponent\nBeta is a component."
    );
    assert_eq!(claim.pointers.len(), 2);
    assert_eq!(claim.pointers[0].span, Span { start: 0, end: 19 });
    assert_eq!(claim.pointers[1].span, Span { start: 37, end: 57 });
    assert_eq!(claim.target, Digest::of(b"claim"));
    assert_eq!(claim.pin.generation_id, 7);
    assert!(claim.eligible);
    assert_eq!(descriptors, build(&input).unwrap());
}

#[test]
fn conditions_and_known_half_open_validity_are_retained_without_inference() {
    let mut input = fixture();
    input.claims[0]
        .claim
        .conditions
        .insert("mode".into(), "safe".into());
    input.claims[0].claim.version = Validity::Bounded {
        start: Some("1.0".into()),
        end: Some("2.0".into()),
    };
    input.claims[0].claim.world = Validity::Bounded {
        start: None,
        end: Some("2030".into()),
    };
    let documents = build(&input).unwrap();
    assert!(documents.iter().all(|document| document.qualifiers
        == serde_json::json!({
            "conditions": {"mode": "safe"},
            "version": {"known": true, "start": "1.0", "end": "2.0"},
            "world": {"known": true, "start": null, "end": "2030"},
        })));
}

#[test]
fn literals_remain_claim_properties_not_entity_descriptors() {
    let mut input = fixture();
    input.claims[0].claim.object = Object::Literal(Literal {
        kind: LiteralKind::Text,
        lexeme: "command".into(),
    });
    input.claims[0].claim.predicate = Predicate::DefaultsTo;
    input.snapshot.claims = input.claims.clone();
    let documents = build(&input).unwrap();
    assert_eq!(documents.len(), 2);
    assert_eq!(
        documents
            .iter()
            .filter(|item| item.kind == "entity")
            .count(),
        1
    );
    assert!(
        documents
            .iter()
            .any(|item| item.text
                == "Alpha\nCommand\nAlpha is a command.\nDEFAULTS_TO\ntext\ncommand")
    );
}

#[test]
fn rejected_or_flagged_claims_remain_rebuildable_but_ineligible_for_lookup() {
    for review in [ReviewState::Rejected, ReviewState::Flagged] {
        let mut input = fixture();
        input.claims[0].review = review;
        let documents = build(&input).unwrap();
        assert_eq!(documents.len(), 3);
        assert!(documents.iter().all(|document| !document.eligible));
    }
}

#[test]
fn endpoint_context_must_name_the_endpoint_inside_its_canonical_block() {
    let mut input = fixture();
    input.claims[0].claim.supports.remove(0);
    assert!(build(&input).is_err());
    let mut input = fixture();
    let source = input.sources.values().next().unwrap();
    let mut canonical = source.canonical().clone();
    for block in &mut canonical.blocks {
        block.source_spans.clear();
    }
    let source = Source::new(source.revision_id().into(), canonical, MARKDOWN.into());
    input.sources.insert(source.revision_id().into(), source);
    assert!(build(&input).is_err());
}

#[test]
fn missing_endpoint_context_is_not_replaced_by_a_name_only_triple() {
    let mut input = fixture();
    input.claims[0].claim.supports.pop();
    assert!(build(&input).is_err());
}

#[test]
fn invented_defining_text_digest_is_refused() {
    let mut input = fixture();
    for support in &mut input.claims[0].claim.supports {
        support.quote_digest = Digest::of(b"invented prose");
    }
    assert!(build(&input).is_err());
}

#[test]
fn defining_context_cannot_expand_a_frozen_name_only_support() {
    let mut input = fixture();
    let support = input.claims[0]
        .claim
        .supports
        .iter_mut()
        .find(|support| support.span.start == 0)
        .unwrap();
    support.span.end = 5;
    support.quote_digest = Digest::of(b"Alpha");
    assert!(build(&input).is_err());
}

#[test]
fn unverified_context_outside_frozen_supports_is_refused() {
    let mut input = fixture();
    for support in &mut input.claims[0].claim.supports {
        support.span = Span { start: 21, end: 35 };
        support.quote_digest = Digest::of(b"Unrelated gap.");
    }
    assert!(build(&input).is_err());
}

#[test]
fn wrong_generation_source_membership_is_refused() {
    let mut input = fixture();
    input.sources.clear();
    assert!(build(&input).is_err());
}

#[test]
fn canonical_artifact_from_other_original_bytes_is_refused() {
    let mut input = fixture();
    let source = input.sources.values().next().unwrap();
    let changed = Source::new(
        source.revision_id().into(),
        source.canonical().clone(),
        MARKDOWN.replace("Unrelated", "Fabricate"),
    );
    input.sources.insert(changed.revision_id().into(), changed);
    assert!(build(&input).is_err());
}

#[test]
fn context_cannot_conflate_disjoint_source_spans_into_one_quote() {
    let mut input = fixture();
    for support in &mut input.claims[0].claim.supports {
        support.span = Span {
            start: 0,
            end: MARKDOWN.len(),
        };
        support.quote_digest = Digest::of(MARKDOWN.as_bytes());
    }
    assert!(build(&input).is_err());
}

#[test]
fn name_only_and_embedded_subword_are_not_defining_contexts() {
    for sentence in [
        "Alpha",
        "Alphabet is a command.",
        "éAlpha is a command.",
        "Alpha_ is a command.",
    ] {
        let input = fixture_with_source(
            &format!("{sentence}\n\nBeta is a component.\n"),
            [sentence, "Beta is a component."],
        );
        assert!(build(&input).is_err(), "accepted {sentence}");
    }
}

#[test]
fn endpoint_context_selection_uses_frozen_support_order_not_caller_order() {
    let markdown = "Alpha is a command. Alpha is a tool.\n\nBeta is a component.\n";
    let mut input = fixture_with_source(markdown, ["Alpha is a command.", "Beta is a component."]);
    let other = fixture_with_source(markdown, ["Alpha is a tool.", "Beta is a component."]);
    input.claims[0]
        .claim
        .supports
        .insert(0, other.claims[0].claim.supports[0].clone());
    let documents = build(&input).unwrap();
    assert!(
        documents
            .iter()
            .any(|item| item.text == "Alpha\nCommand\nAlpha is a command.")
    );
    input.claims[0].claim.supports.reverse();
    assert_eq!(documents, build(&input).unwrap());
}
