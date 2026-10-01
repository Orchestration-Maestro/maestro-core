use super::super::spans::{SeedSpan, union_seed_spans};
use crate::search::Route;
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

fn seed(
    chunk_id: &str,
    revision_id: &str,
    start: usize,
    end: usize,
    input_position: usize,
) -> SeedSpan {
    SeedSpan {
        chunk_id: chunk_id.to_owned(),
        revision_id: revision_id.to_owned(),
        section_id: None,
        span: Span { start, end },
        input_position,
        score: None,
        routes: BTreeSet::from([Route::Dense]),
    }
}

#[test]
fn overlapping_adjacent_and_contained_seeds_keep_every_chunk_reference() {
    let unions = union_seed_spans(vec![
        seed("c2", "rev-a", 1450, 2050, 1),
        seed("c1", "rev-a", 1000, 1600, 0),
        seed("c3", "rev-a", 1100, 1200, 2),
        seed("c4", "rev-a", 2050, 2100, 3),
    ])
    .unwrap();

    assert_eq!(unions.len(), 1);
    assert_eq!(
        unions[0].span,
        Span {
            start: 1000,
            end: 2100
        }
    );
    let chunk_ids: Vec<_> = unions[0]
        .seeds
        .iter()
        .map(|seed| seed.chunk_id.as_str())
        .collect();
    assert_eq!(chunk_ids, ["c1", "c3", "c2", "c4"]);
}

#[test]
fn positive_gaps_and_revision_boundaries_remain_separate() {
    let unions = union_seed_spans(vec![
        seed("later", "rev-a", 20, 30, 1),
        seed("first", "rev-a", 10, 20, 0),
        seed("other-revision", "rev-b", 10, 30, 2),
        seed("gap", "rev-a", 31, 35, 3),
    ])
    .unwrap();

    let actual: Vec<_> = unions
        .iter()
        .map(|union| (union.revision_id.as_str(), union.span.start, union.span.end))
        .collect();
    assert_eq!(
        actual,
        [("rev-a", 10, 30), ("rev-a", 31, 35), ("rev-b", 10, 30)]
    );
}

#[test]
fn empty_or_reversed_seed_spans_are_rejected() {
    for span in [Span { start: 4, end: 4 }, Span { start: 5, end: 4 }] {
        let mut invalid = seed("chunk", "rev-a", span.start, span.end, 0);
        invalid.span = span;
        assert!(union_seed_spans(vec![invalid]).is_err());
    }
}
