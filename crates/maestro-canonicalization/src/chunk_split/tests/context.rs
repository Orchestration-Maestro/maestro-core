//! Context, characterized on small documents: the exact prepared input of each chunk.
use super::*;
use crate::prepared_inputs::Contribution;
use crate::source_units::TextRange;

/// Each chunk under the fake counter.
fn drafted(markdown: &str) -> Vec<ChunkContent> {
    structural_chunks(markdown).unwrap().1
}

/// Each chunk's prepared input under the fake counter.
fn prepared(markdown: &str) -> Vec<String> {
    drafted(markdown)
        .into_iter()
        .map(|chunk| chunk.prepared_input)
        .collect()
}

#[test]
fn a_nested_item_repeats_its_parent_and_no_foreign_task_marker() {
    assert_eq!(
        prepared("- [x] parent\n  - [ ] child\n"),
        ["- [x] parent", "- [x] parent\n\n  - [ ] child"]
    );
}

#[test]
fn parents_repeat_indented_by_their_lists_with_block_separators() {
    let deep = "c".repeat(650);
    let nested = format!("- a\n  - b\n\n    - {deep}");
    assert_eq!(prepared(&format!("- a\n  - b\n    - {deep}\n"))[2], nested);
    // A quote between the lists adds no indentation: only lists count.
    assert_eq!(
        prepared(&format!("- a\n\n  > - b\n  >\n  >   - {deep}\n"))[2],
        nested
    );
    let markdown =
        format!("# H\n\n- first\n\n  second\n\n  ```rust\n  code\n  ```\n\n  - {deep}\n");
    let chunks = drafted(&markdown);
    // Headings lead; a code line's break carries the item's indentation as body layout.
    assert_eq!(chunks[2].prepared_input, "H\nrust\n\n- code\n  ");
    assert_eq!(chunks[2].body_text, "code\n  ");
    assert_eq!(
        chunks[3].prepared_input,
        format!("H\n- first\n  \n  second\n  \n  rust\n  code\n  \n\n  - {deep}")
    );
}

#[test]
fn later_table_rows_repeat_the_header_columns_tab_separated() {
    let (x, y) = ("x".repeat(300), "y".repeat(300));
    let markdown = format!("| a | b |\n|---|---|\n| {x} | {y} |\n| e | f |\n");
    assert_eq!(prepared(&markdown)[1], "a\tb\n\ne\tf");
}

#[test]
fn a_section_that_is_not_a_heading_is_a_structure_error() {
    let markdown = "Intro\n\n# Title\n\nBody\n";
    let mut doc = canonicalize(CanonicalizeInput::new(markdown, "sections")).unwrap();
    let intro = doc.blocks[0].block_id.clone();
    doc.blocks.last_mut().unwrap().parent_section_id = Some(intro);
    let mapped = map_document(&doc, markdown).unwrap();
    let mut count = |text: &str| Ok(text.chars().count() + 2);
    assert!(build_drafts(&doc, markdown, &mapped, &mut count).is_err());
}

#[test]
fn a_piece_inside_deletions_repeats_only_the_delimiters_it_lacks() {
    use InputRole::{SourceContent as Source, StructuralContext as Delimiter};
    let markdown = "~~x ~~y~~ z~~\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "deletions")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let layout = layout(&doc, markdown, &mapped).unwrap();
    let parts = |start, end| {
        let fragment = Fragment {
            contribution: Contribution {
                unit_index: 0,
                range: TextRange { start, end },
            },
            part_ordinal: 0,
            split: SplitKind::Whitespace,
        };
        let mut parts = Vec::new();
        layout.fragment_parts(&fragment, &mut parts).unwrap();
        parts
            .into_iter()
            .map(|part| (part.role, part.text))
            .collect::<Vec<_>>()
    };
    let delimiter = || (Delimiter, "~~".to_owned());
    assert_eq!(parts(0, 4), [(Source, "~~x ".to_owned()), delimiter()]);
    // Starting at the inner closing delimiter: only the outer deletion is open.
    assert_eq!(parts(7, 13), [delimiter(), (Source, "~~ z~~".to_owned())]);
}

#[test]
fn only_units_mapped_as_primary_text_are_primary() {
    let markdown = "- item\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "primary")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let layout = layout(&doc, markdown, &mapped).unwrap();
    let marker = mapped
        .units
        .iter()
        .position(|unit| unit.field == UnitField::ListMarker)
        .unwrap();
    let item = mapped.units.iter().position(|unit| unit.primary).unwrap();
    assert!(!layout.is_primary(marker));
    assert!(layout.is_primary(item));
    // An unknown unit is not primary.
    assert!(!layout.is_primary(mapped.units.len()));
}
