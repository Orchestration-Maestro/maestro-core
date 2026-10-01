//! Canonical held-endpoint selection and pair-local refusal.

use super::{DescriptorError, DescriptorInput, build, tests::fixture};
use maestro_kernel::{
    artifact::Digest,
    facts::{Literal, LiteralKind, Object},
};

#[test]
fn g10_review_hold_error_is_independent_of_attached_claim_order() {
    let mut input = fixture();
    let mut reversed = input.claims[0].clone();
    reversed.id = Digest::of(b"g10 reversed attached claim");
    let Object::Entity(beta) = &input.claims[0].claim.object else {
        panic!("entity fixture");
    };
    reversed.claim.subject = beta.clone();
    reversed.claim.object = Object::Entity(input.claims[0].claim.subject.clone());
    input.claims.push(reversed.clone());
    input.snapshot.claims.push(reversed);
    let mut lower_alpha = input.snapshot.claims[0].clone();
    lower_alpha.id = Digest::of(b"g10 lower alpha collision");
    lower_alpha.claim.subject.name = "alpha".into();
    input.snapshot.claims.push(lower_alpha);
    let mut lower_beta = input.snapshot.claims[1].clone();
    lower_beta.id = Digest::of(b"g10 lower beta collision");
    lower_beta.claim.subject.name = "beta".into();
    input.snapshot.claims.push(lower_beta);
    let before = build(&input).unwrap_err();
    input.claims.reverse();
    let after = build(&input).unwrap_err();
    eprintln!("R2 before = {before:?}");
    eprintln!("R2 after = {after:?}");
    assert_eq!(before, after);
}

#[test]
fn g10_review_object_only_hold_refuses_entire_build() {
    let input = object_held_input();
    let expected = DescriptorError::HeldForReview(
        Digest::parse("9dad7d3c60690eb52f1f1944134592751d29e05cbd2edd8d6169c1155f59a6a6").unwrap(),
    );
    let result = build(&input);
    eprintln!("R3 expected = {expected:?}");
    eprintln!("R3 actual error = {:?}", result.as_ref().err());
    assert_eq!(result.err(), Some(expected));
}

#[test]
fn g10_review_object_hold_does_not_hold_unrelated_subject() {
    use crate::graph::resolve::resolve_snapshot;
    let input = object_held_input();
    let entities = resolve_snapshot(&input.snapshot).unwrap();
    let alpha = entities
        .iter()
        .find(|entity| entity.subject.name == "Alpha")
        .unwrap();
    let beta = entities
        .iter()
        .find(|entity| entity.subject.name == "Beta")
        .unwrap();
    eprintln!("R4 Alpha colliding = {:?}", alpha.colliding);
    eprintln!("R4 Beta colliding = {:?}", beta.colliding);
    assert!(
        alpha.colliding.is_empty(),
        "Alpha colliding = {:?}",
        alpha.colliding
    );
    assert_eq!(beta.colliding, vec![beta.subject.clone()]);
}

#[test]
fn held_entities_outside_attached_claims_do_not_block_the_build() {
    let mut input = fixture();
    for name in ["Gamma", "gamma"] {
        let mut unrelated = input.snapshot.claims[0].clone();
        unrelated.id = Digest::of(name.as_bytes());
        unrelated.claim.subject.name = name.into();
        unrelated.claim.object = Object::Literal(Literal {
            kind: LiteralKind::Text,
            lexeme: "unrelated".into(),
        });
        input.snapshot.claims.push(unrelated);
    }
    assert_eq!(build(&input), build(&fixture()));
}

/// Two distinct object mentions separated by a sourced namespace review.
fn object_held_input() -> DescriptorInput {
    use maestro_kernel::facts::{Decision, DecisionKind, Endpoint, Mention, ReviewRecord};
    let mut input = fixture();
    let mut other = input.snapshot.claims[0].clone();
    other.id = Digest::of(b"g10 duplicate object namespace");
    input.snapshot.claims.push(other.clone());
    input.snapshot.history.push(ReviewRecord {
        reviewer: "reviewer".into(),
        decision: Decision {
            left: Mention {
                claim: input.claims[0].id.clone(),
                endpoint: Endpoint::Object,
            },
            right: Mention {
                claim: other.id,
                endpoint: Endpoint::Object,
            },
            kind: DecisionKind::Separate,
            reason: "the sourced Beta components belong to unrelated documentary namespaces".into(),
        },
    });
    input
}
