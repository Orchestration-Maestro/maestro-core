//! Packing and preparation: shared chunks, context text, containers, part numbers and the table
//! windows a chunk must replay.
use super::*;

/// The fake counter: characters plus two.
fn fake() -> impl FnMut(&str) -> Result<usize, Error> {
    |text| Ok(text.chars().count() + 2)
}

#[test]
fn small_blocks_share_one_chunk_below_the_target() {
    let (_, chunks) = structural_chunks("One.\n\nTwo.\n\nThree.\n\nFour.\n").unwrap();
    assert_eq!(chunks.len(), 1);
}

#[test]
fn context_leads_the_prepared_input_without_a_separator_before_it() {
    let (_, chunks) = structural_chunks("# A\n\n## B\n\nBody one.\n\nBody two.\n").unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.body_text.contains("Body one."))
        .unwrap();
    assert_eq!(chunk.prepared_input, "A\n\nB\n\nBody one.\n\nBody two.");
}

#[test]
fn a_chunk_names_each_container_once_outermost_first() {
    let markdown = "- one\n- two\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "containers")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let chunks = build_drafts(&doc, markdown, &mapped, &mut fake()).unwrap();
    let ids: Vec<_> = doc
        .blocks
        .iter()
        .map(|block| block.block_id.clone())
        .collect();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].container_ids, ids);
}

#[test]
fn a_split_unit_numbers_its_parts_in_order() {
    let (_, chunks) = structural_chunks(&format!("{}\n", "word ".repeat(400))).unwrap();
    let ordinals: Vec<_> = chunks
        .iter()
        .flat_map(|chunk| &chunk.fragments)
        .map(|fragment| fragment.part_ordinal)
        .collect();
    assert!(ordinals.len() > 1);
    assert_eq!(ordinals, (0..ordinals.len()).collect::<Vec<_>>());
}

#[test]
fn table_windows_must_match_their_rows_in_order() {
    let markdown = "| a | b |\n|---|---|\n| c | d |\n| e | f |\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "windows")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let chunks = build_drafts(&doc, markdown, &mapped, &mut fake()).unwrap();
    validate_preparation(&doc, markdown, &mapped, &chunks, &mut fake()).unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.table_windows.len() >= 2)
        .unwrap();
    let layout = layout(&doc, markdown, &mapped).unwrap();
    // Each altered chunk is prepared from its own windows, so only the window checks refuse it.
    let mut swapped = chunk.table_windows.clone();
    swapped.swap(0, 1);
    let mut reversed = chunk.table_windows.clone();
    for window in &mut reversed {
        window.columns.reverse();
    }
    let mut renumbered = chunk.table_windows.clone();
    renumbered.last_mut().unwrap().row_index += 10;
    for windows in [swapped, reversed, renumbered] {
        let body = Body {
            fragments: chunk.fragments.clone(),
            windows,
        };
        let altered = layout.prepare(&body, &mut fake()).unwrap();
        assert!(validate_preparation(&doc, markdown, &mapped, &[altered], &mut fake()).is_err());
    }
}
