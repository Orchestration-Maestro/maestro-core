//! Oversized units: a unit that cannot fit the hard maximum with its mandatory context refuses its
//! document, naming the unit, its block and the block's span in the original, never its text.
use super::*;
use std::collections::BTreeMap;

/// The refusal the chunker gives `markdown` under the fake counter.
fn refusal(markdown: &str) -> String {
    structural_chunks(markdown).unwrap_err().0
}

/// The refusal that names the unit whose text is `text` in `markdown`'s mapping.
fn naming(markdown: &str, text: &str) -> String {
    let doc = canonicalize(CanonicalizeInput::new(markdown, "structural-test")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let unit = mapped.units.iter().find(|unit| unit.text == text).unwrap();
    let block = doc
        .blocks
        .iter()
        .find(|block| block.block_id == unit.block_id)
        .unwrap();
    let start = block
        .source_spans
        .iter()
        .map(|span| span.start)
        .min()
        .unwrap();
    let end = block
        .source_spans
        .iter()
        .map(|span| span.end)
        .max()
        .unwrap();
    format!(
        "the unit {} of block {} at bytes [{start}, {end}) does not fit in 700 tokens with its \
         context",
        unit.unit_id, unit.block_id
    )
}

#[test]
fn a_unit_whose_mandatory_context_leaves_no_room_is_named_with_its_block_and_span() {
    // A heading, a parent item and a table header of 800 characters, which the fake counts as 802
    // tokens, leave no room for the body, item and cell below them.
    for (markdown, body) in [
        (format!("# {}\n\nbody\n", "H".repeat(800)), "body"),
        (format!("- {}\n  - child\n", "P".repeat(800)), "child"),
        (
            format!("| {} |\n|---|\n| value |\n", "H".repeat(800)),
            "value",
        ),
    ] {
        assert_eq!(refusal(&markdown), naming(&markdown, body));
    }
    // The span is the block's: here the paragraph `body` with its line end, after the heading line
    // and a blank line.
    assert!(refusal(&format!("# {}\n\nbody\n", "H".repeat(800))).contains("at bytes [804, 809)"));
}

#[test]
fn a_piece_counted_over_the_maximum_once_chosen_names_its_unit() {
    // The counter gives 700 tokens the first time it sees an input and 701 every time after, so
    // the prefix it chose to fit no longer fits when its chunk is prepared.
    let markdown = format!("{}\n", "x".repeat(1000));
    let doc = canonicalize(CanonicalizeInput::new(&markdown, "structural-test")).unwrap();
    let mapped = map_document(&doc, &markdown).unwrap();
    let mut seen = BTreeMap::new();
    let mut counter = |text: &str| {
        let times: &mut usize = seen.entry(text.to_owned()).or_default();
        *times += 1;
        Ok::<_, Error>(if text.chars().count() > 690 {
            800
        } else if *times == 1 {
            700
        } else {
            701
        })
    };
    let refused = build_drafts(&doc, &markdown, &mapped, &mut counter).unwrap_err();
    assert_eq!(refused.0, naming(&markdown, &"x".repeat(1000)));
}
