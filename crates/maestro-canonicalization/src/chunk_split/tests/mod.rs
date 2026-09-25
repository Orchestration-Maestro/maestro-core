//! Tests of structural preparation and packing.
use super::prepare::fit_prefix;
use super::structure::{Body, layout};
use super::{MAX_TOKENS, build_drafts, validate_preparation};
use crate::{
    chunk_mapping::map_document,
    error::Error,
    model::CanonicalizeInput,
    pipeline::canonicalize,
    prepared_inputs::{ChunkContent, Fragment, InputRole, SplitKind},
    source_units::{MappedDocument, UnitField},
};
use std::collections::BTreeSet;

mod boundaries;
mod context;
mod packing;
mod splitting;

fn structural_chunks(markdown: &str) -> Result<(MappedDocument, Vec<ChunkContent>), Error> {
    let doc = canonicalize(CanonicalizeInput::new(markdown, "structural-test"))?;
    let mapped = map_document(&doc, markdown)?;
    let mut fake_counter = |text: &str| Ok(text.chars().count() + 2);
    let chunks = build_drafts(&doc, markdown, &mapped, &mut fake_counter)?;
    for chunk in &chunks {
        assert!(chunk.token_count <= 700);
        assert_eq!(
            chunk.prepared_input,
            chunk
                .input_parts
                .iter()
                .map(|part| part.text.as_str())
                .collect::<String>()
        );
        assert_eq!(
            chunk.body_text,
            chunk
                .input_parts
                .iter()
                .filter(|part| part.role == InputRole::SourceContent || part.body_layout)
                .map(|part| part.text.as_str())
                .collect::<String>()
        );
        for part in &chunk.input_parts {
            assert_eq!(
                &chunk.prepared_input[part.prepared_range.start..part.prepared_range.end],
                part.text
            );
        }
    }
    for (index, unit) in mapped
        .units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.primary)
    {
        let mut ranges: Vec<_> = chunks
            .iter()
            .flat_map(|chunk| &chunk.fragments)
            .filter(|fragment| fragment.contribution.unit_index == index)
            .map(|fragment| fragment.contribution.range)
            .collect();
        ranges.sort_by_key(|range| range.start);
        let mut cursor = 0;
        for range in ranges {
            assert_eq!(range.start, cursor, "body gap or overlap");
            assert!(range.end > range.start);
            cursor = range.end;
        }
        assert_eq!(cursor, unit.text.len(), "missing rendered body");
    }
    Ok((mapped, chunks))
}

#[test]
fn scalar_fallback_preserves_all_body_text_without_overlap() {
    let markdown = "界".repeat(1500);
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.body_text.as_str())
            .collect::<String>(),
        markdown
    );
}

#[test]
fn code_continuations_retain_info_and_line_fragment_identity() {
    let body = format!("{}\n", "x".repeat(1500));
    let markdown = format!("```rust\n{body}```\n");
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.body_text.as_str())
            .collect::<String>(),
        body
    );
    for chunk in &chunks {
        assert!(
            chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::StructuralContext && part.text == "rust")
        );
        assert!(!chunk.prepared_input.contains("```"));
        assert!(
            chunk
                .fragments
                .iter()
                .any(|fragment| fragment.split == SplitKind::CodeLineFragment)
        );
    }
}

#[test]
fn table_cell_windows_keep_matching_headers_without_missing_column_placeholders() {
    let markdown = format!(
        "| Name | Value |\n|---|---|\n| alpha | {} |\n",
        "界".repeat(1500)
    );
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    let cells: Vec<_> = chunks
        .iter()
        .filter(|chunk| chunk.body_text.contains('界'))
        .collect();
    assert!(cells.len() > 1);
    for chunk in cells {
        assert_eq!(chunk.table_windows.len(), 1);
        assert_eq!(chunk.table_windows[0].columns, [1]);
        assert_eq!(chunk.table_windows[0].row_index, 1);
        assert!(
            chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::TableHeaderContext && part.text == "Value")
        );
        assert!(!chunk.body_text.contains('\t'));
        assert!(
            chunk
                .fragments
                .iter()
                .any(|fragment| fragment.split == SplitKind::CellFragment)
        );
    }
}

#[test]
fn nested_item_repeats_parent_but_not_its_own_unsplit_body() {
    let markdown = format!("- Parent\n  - {}\n", "x".repeat(1500));
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    let children: Vec<_> = chunks
        .iter()
        .filter(|chunk| chunk.body_text.contains('x'))
        .collect();
    assert!(children.len() > 1);
    for chunk in children {
        assert!(chunk.prepared_input.contains("Parent"));
        assert!(
            chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::ParentListContext && part.text == "Parent")
        );
        assert!(
            !chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::ParentListContext && part.text.contains('x'))
        );
    }
}

#[test]
fn task_status_is_source_backed_context_on_every_item_continuation() {
    for status in ["x", " "] {
        let markdown = format!("- [{status}] {}\n", "a ".repeat(1500));
        let (_, chunks) = structural_chunks(&markdown).unwrap();
        let continuations: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.body_text.contains('a'))
            .collect();
        assert!(continuations.len() > 1);
        for chunk in continuations {
            assert!(
                chunk
                    .input_parts
                    .iter()
                    .any(|part| part.role == InputRole::StructuralContext
                        && part.text == format!("[{status}] ")
                        && !part.contributions.is_empty())
            );
        }
    }
}

