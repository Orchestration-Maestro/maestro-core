//! A question may expect a document without sections whole: any passage of
//! that document holds it, a passage of another document does not, and a
//! section is held by its own passages only, never by another passage of its
//! document. A document held by several passages gains once, at its best
//! rank.

use super::support::{DOCUMENT, Hit, bundle, close, documents, hit, question, sections, whole};
use crate::eval::{Expected, Failure, FailureClass, Metrics, judge::judge, metric::measure};
use std::collections::BTreeMap;

/// The metrics of one answerable question that expects `expected` and got
/// `hits`.
fn metrics(expected: &[Expected], hits: &[Hit]) -> Metrics {
    let result = judge(&question("whole", true), expected, &bundle(hits), 0);
    measure(&[result], 1)
}

/// The rank the runner gives each of `expected` among `hits`.
fn ranks(expected: &[Expected], hits: &[Hit]) -> Vec<Option<u32>> {
    let result = judge(&question("whole", true), expected, &bundle(hits), 0);
    result
        .expected
        .iter()
        .map(|Expected { rank, .. }| *rank)
        .collect()
}

#[test]
fn a_document_expected_whole_takes_the_rank_of_its_best_passage() {
    let hits = [
        hit(1, "other", 0.9),
        whole(2, "article", 0.5),
        whole(3, "article", 0.7),
    ];
    assert_eq!(ranks(&documents(&["article"]), &hits), [Some(2)]);
}

#[test]
fn a_document_expected_whole_gains_once_however_many_passages_hold_it() {
    let hits = [
        whole(1, "article", 0.9),
        whole(2, "article", 0.5),
        hit(3, "other", 0.1),
    ];
    let found = metrics(&documents(&["article"]), &hits);
    close(found.ndcg_at_10.unwrap().value, 1.0);
    close(found.mrr_at_10.unwrap().value, 1.0);
    // Only the article is retrieved, first: 1 / (1 + 1/log2 3). A gain for
    // its second passage would give 1.
    let half = metrics(&documents(&["article", "unfound"]), &hits);
    close(half.ndcg_at_10.unwrap().value, 0.613_147_192_765_458_4);
}

#[test]
fn a_passage_of_another_document_does_not_hold_a_document_expected_whole() {
    let hits = [whole(1, "other-article", 0.9), hit(2, "section", 0.5)];
    assert_eq!(ranks(&documents(&["article"]), &hits), [None]);
}

#[test]
fn a_section_is_held_by_its_own_passages_only() {
    let of_its_document = Hit {
        section: None,
        ..hit(1, "unused", 0.9)
    };
    let hits = [of_its_document, hit(2, "other", 0.5)];
    assert_eq!(hits[0].document, DOCUMENT);
    assert_eq!(ranks(&sections(&["wanted"]), &hits), [None]);
}

#[test]
fn sections_and_documents_expected_together_are_each_ranked() {
    let hits = [
        whole(1, "article", 0.2),
        hit(2, "wanted", 0.9),
        whole(3, "unrelated", 0.5),
    ];
    let mut expected = sections(&["wanted"]);
    expected.extend(documents(&["article", "unfound"]));
    assert_eq!(ranks(&expected, &hits), [Some(1), Some(3), None]);
}

#[test]
fn a_route_that_found_a_document_expected_whole_did_not_miss_it() {
    let only_bm25 = Hit {
        routes: &["bm25"],
        ..whole(7, "article", 0.1)
    };
    let mut hits: Vec<Hit> = (1..=6)
        .map(|n| hit(n, "other", 1.0 / f64::from(n)))
        .collect();
    hits.push(only_bm25);
    let failures = judge(
        &question("misranked", true),
        &documents(&["article"]),
        &bundle(&hits),
        0,
    )
    .failures;
    let expected = Failure {
        cutoff: 5,
        class: FailureClass::Misranked,
        missed_by: vec!["dense".to_owned()],
        unavailable: BTreeMap::new(),
    };
    assert_eq!(failures, [expected]);
}
