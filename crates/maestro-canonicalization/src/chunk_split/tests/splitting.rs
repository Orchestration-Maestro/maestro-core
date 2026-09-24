//! Splitting, characterized on small documents: where oversized units and rows are cut.
use super::*;

/// Each chunk's prepared input under the fake counter.
fn prepared(markdown: &str) -> Vec<String> {
    structural_chunks(markdown)
        .unwrap()
        .1
        .into_iter()
        .map(|c| c.prepared_input)
        .collect()
}

/// The pieces of a document's longest unit: start, end and how each was cut.
fn cuts(markdown: &str) -> Vec<(usize, usize, SplitKind)> {
    let (mapped, chunks) = structural_chunks(markdown).unwrap();
    let longest = (0..mapped.units.len())
        .max_by_key(|&i| mapped.units[i].text.len())
        .unwrap();
    chunks
        .iter()
        .flat_map(|c| &c.fragments)
        .filter(|f| f.contribution.unit_index == longest)
        .map(|f| {
            (
                f.contribution.range.start,
                f.contribution.range.end,
                f.split,
            )
        })
        .collect()
}

#[test]
fn a_wide_row_splits_by_column_and_its_pieces_pack_along_the_row() {
    let (left, middle, right) = ("x".repeat(300), "y".repeat(300), "z".repeat(300));
    assert_eq!(
        prepared(&format!(
            "| a | b | c |\n|---|---|---|\n| {left} | {middle} | {right} |\n"
        )),
        [
            "a\tb\tc".to_owned(),
            format!("a\tb\n\n{left}\t{middle}"),
            format!("c\n\n{right}")
        ]
    );
    // A cell of two units is one piece of the row before it splits in turn.
    let (plain, struck) = ("A".repeat(390), "B".repeat(390));
    assert_eq!(
        prepared(&format!(
            "| a | b |\n|---|---|\n| s | {plain} ~~{struck}~~ |\n"
        )),
        [
            "a\tb".to_owned(),
            "a\n\ns".to_owned(),
            format!("b\n\n{plain} "),
            format!("b\n\n~~{struck}~~")
        ]
    );
    // Tables never share a chunk, even with the same columns.
    assert_eq!(
        prepared("| a |\n|---|\n| b |\n\n| c |\n|---|\n| d |\n"),
        ["a\nb", "c\nd"]
    );
}

#[test]
fn oversized_units_cut_at_sentences_code_lines_then_whitespace() {
    use SplitKind::{CellFragment, CodeLine, CodeLineFragment, Scalar, Sentence, Whitespace};
    // The cuts as made today, short last pieces included.
    let words = format!("{}\n", "word ".repeat(300).trim_end());
    assert_eq!(
        cuts(&words),
        [
            (0, 365, Whitespace),
            (365, 925, Whitespace),
            (925, 1490, Whitespace),
            (1490, 1495, Whitespace),
            (1495, 1499, Scalar)
        ]
    );
    let sentences = format!("{}\n", "Sentence one here. ".repeat(80).trim_end());
    assert_eq!(
        cuts(&sentences),
        [
            (0, 361, Sentence),
            (361, 931, Sentence),
            (931, 1501, Sentence),
            (1501, 1510, Whitespace),
            (1510, 1514, Whitespace),
            (1514, 1519, Scalar)
        ]
    );
    let lines = format!(
        "```\n{}```\n",
        "let value = 1; // a line of code\n".repeat(40)
    );
    assert_eq!(
        cuts(&lines),
        [
            (0, 627, CodeLine),
            (627, 1287, CodeLine),
            (1287, 1320, CodeLine)
        ]
    );
    let line = format!("```\n{}\n```\n", "x".repeat(900));
    assert_eq!(
        cuts(&line),
        [(0, 450, CodeLineFragment), (450, 901, CodeLineFragment)]
    );
    let cell = format!(
        "| a |\n|---|\n| {} |\n",
        "cell words ".repeat(90).trim_end()
    );
    assert_eq!(
        cuts(&cell),
        [
            (0, 484, CellFragment),
            (484, 979, CellFragment),
            (979, 984, CellFragment),
            (984, 989, CellFragment)
        ]
    );
}

#[test]
fn every_piece_counts_at_most_the_maximum_when_counted_again() {
    let markdown = format!("{}\n", "word ".repeat(300).trim_end());
    let doc = canonicalize(CanonicalizeInput::new(&markdown, "recount")).unwrap();
    let mapped = map_document(&doc, &markdown).unwrap();
    let mut at_most = |s: &str| {
        Ok(if s.len() <= 600 {
            MAX_TOKENS
        } else {
            MAX_TOKENS + 1
        })
    };
    let chunks = build_drafts(&doc, &markdown, &mapped, &mut at_most).unwrap();
    assert!(chunks.iter().all(|c| c.token_count == MAX_TOKENS));
    // A counter that grows when it sees the same text again: the final recount refuses the piece.
    let mut seen = BTreeSet::new();
    let mut unstable = |s: &str| {
        Ok(if seen.insert(s.to_owned()) {
            s.chars().count() + 2
        } else {
            MAX_TOKENS + 100
        })
    };
    assert!(build_drafts(&doc, &markdown, &mapped, &mut unstable).is_err());
}
