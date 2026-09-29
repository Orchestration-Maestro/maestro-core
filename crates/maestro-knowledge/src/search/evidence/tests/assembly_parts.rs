//! Assembly reserves the best passage of each question part the search
//! recorded, and assembles a question that was not split byte for byte as
//! before.

use super::super::{EvidenceCounter, assemble::assemble_blocking};
use super::support::{Fixture, control, evidence_input, fixture};
use crate::search::{EvidenceInput, PartRecord, PartsRecord, Unsplit};
use maestro_kernel::evidence::{Bundle, RouteStatus};

/// Three short sources, ranked in order.
fn sources() -> Fixture {
    fixture(&[
        ("first.md", "# First\n\nThe first source passage.\n"),
        ("second.md", "# Second\n\nThe second source passage.\n"),
        ("third.md", "# Third\n\nThe third source passage.\n"),
    ])
}

/// The bundle of `input`, counted in UTF-8 bytes.
fn assemble(fixture: &Fixture, input: &EvidenceInput) -> Bundle {
    assemble_blocking(
        &fixture.database,
        input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap()
}

/// A part whose best passage is the chunk `best`.
fn part(best: &str) -> PartRecord {
    PartRecord {
        text: "a part".to_owned(),
        bridge: String::new(),
        bridge_status: None,
        status: RouteStatus::Ok,
        best: Some(best.to_owned()),
    }
}

#[test]
fn the_whole_best_passage_then_each_part_best_passage_take_the_slots() {
    let fixture = sources();
    let mut input = evidence_input(&fixture, "Which source?");
    input.budget.k = 2;
    let chunk = |index: usize| input.ranked[index].candidate.fused.chunk_id.clone();
    let (first, second, last) = (chunk(0), chunk(1), chunk(input.ranked.len() - 1));
    let whole = assemble(&fixture, &input);
    let chunks = |bundle: &Bundle| {
        let mut chunks = bundle
            .trace
            .iter()
            .flat_map(|trace| trace.chunk_ids.clone())
            .collect::<Vec<_>>();
        chunks.sort();
        chunks
    };
    let mut expected = vec![first.clone(), second];
    expected.sort();
    assert_eq!(chunks(&whole), expected);
    input.observations.question_parts = Some(PartsRecord {
        whole: Some(first.clone()),
        parts: vec![part(&last)],
        unsplit: None,
    });
    let mut expected = vec![first, last.clone()];
    expected.sort();
    assert_eq!(chunks(&assemble(&fixture, &input)), expected);
    input.budget.k = 1;
    let parts = assemble(&fixture, &input);
    assert_eq!(parts.passages.len(), 1);
    assert_ne!(parts.trace[0].chunk_ids, [last]);
}

#[test]
fn a_question_that_was_not_split_assembles_the_same_bytes() {
    let fixture = sources();
    let mut input = evidence_input(&fixture, "Which source?");
    let before = serde_json::to_vec(&assemble(&fixture, &input)).unwrap();
    input.observations.question_parts = Some(PartsRecord {
        whole: None,
        parts: Vec::new(),
        unsplit: Some(Unsplit::NoRelation),
    });
    assert_eq!(
        serde_json::to_vec(&assemble(&fixture, &input)).unwrap(),
        before
    );
}
