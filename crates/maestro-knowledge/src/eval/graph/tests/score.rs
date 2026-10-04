//! The scores: a proof counts only when every anchor of one allowed proof is
//! in the delivered evidence bundle, an item counts once, the paired gain
//! follows the spec's 2,000-resample seed-0 rule, and construction precision
//! is a point estimate over reviewed claims.

use super::support::{REVISION, answerable, chain, check, labels, located, retries};
use crate::eval::graph::{
    ClaimReview, ClaimVerdict, ConstructionError, FamilyProof, GAIN_SEED, GainError, Located,
    Stage, Triple, proof_complete, proof_gain, score_construction, score_proofs,
};
use std::{collections::BTreeMap, slice::from_ref};

/// The two anchors of the chain of `q-2`.
fn chain_anchors() -> [Located; 2] {
    [
        located("The relay requires the lantern."),
        located("The lantern is part of the beacon."),
    ]
}

#[test]
fn a_partial_chain_scores_zero() {
    let [first, second] = chain_anchors();
    let proofs = vec![vec![first.clone(), second.clone()]];
    assert!(!proof_complete(&proofs, from_ref(&first)));
    assert!(!proof_complete(&proofs, &[]));
    assert!(proof_complete(&proofs, &[second, first]));
}

#[test]
fn an_anchor_counts_only_inside_a_delivered_passage_of_its_revision() {
    let [first, _] = chain_anchors();
    let proofs = vec![vec![first.clone()]];
    let around = Located {
        revision_id: REVISION.to_owned(),
        span: [first.span[0] - 1, first.span[1] + 1],
    };
    assert!(proof_complete(&proofs, from_ref(&around)));
    let cut = Located {
        revision_id: REVISION.to_owned(),
        span: [first.span[0], first.span[1] - 1],
    };
    assert!(!proof_complete(&proofs, &[cut]), "a truncated passage");
    let elsewhere = Located {
        revision_id: "revision-2".to_owned(),
        span: first.span,
    };
    assert!(!proof_complete(&proofs, &[elsewhere]));
    assert!(!proof_complete(&[Vec::new()], &[first]), "an empty proof");
}

#[test]
fn a_complete_alternative_scores_once_and_unanswerable_items_are_not_scored() {
    let mut labels = labels();
    labels[1] = answerable("q-2", "f-2", "multi_hop", &[chain(), chain()]);
    let checked = check(&labels, Stage::Frozen).unwrap();
    let [first, second] = chain_anchors();
    let delivered = BTreeMap::from([
        ("q-2".to_owned(), vec![first, second]),
        ("q-3".to_owned(), vec![located("| retries | 3 |\n")]),
    ]);
    let score = score_proofs(&checked.items, &delivered);
    assert_eq!((score.complete, score.of), (1, 2));
    assert_eq!(
        score.families,
        vec![
            FamilyProof {
                family: "f-1".to_owned(),
                complete: false
            },
            FamilyProof {
                family: "f-2".to_owned(),
                complete: true
            },
        ]
    );
    drop(retries());
}

/// `pairs` families, the first `wins` complete only in the candidate and the
/// next `losses` only in the baseline; the baseline and candidate lists.
fn paired(pairs: usize, wins: usize, losses: usize) -> (Vec<FamilyProof>, Vec<FamilyProof>) {
    let family = |index: usize, complete: bool| FamilyProof {
        family: format!("f-{index:03}"),
        complete,
    };
    let baseline = (0..pairs)
        .map(|index| family(index, index >= wins && index < wins + losses))
        .collect();
    let candidate = (0..pairs)
        .map(|index| family(index, index < wins))
        .collect();
    (baseline, candidate)
}

#[test]
fn four_wins_and_no_loss_among_80_meet_the_five_point_gate() {
    let (baseline, candidate) = paired(80, 4, 0);
    let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
    assert_eq!((gain.pairs, gain.wins, gain.losses), (80, 4, 0));
    assert!((gain.observed - 0.05).abs() < 1e-12, "{gain:?}");
    // The 50th and 1,950th of the 2,000 sorted seed-0 resampled deltas.
    assert!((gain.low - 1.0 / 80.0).abs() < 1e-12, "{gain:?}");
    assert!((gain.high - 8.0 / 80.0).abs() < 1e-12, "{gain:?}");
    assert!(gain.passed);
}

#[test]
fn three_wins_and_no_loss_among_80_fail_the_five_point_gate() {
    let (baseline, candidate) = paired(80, 3, 0);
    let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
    assert!((gain.observed - 0.0375).abs() < 1e-12, "{gain:?}");
    assert!(gain.low >= 0.0, "{gain:?}");
    assert!(!gain.passed);
}

#[test]
fn a_gain_with_a_nonpositive_50th_delta_fails_even_at_five_points() {
    let (baseline, candidate) = paired(80, 8, 4);
    let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
    assert_eq!((gain.wins, gain.losses), (8, 4));
    assert!((gain.observed - 0.05).abs() < 1e-12, "{gain:?}");
    assert!(gain.low <= 0.0, "{gain:?}");
    assert!(!gain.passed);
}

