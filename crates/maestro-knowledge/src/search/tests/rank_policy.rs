use super::{rerank::candidate, section_prior::apply};
use crate::search::{
    Ranked, SectionClassSet, SectionPrior,
    rank_policy::{blend, cap_demotion},
};
use std::{collections::BTreeSet, num::NonZeroU32};

fn ids(ranked: &[Ranked]) -> Vec<&str> {
    ranked
        .iter()
        .map(|item| item.candidate.fused.chunk_id.as_str())
        .collect()
}

fn reranked(order: &[&str]) -> Vec<Ranked> {
    order
        .iter()
        .map(|id| Ranked {
            candidate: candidate(id, 1.0, id),
            score: Some(1.0),
        })
        .collect()
}

fn rrf(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).unwrap()
}

#[test]
fn blend_uses_the_configured_rank_fusion_constant() {
    let fused = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let mut ranked = reranked(&["c", "b", "a"]);
    blend(&mut ranked, &fused, Some(0.4), rrf(1));
    assert_eq!(ids(&ranked), ["c", "a", "b"]);
    let mut ranked = reranked(&["c", "b", "a"]);
    blend(&mut ranked, &fused, Some(0.4), rrf(60));
    assert_eq!(ids(&ranked), ["c", "b", "a"]);
}

#[test]
fn equal_blended_positions_keep_the_fused_order() {
    let mut ranked = reranked(&["x", "y"]);
    blend(
        &mut ranked,
        &["y".to_owned(), "x".to_owned()],
        Some(0.5),
        rrf(60),
    );
    assert_eq!(ids(&ranked), ["y", "x"]);
}

#[test]
fn demotion_cap_bounds_final_positions() {
    let mut ranked = (0..12)
        .rev()
        .map(|index| Ranked {
            candidate: candidate(&index.to_string(), f64::from(12 - index), "text"),
            score: Some(f64::from(index)),
        })
        .collect::<Vec<_>>();
    cap_demotion(
        &mut ranked,
        &(0..12).map(|index| index.to_string()).collect::<Vec<_>>(),
        Some(0),
    );
    assert_eq!(
        ids(&ranked)[..10],
        (0..10).map(|index| index.to_string()).collect::<Vec<_>>()
    );
}

#[test]
fn a_cap_leaves_items_within_their_bound_in_place() {
    let fused = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let mut ranked = reranked(&["b", "a", "c"]);
    cap_demotion(&mut ranked, &fused, Some(1));
    assert_eq!(ids(&ranked), ["b", "a", "c"]);
    cap_demotion(&mut ranked, &fused, None);
    assert_eq!(ids(&ranked), ["b", "a", "c"]);
}

#[test]
fn blend_endpoints_preserve_original_ties_and_raw_scores() {
    let original = vec![
        Ranked {
            candidate: candidate("second", 1.0, "b"),
            score: Some(99.0),
        },
        Ranked {
            candidate: candidate("first", 1.0, "a"),
            score: Some(-99.0),
        },
    ];
    let fused = vec!["first".to_owned(), "second".to_owned()];
    for weight in [None, Some(0.0)] {
        let mut ranked = original.clone();
        blend(&mut ranked, &fused, weight, rrf(60));
        assert_eq!(ranked, original);
    }
    let mut ranked = original.clone();
    blend(&mut ranked, &fused, Some(1.0), rrf(60));
    assert_eq!(ranked, original.into_iter().rev().collect::<Vec<_>>());
}

#[test]
fn section_classes_exempt_explicit_queries_and_never_match_word_substrings() {
    for (name, path, explicit) in [
        (
            "changelog",
            "guide / Change Log",
            "what changes are in the changelog",
        ),
        (
            "release_notes",
            "guide/release_notes/new",
            "show release notes",
        ),
        (
            "conversion",
            "guide/Conversion/task",
            "how to convert a task",
        ),
    ] {
        let mut classes = SectionClassSet::default();
        assert!(classes.insert(name));
        assert!(!classes.insert("unknown"));
        let prior = SectionPrior::Soft {
            weight: 0.5,
            classes,
        };
        assert!(prior.penalizes("run a task", path));
        assert!(!prior.penalizes(explicit, path));
        assert!(!prior.penalizes("run a task", "guide / ordinary task"));
        assert!(!prior.penalizes("run a task", "guide / preconversion"));
    }
}

#[test]
fn section_prior_is_soft_stable_and_keeps_scores() {
    let mut ranked = vec![
        Ranked {
            candidate: candidate("generic", 0.3, "a"),
            score: Some(9.0),
        },
        Ranked {
            candidate: candidate("procedure", 0.2, "b"),
            score: Some(2.0),
        },
        Ranked {
            candidate: candidate("other", 0.1, "c"),
            score: None,
        },
    ];
    let original = ranked.clone();
    let penalized = BTreeSet::from(["generic".to_owned()]);
    apply(SectionPrior::Off, &mut ranked, &penalized);
    assert_eq!(ranked, original);
    apply(
        SectionPrior::Soft {
            weight: 0.75,
            classes: SectionClassSet::default(),
        },
        &mut ranked,
        &penalized,
    );
    assert_eq!(ids(&ranked), ["procedure", "other", "generic"]);
    assert_eq!(ranked[2], original[0]);
}