#[test]
fn list_ordinals_retain_source_backed_input_provenance() {
    let (mapped, chunks) = structural_chunks("23. body\n").unwrap();
    let marker = mapped
        .units
        .iter()
        .position(|unit| unit.field == UnitField::ListMarker)
        .unwrap();
    assert!(chunks[0].input_parts.iter().any(|part| {
        part.role == InputRole::StructuralContext
            && part
                .contributions
                .iter()
                .any(|chunk| chunk.unit_index == marker)
            && part.mappings.iter().any(|run| !run.origins.is_empty())
    }));
}

#[test]
fn definition_descriptions_prefer_complete_items_below_the_hard_limit() {
    let markdown = format!(
        "Term\n\n:   {}\n\n    {}\n",
        "a".repeat(510),
        "b".repeat(100)
    );
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].prepared_input.starts_with("Term\n\n"));
}

#[test]
fn whole_items_win_over_an_early_paragraph_target() {
    let markdown = format!("- {}\n\n  {}\n", "a".repeat(510), "b".repeat(100));
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].body_text.contains(&"a".repeat(510)));
    assert!(chunks[0].body_text.contains(&"b".repeat(100)));
}

#[test]
fn list_continuation_lines_have_explicit_canonical_indentation() {
    let (_, chunks) = structural_chunks("- first\n  second\n").unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].prepared_input, "- first\n  second");
    assert!(
        chunks[0]
            .input_parts
            .iter()
            .any(|part| part.role == InputRole::FormattingSeparator && part.text == "  ")
    );
}

#[test]
fn nested_item_context_keeps_all_direct_parent_paragraphs() {
    let markdown = format!("- Before\n  - {}\n\n  After\n", "x".repeat(1500));
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    for chunk in chunks.iter().filter(|chunk| chunk.body_text.contains('x')) {
        let parent: String = chunk
            .input_parts
            .iter()
            .filter(|part| part.role == InputRole::ParentListContext)
            .map(|part| part.text.as_str())
            .collect();
        assert!(parent.contains("Before"));
        assert!(parent.contains("After"));
        assert!(!parent.contains('x'));
    }
}

#[test]
fn mandatory_context_is_not_clipped_to_make_a_chunk_fit() {
    for markdown in [
        format!("# {}\n\nbody\n", "H".repeat(800)),
        format!("- {}\n  - child\n", "P".repeat(800)),
        format!("| {} |\n|---|\n| value |\n", "H".repeat(800)),
    ] {
        assert!(structural_chunks(&markdown).is_err());
    }
}

#[test]
fn deletion_semantics_surround_every_continuation_without_extra_body_coverage() {
    let markdown = format!("~~{}~~\n", "x".repeat(1500));
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.body_text.as_str())
            .collect::<String>(),
        markdown.trim_end_matches('\n')
    );
    for chunk in chunks {
        assert!(chunk.prepared_input.starts_with("~~"));
        assert!(chunk.prepared_input.ends_with("~~"));
    }
}

#[test]
fn section_boundaries_and_heading_or_header_only_documents_remain_independent() {
    let (_, chunks) = structural_chunks("# One\n\na\n\n# Two\n\nb\n").unwrap();
    assert_eq!(chunks.len(), 2);
    assert_ne!(chunks[0].section_id, chunks[1].section_id);
    for chunk in &chunks {
        assert!(
            !chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::HeadingContext)
        );
    }
    for markdown in ["# Alone\n", "| Header |\n|---|\n"] {
        let (_, chunks) = structural_chunks(markdown).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].body_text.is_empty());
    }
    assert!(
        structural_chunks("---\ntitle: Metadata\n---\n")
            .unwrap()
            .1
            .is_empty()
    );
}

#[test]
fn definition_continuations_retain_the_actual_term() {
    let markdown = format!("Term\n: {}\n", "x".repeat(1500));
    let (_, chunks) = structural_chunks(&markdown).unwrap();
    for chunk in chunks.iter().filter(|chunk| chunk.body_text.contains('x')) {
        assert!(
            chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::StructuralContext && part.text == "Term")
        );
    }
}

#[test]
fn tables_inside_items_keep_the_list_marker_and_row_indentation() {
    let (_, chunks) = structural_chunks("- | A | B |\n  |---|---|\n  | one | two |\n").unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].table_windows.len(), 2);
    assert_eq!(chunks[0].prepared_input, "- A\tB\n  one\ttwo");
}

#[test]
fn genuine_empty_cells_are_retained_in_whole_rows() {
    let (_, chunks) =
        structural_chunks("| A | B | C |\n|---|---|---|\n| one | | three |\n").unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].body_text, "A\tB\tC\none\t\tthree");
    assert_eq!(chunks[0].table_windows[1].columns, [0, 1, 2]);
}

#[test]
fn failed_shorter_boundary_does_not_discard_a_measured_fitting_prefix() {
    let mut measured = Vec::new();
    let result = fit_prefix("abcdefgh", &[2], &mut |end| {
        measured.push(end);
        Ok(end == 4)
    })
    .unwrap();
    assert_eq!(result, 4);
    assert_eq!(measured, [8, 4, 2]);
}
