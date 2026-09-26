//! Each failure of an answerable question gets its class at each cut-off it
//! fails at, with the routes whose trace did not find an expected section and
//! the routes that could not run.

use super::support::{ALL_RAN, Hit, bundle, bundle_with, hit, question, sections};
use crate::eval::{Failure, FailureClass, judge::judge};
use std::collections::BTreeMap;

/// The failures of a question expecting `expected`, among `hits`, when the
/// routes are `routes`.
fn failures(expected: &[&str], hits: &[Hit], routes: &[(&str, Option<&str>)]) -> Vec<Failure> {
    judge(
        &question("failing", true),
        &sections(expected),
        &bundle_with(hits, routes),
        0,
    )
    .failures
}

/// The failure at `cutoff` of `class`, missed by `missed_by`, with
/// `unavailable` routes and their reasons.
fn failure(
    cutoff: u32,
    class: FailureClass,
    missed_by: &[&str],
    unavailable: &[(&str, &str)],
) -> Failure {
    Failure {
        cutoff,
        class,
        missed_by: missed_by.iter().map(|&route| route.to_owned()).collect(),
        unavailable: unavailable
            .iter()
            .map(|&(route, reason)| (route.to_owned(), reason.to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// `count` passages of `section`, numbered from `first`, found by `bm25` and
/// `dense`, the first scored highest.
fn filler(first: u32, count: u32, section: &'static str) -> Vec<Hit> {
    (first..first + count)
        .map(|n| hit(n, section, 1.0 / f64::from(n)))
        .collect()
}

#[test]
fn an_expected_section_nowhere_in_the_bundle_is_not_retrieved_at_each_cutoff() {
    let routes = [
        ("bm25", None),
        ("dense", None),
        ("graph", Some("the graph is not built")),
        ("rerank", None),
    ];
    let expected = [
        failure(
            5,
            FailureClass::NotRetrieved,
            &["bm25", "dense"],
            &[("graph", "the graph is not built")],
        ),
        failure(
            10,
            FailureClass::NotRetrieved,
            &["bm25", "dense"],
            &[("graph", "the graph is not built")],
        ),
    ];
    assert_eq!(
        failures(&["wanted"], &filler(1, 3, "other"), &routes),
        expected
    );
}

#[test]
fn an_empty_bundle_is_not_retrieved_by_any_route_that_ran() {
    let routes = [("bm25", None), ("dense", Some("no room for the embedder"))];
    let unavailable = [("dense", "no room for the embedder")];
    assert_eq!(
        failures(&["wanted"], &[], &routes),
        [
            failure(5, FailureClass::NotRetrieved, &["bm25"], &unavailable),
            failure(10, FailureClass::NotRetrieved, &["bm25"], &unavailable),
        ]
    );
}

#[test]
fn a_section_below_the_cutoff_is_misranked_and_names_the_routes_that_missed_it() {
    let mut hits = filler(1, 6, "other");
    hits.push(Hit {
        routes: &["bm25"],
        ..hit(7, "wanted", 0.01)
    });
    let routes = [("bm25", None), ("dense", None), ("rerank", Some("no room"))];
    assert_eq!(
        failures(&["wanted"], &hits, &routes),
        [failure(
            5,
            FailureClass::Misranked,
            &["dense"],
            &[("rerank", "no room")]
        )]
    );
}

#[test]
fn a_section_past_the_last_cutoff_is_misranked_at_both() {
    let mut hits = filler(1, 11, "other");
    hits.push(Hit {
        routes: &["dense"],
        ..hit(12, "wanted", 0.01)
    });
    assert_eq!(
        failures(&["wanted"], &hits, &ALL_RAN),
        [
            failure(5, FailureClass::Misranked, &["bm25"], &[]),
            failure(10, FailureClass::Misranked, &["bm25"], &[]),
        ]
    );
}

#[test]
fn a_question_with_two_sections_fails_only_when_neither_is_within_the_cutoff() {
    let mut hits = filler(1, 11, "other");
    hits[2] = Hit {
        routes: &["dense"],
        ..hit(3, "one", 1.0 / 3.0)
    };
    hits.push(Hit {
        routes: &["bm25"],
        ..hit(12, "two", 0.01)
    });
    assert_eq!(failures(&["one", "two"], &hits, &ALL_RAN), []);
    hits[2] = Hit {
        routes: &["dense"],
        ..hit(3, "other", 1.0 / 3.0)
    };
    hits[7] = Hit {
        routes: &["dense"],
        ..hit(8, "one", 1.0 / 8.0)
    };
    // Each route found one of the two sections: none missed them both.
    assert_eq!(
        failures(&["one", "two"], &hits, &ALL_RAN),
        [failure(5, FailureClass::Misranked, &[], &[])]
    );
}

#[test]
fn a_section_at_a_cutoff_ranks_within_it() {
    let cutoffs_failed = |rank: u32| -> Vec<u32> {
        let hits: Vec<Hit> = (1..=12)
            .map(|n| {
                let section = if n == rank { "wanted" } else { "other" };
                hit(n, section, 1.0 / f64::from(n))
            })
            .collect();
        let failures = failures(&["wanted"], &hits, &ALL_RAN);
        failures.iter().map(|failure| failure.cutoff).collect()
    };
    assert_eq!(cutoffs_failed(5), Vec::<u32>::new());
    assert_eq!(cutoffs_failed(6), [5]);
    assert_eq!(cutoffs_failed(10), [5]);
    assert_eq!(cutoffs_failed(11), [5, 10]);
}

#[test]
fn an_unanswerable_question_has_no_failure_class_even_with_passages() {
    let result = judge(
        &question("unanswerable", false),
        &[],
        &bundle(&filler(1, 3, "other")),
        0,
    );
    assert_eq!(result.passages, 3);
    assert_eq!(result.failures, []);
}
