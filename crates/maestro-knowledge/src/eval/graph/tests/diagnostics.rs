//! Observed proof losses and citation presence are distinct from support.

use crate::eval::graph::{
    CheckedItem, ConclusionObservation, DiagnosticRatio, Located, ProofObservation, QuestionKind,
    score_graph,
};

use std::{array::from_fn, slice::from_ref};

/// Synthetic two-anchor proof.
fn item() -> CheckedItem {
    CheckedItem {
        id: "q-1".to_owned(),
        family: "f-1".to_owned(),
        kind: QuestionKind::MultiHop,
        proofs: vec![vec![anchor(0), anchor(10)]],
    }
}
/// Exact synthetic source coordinate.
fn anchor(start: usize) -> Located {
    Located {
        revision_id: "revision-1".to_owned(),
        span: [start, start + 5],
    }
}
/// Numerator, denominator and unobserved count.
fn ratio(count: usize, of: usize, unobserved: usize) -> DiagnosticRatio {
    DiagnosticRatio {
        count,
        of,
        unobserved,
    }
}

#[test]
fn candidate_pre_fusion_post_packing_and_wire_losses_have_explicit_denominators() {
    for loss_at in 0..4 {
        let row = ProofObservation {
            id: "q-1".to_owned(),
            stages: from_fn(|stage| {
                Some(if stage < loss_at {
                    vec![anchor(0), anchor(10)]
                } else {
                    vec![anchor(0)]
                })
            }),
            conclusions: None,
        };
        let score = score_graph(&[item()], &[row]).unwrap();
        for stage in 0..4 {
            assert_eq!(
                score.stages[stage].complete,
                ratio(usize::from(stage < loss_at), 1, 0)
            );
            assert_eq!(
                score.stages[stage].attrition,
                ratio(
                    usize::from(stage == loss_at),
                    usize::from(stage <= loss_at),
                    0
                )
            );
        }
        assert_eq!((score.final_wire.complete, score.final_wire.of), (0, 1));
    }
}

#[test]
fn unknown_stages_are_not_silently_kept_or_lost() {
    let row = ProofObservation {
        id: "q-1".to_owned(),
        stages: [
            Some(vec![anchor(0), anchor(10)]),
            None,
            Some(vec![anchor(0)]),
            None,
        ],
        conclusions: None,
    };
    let score = score_graph(&[item()], &[row]).unwrap();
    assert_eq!(score.stages[1].complete, ratio(0, 0, 1));
    assert_eq!(score.stages[2].complete, ratio(0, 1, 0));
    assert_eq!(score.stages[2].attrition, ratio(0, 0, 1));
    assert_eq!(score.stages[3].attrition, ratio(0, 0, 1));
    assert_eq!(score.conclusions.unobserved_items, 1);
}

#[test]
fn present_citations_do_not_prove_supported_conclusions() {
    let row = ProofObservation {
        id: "q-1".to_owned(),
        stages: [None, None, None, Some(vec![anchor(0), anchor(10)])],
        conclusions: Some(vec![
            ConclusionObservation {
                citations: vec![anchor(0), anchor(10)],
                supported: true,
            },
            ConclusionObservation {
                citations: vec![anchor(0)],
                supported: true,
            },
            ConclusionObservation {
                citations: vec![anchor(0), anchor(10)],
                supported: false,
            },
            ConclusionObservation {
                citations: vec![],
                supported: true,
            },
        ]),
    };
    let score = score_graph(&[item()], &[row]).unwrap();
    assert_eq!(score.conclusions.citation_presence, ratio(3, 4, 0));
    assert_eq!(score.conclusions.citation_support, ratio(2, 4, 0));
    assert_eq!(score.conclusions.unsupported, ratio(3, 4, 0));
}

#[test]
fn citations_and_wire_must_share_one_complete_alternative() {
    let mut item = item();
    item.proofs = vec![vec![anchor(0)], vec![anchor(10)]];
    let row = ProofObservation {
        id: "q-1".to_owned(),
        stages: [None, None, None, Some(vec![anchor(0)])],
        conclusions: Some(vec![ConclusionObservation {
            citations: vec![anchor(10)],
            supported: true,
        }]),
    };
    let score = score_graph(from_ref(&item), from_ref(&row)).unwrap();
    assert_eq!(score.conclusions.citation_support, ratio(0, 1, 0));
    assert_eq!(score.conclusions.unsupported, ratio(1, 1, 0));
    let mut unknown = row;
    unknown.stages[3] = None;
    let score = score_graph(&[item], &[unknown]).unwrap();
    assert_eq!(score.conclusions.citation_presence, ratio(1, 1, 0));
    assert_eq!(score.conclusions.citation_support, ratio(0, 0, 1));
    assert_eq!(score.conclusions.unsupported, ratio(0, 0, 1));
}

#[test]
fn inconsistent_later_proofs_and_invalid_observation_ids_refuse_safely() {
    let row = ProofObservation {
        id: "q-1".to_owned(),
        stages: [
            Some(vec![anchor(0)]),
            None,
            None,
            Some(vec![anchor(0), anchor(10)]),
        ],
        conclusions: None,
    };
    let error = score_graph(&[item()], from_ref(&row)).unwrap_err();
    assert_eq!(error.code, "graph_proof_inconsistent");
    assert_eq!(error.item.as_deref(), Some("q-1"));
    let mut row = row;
    row.stages = [None, None, None, None];
    assert_eq!(
        score_graph(&[item()], &[row.clone(), row.clone()])
            .unwrap_err()
            .code,
        "graph_observation_duplicate"
    );
    row.id = "unknown".to_owned();
    assert_eq!(
        score_graph(&[item()], from_ref(&row)).unwrap_err().code,
        "graph_observation_unknown"
    );
    row.id = "private question with spaces".to_owned();
    let error = score_graph(&[item()], &[row]).unwrap_err();
    assert_eq!(error.code, "graph_observation_unsafe_id");
    assert!(error.item.is_none());
}
