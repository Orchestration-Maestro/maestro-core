//! The soft section prior: strict demotion and precise, bilingual exemptions.

use super::rerank::candidate;
use crate::search::{Ranked, SectionClassSet, SectionPrior};
use std::collections::BTreeSet;

fn ranked(ids: &[&str]) -> Vec<Ranked> {
    ids.iter()
        .map(|id| Ranked {
            candidate: candidate(id, 1.0, id),
            score: Some(1.0),
        })
        .collect()
}

fn order(ranked: &[Ranked]) -> Vec<&str> {
    ranked
        .iter()
        .map(|item| item.candidate.fused.chunk_id.as_str())
        .collect()
}

fn soft(weight: f32, names: &[&str]) -> SectionPrior {
    let mut classes = SectionClassSet::default();
    for name in names {
        assert!(classes.insert(name));
    }
    SectionPrior::Soft { weight, classes }
}

#[test]
fn half_weight_moves_a_penalized_first_below_the_second() {
    let mut items = ranked(&["generic", "procedure", "other"]);
    let penalized = BTreeSet::from(["generic".to_owned()]);
    soft(0.5, &[]).apply(&mut items, &penalized);
    assert_eq!(order(&items), ["procedure", "generic", "other"]);
}

#[test]
fn a_penalized_rank_moves_to_its_rank_over_one_minus_weight() {
    let mut items = ranked(&["0", "1", "2", "3", "4"]);
    let penalized = BTreeSet::from(["0".to_owned()]);
    soft(0.75, &[]).apply(&mut items, &penalized);
    assert_eq!(order(&items), ["1", "2", "3", "0", "4"]);
}

#[test]
fn generic_actions_never_exempt_a_class_that_shares_their_word() {
    let prior = soft(0.5, &["changelog", "release_notes", "conversion"]);
    assert!(prior.penalizes("release a job", "guide/release_notes/new"));
    assert!(prior.penalizes("how to save your changes", "guide / Change Log"));
    assert!(prior.penalizes("run a task", "guide/Conversion/task"));
}

#[test]
fn explicit_french_questions_are_exempt() {
    let prior = soft(0.5, &["changelog", "release_notes", "conversion"]);
    for (query, path) in [
        (
            "quelles sont les notes de version",
            "guide/release_notes/new",
        ),
        (
            "afficher le journal des modifications",
            "guide / Change Log",
        ),
        ("comment convertir un job", "guide/Conversion/task"),
    ] {
        assert!(!prior.penalizes(query, path), "{query}");
    }
}

#[test]
fn a_question_naming_any_configured_class_switches_the_prior_off() {
    let prior = soft(0.5, &["changelog", "conversion"]);
    assert!(!prior.penalizes("show the changelog", "guide/Conversion/task"));
    assert!(prior.penalizes("show the release notes", "guide/Conversion/task"));
}

#[test]
fn only_selected_classes_are_penalized_and_a_repeated_name_stays_selected() {
    let prior = soft(0.5, &["changelog", "changelog"]);
    assert!(prior.penalizes("run a task", "guide / Change Log"));
    assert!(!prior.penalizes("run a task", "guide/release_notes/new"));
}
