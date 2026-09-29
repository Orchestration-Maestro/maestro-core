//! Each part's best passage joins the whole question's ranking within the
//! fusion pool's bound.

use super::rerank::candidate;
use crate::search::{Ranked, part_search::add_bests};
use std::collections::HashSet;

/// A ranked candidate `id`.
fn ranked(id: &str) -> Ranked {
    Ranked {
        candidate: candidate(id, 0.01, id),
        score: Some(0.5),
    }
}

/// The chunk IDs of `ranked`.
fn ids(ranked: &[Ranked]) -> Vec<String> {
    ranked
        .iter()
        .map(|ranked| ranked.candidate.fused.chunk_id.clone())
        .collect()
}

#[test]
fn a_best_is_appended_once_after_the_whole_ranking() {
    let mut list = vec![ranked("a"), ranked("b")];
    let mut order = ids(&list);
    add_bests(
        &mut list,
        &mut order,
        vec![ranked("b"), ranked("c")],
        &HashSet::new(),
    );
    assert_eq!(ids(&list), ["a", "b", "c"]);
    assert_eq!(order, ["a", "b", "c"]);
}

#[test]
fn at_the_pool_bound_a_best_displaces_the_lowest_unreserved_candidate() {
    let mut list = (0..120)
        .map(|index| ranked(&format!("c{index}")))
        .collect::<Vec<_>>();
    let mut order = ids(&list);
    let reserved = ["c0", "c119", "p1", "p2"]
        .map(str::to_owned)
        .into_iter()
        .collect::<HashSet<_>>();
    add_bests(
        &mut list,
        &mut order,
        vec![ranked("p1"), ranked("p2")],
        &reserved,
    );
    let kept = ids(&list);
    assert_eq!(kept.len(), 120);
    assert_eq!(order, kept);
    assert_eq!(&kept[..1], ["c0"]);
    assert_eq!(&kept[116..], ["c116", "c119", "p1", "p2"]);
    assert!(!kept.contains(&"c117".to_owned()));
    assert!(!kept.contains(&"c118".to_owned()));
}
