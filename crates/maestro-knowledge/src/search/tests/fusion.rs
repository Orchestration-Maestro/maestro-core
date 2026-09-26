use crate::search::{Fused, Hit, Route, RouteList, fuse};
use std::collections::BTreeMap;
use std::num::NonZeroU32;

fn hit(chunk_id: &str, score: f64) -> Hit {
    Hit {
        chunk_id: chunk_id.to_owned(),
        score,
    }
}

fn list(route: Route, chunk_ids: &[&str]) -> RouteList {
    RouteList {
        route,
        hits: chunk_ids
            .iter()
            .map(|chunk_id| hit(chunk_id, 0.0))
            .collect(),
    }
}

fn ranks(entries: &[(Route, u32)]) -> BTreeMap<Route, NonZeroU32> {
    entries
        .iter()
        .map(|(route, rank)| (*route, NonZeroU32::new(*rank).unwrap()))
        .collect()
}

fn assert_score(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-15, "{actual} != {expected}");
}

fn assert_fused(actual: &Fused, chunk_id: &str, score: f64, expected_ranks: &[(Route, u32)]) {
    assert_eq!(actual.chunk_id, chunk_id);
    assert_score(actual.score, score);
    assert_eq!(actual.ranks, ranks(expected_ranks));
}

#[test]
fn two_routes_use_reciprocal_ranks_and_break_ties_by_chunk_id() {
    let fused = fuse(
        &[
            RouteList {
                route: Route::Dense,
                hits: vec![hit("alpha", 1_000.0), hit("beta", -1_000.0)],
            },
            RouteList {
                route: Route::Lexical,
                hits: vec![hit("beta", f64::NAN), hit("alpha", 0.0)],
            },
        ],
        10,
    );

    // Both scores are 1/61 + 1/62 = 123/3782; IDs break the tie.
    assert_eq!(fused.len(), 2);
    assert_fused(
        &fused[0],
        "alpha",
        123.0 / 3782.0,
        &[(Route::Dense, 1), (Route::Lexical, 2)],
    );
    assert_fused(
        &fused[1],
        "beta",
        123.0 / 3782.0,
        &[(Route::Dense, 2), (Route::Lexical, 1)],
    );
}

#[test]
fn three_routes_sum_only_the_ranks_that_hold_each_chunk() {
    let fused = fuse(
        &[
            list(Route::Dense, &["alpha", "beta"]),
            list(Route::Lexical, &["beta", "alpha", "gamma"]),
            list(Route::Structured, &["gamma", "alpha"]),
        ],
        10,
    );

    // alpha = 1/61 + 1/62 + 1/62 = 92/1891.
    // beta = 1/62 + 1/61 = 123/3782.
    // gamma = 1/63 + 1/61 = 124/3843.
    assert_eq!(fused.len(), 3);
    assert_fused(
        &fused[0],
        "alpha",
        92.0 / 1891.0,
        &[
            (Route::Dense, 1),
            (Route::Lexical, 2),
            (Route::Structured, 2),
        ],
    );
    assert_fused(
        &fused[1],
        "beta",
        123.0 / 3782.0,
        &[(Route::Dense, 2), (Route::Lexical, 1)],
    );
    assert_fused(
        &fused[2],
        "gamma",
        124.0 / 3843.0,
        &[(Route::Lexical, 3), (Route::Structured, 1)],
    );
}

#[test]
fn two_routes_order_distinct_scores_by_their_reciprocal_rank_sum() {
    let fused = fuse(
        &[
            list(Route::Dense, &["alpha", "beta", "gamma"]),
            list(Route::Lexical, &["gamma", "alpha"]),
        ],
        10,
    );

    // alpha = 1/61 + 1/62 = 123/3782; gamma = 1/63 + 1/61 = 124/3843.
    // beta = 1/62. Thus alpha > gamma > beta.
    assert_eq!(fused.len(), 3);
    assert_fused(
        &fused[0],
        "alpha",
        123.0 / 3782.0,
        &[(Route::Dense, 1), (Route::Lexical, 2)],
    );
    assert_fused(
        &fused[1],
        "gamma",
        124.0 / 3843.0,
        &[(Route::Dense, 3), (Route::Lexical, 1)],
    );
    assert_fused(&fused[2], "beta", 1.0 / 62.0, &[(Route::Dense, 2)]);
}

#[test]
fn ranks_are_one_based_and_change_the_order_from_zero_based_rrf() {
    let mut dense = Vec::new();
    for rank in 1..=26 {
        let chunk_id = match rank {
            1 => "alpha".to_owned(),
            26 => "beta".to_owned(),
            _ => format!("dense-{rank:03}"),
        };
        dense.push(hit(&chunk_id, f64::from(rank)));
    }
    let mut lexical = (1..=146)
        .map(|rank| hit(&format!("zz-lexical-{rank:03}"), f64::from(rank)))
        .collect::<Vec<_>>();
    lexical[145] = hit("beta", 146.0);

    let fused = fuse(
        &[
            RouteList {
                route: Route::Dense,
                hits: dense,
            },
            RouteList {
                route: Route::Lexical,
                hits: lexical,
            },
        ],
        2,
    );

    // One-based beta: 1/86 + 1/206 > alpha's 1/61. With K + rank - 1,
    // beta's 1/85 + 1/205 < alpha's 1/60, so the order would reverse.
    assert_eq!(fused.len(), 2);
    assert_fused(
        &fused[0],
        "beta",
        1.0 / 86.0 + 1.0 / 206.0,
        &[(Route::Dense, 26), (Route::Lexical, 146)],
    );
    assert_fused(&fused[1], "alpha", 1.0 / 61.0, &[(Route::Dense, 1)]);
}

#[test]
fn duplicates_keep_their_first_position_without_shifting_later_ranks() {
    let fused = fuse(
        &[RouteList {
            route: Route::Identifier,
            hits: vec![hit("repeat", 1.0), hit("repeat", 2.0), hit("later", 3.0)],
        }],
        10,
    );

    assert_eq!(fused.len(), 2);
    assert_fused(&fused[0], "repeat", 1.0 / 61.0, &[(Route::Identifier, 1)]);
    assert_fused(&fused[1], "later", 1.0 / 62.0, &[(Route::Identifier, 2)]);
}

#[test]
fn permuting_distinct_route_lists_does_not_change_the_result() {
    let lists = [
        list(Route::Dense, &["a", "b", "shared"]),
        list(Route::Lexical, &["shared", "b", "c"]),
        list(Route::Structured, &["c", "a", "shared"]),
    ];
    let expected = fuse(&lists, 10);

    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0], [2, 1, 0]] {
        let permutation = order.map(|index| lists[index].clone());
        assert_eq!(fuse(&permutation, 10), expected);
    }
}

#[test]
fn empty_inputs_empty_routes_and_zero_limit_return_no_results() {
    assert!(fuse(&[], 10).is_empty());
    assert!(fuse(&[list(Route::Dense, &[])], 10).is_empty());
    assert!(fuse(&[list(Route::Dense, &["a"])], 0).is_empty());
}

#[test]
fn route_names_are_stable() {
    assert_eq!(Route::Dense.name(), "dense");
    assert_eq!(Route::Lexical.name(), "lexical");
    assert_eq!(Route::Identifier.name(), "identifier");
    assert_eq!(Route::Structured.name(), "structured");
}

#[test]
#[should_panic(expected = "one list per route is expected")]
fn debug_asserts_that_each_route_has_one_list() {
    let _unused = fuse(
        &[list(Route::Dense, &["a"]), list(Route::Dense, &["b"])],
        10,
    );
}