#[test]
fn the_gain_refuses_duplicate_unpaired_or_no_families() {
    let (mut baseline, candidate) = paired(3, 1, 0);
    baseline[1].family = baseline[0].family.clone();
    assert_eq!(
        proof_gain(&baseline, &candidate, GAIN_SEED),
        Err(GainError::DuplicateFamily)
    );
    let (baseline, mut candidate) = paired(3, 1, 0);
    candidate[2].family = "f-999".to_owned();
    assert_eq!(
        proof_gain(&baseline, &candidate, GAIN_SEED),
        Err(GainError::UnpairedFamily)
    );
    assert_eq!(proof_gain(&[], &[], GAIN_SEED), Err(GainError::Empty));
}

/// `count` reviews with `verdict`, their quotes exact.
fn reviews(count: usize, verdict: ClaimVerdict) -> Vec<ClaimReview> {
    (0..count)
        .map(|index| ClaimReview {
            claim_id: format!("claim-{verdict:?}-{index}"),
            verdict,
            quote_exact: true,
        })
        .collect()
}

#[test]
fn construction_precision_is_a_95_percent_point_estimate() {
    let mut nineteen = reviews(19, ClaimVerdict::Correct);
    nineteen.extend(reviews(1, ClaimVerdict::Incorrect));
    let score = score_construction(&nineteen, &[], &[]).unwrap();
    assert_eq!((score.correct, score.accepted), (19, 20));
    assert!(score.precision_passed && score.exact);
    let mut eighteen = reviews(18, ClaimVerdict::Correct);
    eighteen.extend(reviews(2, ClaimVerdict::Incorrect));
    assert!(
        !score_construction(&eighteen, &[], &[])
            .unwrap()
            .precision_passed
    );
    assert!(
        !score_construction(&[], &[], &[]).unwrap().precision_passed,
        "no claim"
    );
    let mut unresolved = reviews(40, ClaimVerdict::Correct);
    unresolved.extend(reviews(1, ClaimVerdict::Unresolved));
    let score = score_construction(&unresolved, &[], &[]).unwrap();
    assert_eq!(score.unresolved, 1);
    assert!(!score.precision_passed);
}

#[test]
fn every_accepted_quote_must_be_exact_and_recall_counts_gold_links_built() {
    let mut inexact = reviews(20, ClaimVerdict::Correct);
    inexact[3].quote_exact = false;
    let score = score_construction(&inexact, &[], &[]).unwrap();
    assert_eq!(score.inexact_quotes, 1);
    assert!(score.precision_passed && !score.exact);
    let triple = |subject: &str| Triple {
        subject: subject.to_owned(),
        predicate: "DEFAULTS_TO".to_owned(),
        object: "3".to_owned(),
    };
    let gold = [triple("retries"), triple("timeout")];
    let built = [triple("retries"), triple("other"), triple("retries")];
    let score = score_construction(&reviews(1, ClaimVerdict::Correct), &gold, &built).unwrap();
    assert_eq!((score.gold_found, score.gold), (1, 2));
}

#[test]
fn repeated_claim_reviews_cannot_inflate_precision() {
    for verdict in [ClaimVerdict::Correct, ClaimVerdict::Incorrect] {
        let mut repeated = vec![reviews(1, ClaimVerdict::Correct)[0].clone(); 19];
        let mut last = repeated[0].clone();
        last.verdict = verdict;
        repeated.push(last);
        assert_eq!(
            score_construction(&repeated, &[], &[]),
            Err(ConstructionError::DuplicateClaim)
        );
    }
}

#[test]
fn only_the_second_distinct_alternative_is_delivered() {
    let [first, second] = chain_anchors();
    assert!(proof_complete(
        &[vec![first], vec![second.clone()]],
        &[second]
    ));
}

#[test]
fn ten_wins_among_200_pass_and_nine_fail_with_seed_zero_bounds() {
    let (baseline, candidate) = paired(200, 10, 0);
    let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
    assert_eq!((gain.pairs, gain.wins, gain.losses), (200, 10, 0));
    assert!((gain.observed - 0.05).abs() < 1e-12);
    assert_eq!((gain.low, gain.high), (5.0 / 200.0, 16.0 / 200.0));
    assert!(gain.passed);
    let (baseline, candidate) = paired(200, 9, 0);
    let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
    assert!((gain.observed - 0.045).abs() < 1e-12);
    assert!(!gain.passed);
}

#[test]
fn historical_80_pair_losses_fail_at_five_points_with_exact_bounds() {
    for (wins, losses, low, high) in [
        (5, 1, 0.0, 0.1125),
        (6, 2, -0.0125, 0.125),
        (8, 4, -0.0375, 0.1375),
    ] {
        let (baseline, candidate) = paired(80, wins, losses);
        let gain = proof_gain(&baseline, &candidate, GAIN_SEED).unwrap();
        assert_eq!((gain.low, gain.high), (low, high));
        assert!((gain.observed - 0.05).abs() < 1e-12);
        assert!(!gain.passed);
    }
}
