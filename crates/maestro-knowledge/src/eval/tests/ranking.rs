//! A bundle lists its passages in reading order, so the runner ranks them
//! itself: by the reranker's score, highest first, and by passage number
//! where scores are equal or absent; a score that is not a finite number
//! ranks as none.

use super::support::{Hit, bundle, hit, question, sections, unscored};
use crate::eval::{Expected, judge::judge};

/// The rank the runner gives each of `expected` among `hits`.
fn ranks(expected: &[&str], hits: &[Hit]) -> Vec<Option<u32>> {
    let result = judge(
        &question("ranked", true),
        &sections(expected),
        &bundle(hits),
        0,
    );
    result
        .expected
        .iter()
        .map(|Expected { rank, .. }| *rank)
        .collect()
}

#[test]
fn passages_rank_by_score_whatever_the_order_the_bundle_lists_them_in() {
    let hits = [
        hit(1, "low", 0.2),
        hit(2, "high", 0.9),
        hit(3, "middle", 0.5),
    ];
    assert_eq!(
        ranks(&["high", "middle", "low"], &hits),
        [Some(1), Some(2), Some(3)]
    );
}

#[test]
fn passages_of_equal_score_rank_by_number_lowest_first() {
    let hits = [
        hit(3, "third", 0.5),
        hit(1, "first", 0.9),
        hit(2, "second", 0.5),
    ];
    assert_eq!(
        ranks(&["first", "second", "third"], &hits),
        [Some(1), Some(2), Some(3)]
    );
}

#[test]
fn a_negative_zero_score_ties_with_a_zero_score() {
    let hits = [hit(2, "later", 0.0), hit(1, "earlier", -0.0)];
    assert_eq!(ranks(&["earlier", "later"], &hits), [Some(1), Some(2)]);
}

#[test]
fn passages_without_scores_rank_by_number_lowest_first() {
    let hits = [
        unscored(9, "ninth"),
        unscored(2, "second"),
        unscored(4, "fourth"),
    ];
    assert_eq!(
        ranks(&["second", "fourth", "ninth"], &hits),
        [Some(1), Some(2), Some(3)]
    );
}

#[test]
fn scored_passages_rank_before_unscored_ones() {
    let hits = [
        unscored(1, "unscored"),
        hit(2, "low", 0.1),
        hit(3, "high", 0.9),
    ];
    assert_eq!(
        ranks(&["high", "low", "unscored"], &hits),
        [Some(1), Some(2), Some(3)]
    );
}

#[test]
fn a_section_found_twice_takes_the_rank_of_its_best_passage() {
    let hits = [
        hit(1, "other", 0.9),
        hit(2, "twice", 0.2),
        hit(3, "twice", 0.5),
    ];
    assert_eq!(ranks(&["twice"], &hits), [Some(2)]);
}

#[test]
fn a_section_no_passage_holds_has_no_rank() {
    let without_section = Hit {
        section: None,
        ..hit(1, "unused", 0.9)
    };
    let hits = [without_section, hit(2, "other", 0.5)];
    assert_eq!(ranks(&["absent"], &hits), [None]);
}

#[test]
fn a_score_that_is_not_a_finite_number_ranks_as_none() {
    // A bundle is checked when it is read or written, not when a retrieval
    // hands it over in the process, so any score can reach the runner.
    let hits = [
        hit(1, "not-a-number", f64::NAN),
        hit(2, "scored", 0.1),
        hit(3, "infinite", f64::INFINITY),
        hit(4, "below-every-score", f64::NEG_INFINITY),
        unscored(5, "unscored"),
    ];
    assert_eq!(
        ranks(
            &[
                "scored",
                "not-a-number",
                "infinite",
                "below-every-score",
                "unscored"
            ],
            &hits
        ),
        [Some(1), Some(2), Some(3), Some(4), Some(5)]
    );
}
