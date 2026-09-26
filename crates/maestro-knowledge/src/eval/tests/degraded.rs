//! A degraded search, one where a route or the reranker could not run, still
//! counts in every retrieval metric, but its latency is left out of the
//! percentiles, in a run and in a comparison, and the report counts it apart
//! (SC-S1-004).

use super::support::{ALL_RAN, bundle_with, close, hit, question, report_of, sections};
use crate::eval::{QuestionResult, compare, judge::judge, metric::measure};

/// The routes of a search where the dense route could not run.
const WITHOUT_DENSE: [(&str, Option<&str>); 3] = [
    ("bm25", None),
    ("dense", Some("no room for the embedder")),
    ("rerank", None),
];

/// The routes of a search where the reranker could not run.
const WITHOUT_RERANK: [(&str, Option<&str>); 3] = [
    ("bm25", None),
    ("dense", None),
    ("rerank", Some("no room for the reranker")),
];

/// The question `id`, whose expected section is found first, retrieved in
/// `latency_us` µs by `routes`.
fn found(id: &str, latency_us: u32, routes: &[(&str, Option<&str>)]) -> QuestionResult {
    judge(
        &question(id, true),
        &sections(&["wanted"]),
        &bundle_with(&[hit(1, "wanted", 0.9)], routes),
        latency_us,
    )
}

#[test]
fn a_search_is_degraded_when_a_route_or_the_reranker_could_not_run() {
    assert!(!found("whole", 1, &ALL_RAN).degraded);
    assert!(found("without-dense", 1, &WITHOUT_DENSE).degraded);
    assert!(found("without-rerank", 1, &WITHOUT_RERANK).degraded);
    let refused = judge(
        &question("refused", false),
        &[],
        &bundle_with(&[], &WITHOUT_DENSE),
        1,
    );
    assert!(refused.degraded);
}

#[test]
fn a_degraded_search_is_left_out_of_the_latency_percentiles_only() {
    // Six whole searches in 10 to 60 µs, then two degraded ones, far slower,
    // one of them missing its section.
    let mut results: Vec<QuestionResult> = (1..=6)
        .map(|n| found(&format!("whole-{n}"), n * 10, &ALL_RAN))
        .collect();
    results.push(found("without-dense", 900, &WITHOUT_DENSE));
    results.push(judge(
        &question("without-rerank", true),
        &sections(&["wanted"]),
        &bundle_with(&[hit(1, "other", 0.9)], &WITHOUT_RERANK),
        800,
    ));
    let metrics = measure(&results, 3);
    // By nearest rank over the six whole searches: the 3rd and the 6th.
    let p50 = metrics.latency_p50_us.unwrap();
    let p95 = metrics.latency_p95_us.unwrap();
    close(p50.value, 30.0);
    close(p95.value, 60.0);
    // No resample draws a degraded latency either.
    assert!(p50.high <= 60.0 && p95.high <= 60.0, "{p50:?} {p95:?}");
    // Both degraded searches count in recall: 7 of the 8 are found.
    close(metrics.recall_at_5.unwrap().value, 7.0 / 8.0);
}

#[test]
fn a_run_whose_every_search_was_degraded_has_no_latency() {
    let results = [
        found("without-dense", 10, &WITHOUT_DENSE),
        found("without-rerank", 20, &WITHOUT_RERANK),
    ];
    let metrics = measure(&results, 1);
    assert_eq!(metrics.latency_p50_us, None);
    assert_eq!(metrics.latency_p95_us, None);
    close(metrics.recall_at_5.unwrap().value, 1.0);
}

#[test]
fn a_comparison_leaves_each_runs_degraded_searches_out_of_its_latencies() {
    let baseline = report_of(vec![
        found("first", 10, &ALL_RAN),
        found("second", 20, &ALL_RAN),
    ]);
    let candidate = report_of(vec![
        found("first", 5, &ALL_RAN),
        found("second", 1000, &WITHOUT_DENSE),
    ]);
    assert_eq!(
        [baseline.degraded_searches, candidate.degraded_searches],
        [0, 1]
    );
    let differences = compare(&baseline, &candidate, 1).unwrap().differences;
    // The baseline's median and 95th percentile are 10 and 20 µs; the
    // candidate's, over its one whole search, 5 µs.
    close(differences.latency_p50_us.unwrap().value, -5.0);
    close(differences.latency_p95_us.unwrap().value, -15.0);
}
