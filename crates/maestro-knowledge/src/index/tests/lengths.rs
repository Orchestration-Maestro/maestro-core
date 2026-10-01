//! The lengths the sparse vectors are weighed against: the average term
//! count of every passage counted, zero with no passage or no term, when no
//! passage has a vector to weigh.

use super::super::sparse::Lengths;
use crate::lexical::{AverageLength, Passage};

#[test]
fn no_passage_counted_has_a_mean_of_zero_and_no_vector() {
    let lengths = Lengths::default();
    assert!(lengths.mean().abs() < f64::EPSILON);
    assert!(lengths.vector("jobs retry").is_none());
}

#[test]
fn passages_without_terms_leave_every_vector_empty() {
    let mut lengths = Lengths::default();
    lengths.add("the of and");
    lengths.add("");
    assert!(lengths.mean().abs() < f64::EPSILON);
    assert!(lengths.vector("the of and").is_none());
}

#[test]
fn a_passage_is_weighed_against_the_mean_of_every_passage_counted() {
    let mut lengths = Lengths::default();
    for passage in ["job job lot", "the of and", "job retry"] {
        lengths.add(passage);
    }
    // 3 terms, 0, then 2: 5 over 3 passages.
    assert!((lengths.mean() - 5.0 / 3.0).abs() < 1e-12);
    let expected = Passage::new("job lot").vector(AverageLength::new(5.0 / 3.0).unwrap());
    assert_eq!(lengths.vector("job lot"), Some(expected));
}
