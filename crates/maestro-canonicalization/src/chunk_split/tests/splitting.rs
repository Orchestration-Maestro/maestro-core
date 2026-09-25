//! Splitting, characterized on small documents: where oversized units and rows are cut.
use super::*;

/// Each chunk's prepared input under the fake counter.
fn prepared(markdown: &str) -> Vec<String> {
    structural_chunks(markdown)
        .unwrap()
        .1
        .into_iter()
        .map(|chunk| chunk.prepared_input)
        .collect()
}

/// The pieces of a document's longest unit: start, end and how each was cut.
fn cuts(markdown: &str) -> Vec<(usize, usize, SplitKind)> {
    let (mapped, chunks) = structural_chunks(markdown).unwrap();
    let longest = (0..mapped.units.len())
        .max_by_key(|&index| mapped.units[index].text.len())
        .unwrap();
    chunks
        .iter()
        .flat_map(|chunk| &chunk.fragments)
        .filter(|fragment| fragment.contribution.unit_index == longest)
        .map(|fragment| {
            (
                fragment.contribution.range.start,
                fragment.contribution.range.end,
                fragment.split,
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
    // Once the rest of a unit fits, it is one last piece, named like the cuts before it.
    let words = format!("{}\n", "word ".repeat(300).trim_end());
    assert_eq!(
        cuts(&words),
        [
            (0, 370, Whitespace),
            (370, 930, Whitespace),
            (930, 1499, Whitespace)
        ]
    );
    let unbroken = format!("{}\n", "c".repeat(800));
    assert_eq!(cuts(&unbroken), [(0, 400, Scalar), (400, 800, Scalar)]);
    // The one sentence end lies past the halved prefix, whose cut inside a word moves back to
    // the last whitespace.
    let late_end = format!("{}. End.\n", "word ".repeat(300).trim_end());
    assert_eq!(
        cuts(&late_end),
        [
            (0, 375, Whitespace),
            (375, 940, Whitespace),
            (940, 1505, Sentence)
        ]
    );
    let sentences = format!("{}\n", "Sentence one here. ".repeat(80).trim_end());
    assert_eq!(
        cuts(&sentences),
        [
            (0, 361, Sentence),
            (361, 931, Sentence),
            (931, 1519, Sentence)
        ]
    );
    let lines = format!(
        "```\n{}```\n",
        "let value = 1; // a line of code\n".repeat(40)
    );
    assert_eq!(cuts(&lines), [(0, 627, CodeLine), (627, 1320, CodeLine)]);
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
        [(0, 489, CellFragment), (489, 989, CellFragment)]
    );
}

#[test]
fn every_piece_counts_at_most_the_maximum_when_counted_again() {
    let markdown = format!("{}\n", "word ".repeat(300).trim_end());
    let doc = canonicalize(CanonicalizeInput::new(&markdown, "recount")).unwrap();
    let mapped = map_document(&doc, &markdown).unwrap();
    let mut at_most = |text: &str| {
        Ok(if text.len() <= 600 {
            MAX_TOKENS
        } else {
            MAX_TOKENS + 1
        })
    };
    let chunks = build_drafts(&doc, &markdown, &mapped, &mut at_most).unwrap();
    assert!(chunks.iter().all(|chunk| chunk.token_count == MAX_TOKENS));
    // A counter that grows when it sees the same text again: the final recount refuses the piece.
    let mut seen = BTreeSet::new();
    let mut unstable = |text: &str| {
        Ok(if seen.insert(text.to_owned()) {
            text.chars().count() + 2
        } else {
            MAX_TOKENS + 100
        })
    };
    assert!(build_drafts(&doc, &markdown, &mapped, &mut unstable).is_err());
}
