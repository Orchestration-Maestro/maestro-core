use super::super::super::{
    sections::SectionIndex,
    spans::{SeedSpan, SpanUnion},
};
use super::{document, union};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

fn exact_union(document: &CanonicalDocument, span: Span) -> SpanUnion {
    let revision_id = document.revision_id.clone();
    SpanUnion {
        revision_id: revision_id.clone(),
        span,
        seeds: vec![SeedSpan {
            chunk_id: "chunk-a".to_owned(),
            revision_id,
            section_id: None,
            span,
            input_position: 0,
            score: None,
            routes: BTreeSet::new(),
        }],
    }
}

fn share_heading_start(document: &mut CanonicalDocument, parent: &str, child: &str) {
    let parent_id = document
        .sections
        .iter()
        .find(|section| section.title == parent)
        .unwrap()
        .section_id
        .clone();
    let child_id = document
        .sections
        .iter()
        .find(|section| section.title == child)
        .unwrap()
        .section_id
        .clone();
    let parent_span = document
        .blocks
        .iter()
        .find(|block| block.block_id == parent_id)
        .unwrap()
        .source_spans[0];
    document
        .blocks
        .iter_mut()
        .find(|block| block.block_id == child_id)
        .unwrap()
        .source_spans[0] = parent_span;
}

#[test]
fn mismatched_heading_parent_is_rejected() {
    let markdown = "## Parent\n\n### Child\n";
    let mut source_document = document(markdown);
    let child_id = source_document
        .sections
        .iter()
        .find(|section| section.title == "Child")
        .unwrap()
        .section_id
        .clone();
    source_document
        .blocks
        .iter_mut()
        .find(|block| block.block_id == child_id)
        .unwrap()
        .parent_section_id = None;

    assert_eq!(
        SectionIndex::new(&source_document, markdown).unwrap_err(),
        "canonical section and heading disagree"
    );
}

#[test]
fn empty_section_heading_extent_is_rejected() {
    let markdown = "## Empty\n";
    let mut source_document = document(markdown);
    let heading_id = source_document.sections[0].section_id.clone();
    let heading = source_document
        .blocks
        .iter_mut()
        .find(|block| block.block_id == heading_id)
        .unwrap();
    heading.source_spans[0].end = heading.source_spans[0].start;

    assert_eq!(
        SectionIndex::new(&source_document, markdown).unwrap_err(),
        "canonical section extent is invalid"
    );
}

#[test]
fn root_windows_skip_nested_blocks() {
    let markdown = "Before.\n\n> Nested body.\n\nAfter.\n";
    let source_document = document(markdown);
    let seed = union(&source_document, markdown, "Before.", "Before.", None);
    let expansion = SectionIndex::new(&source_document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();
    let window = expansion.window_plan(seed.span).unwrap();

    assert_eq!(window.after.len(), 2);
    assert_eq!(window.after[0].start, markdown.find("> Nested").unwrap());
    assert_eq!(window.after[1].start, markdown.find("After.").unwrap());
}

#[test]
fn window_seed_boundaries_belong_to_one_sibling() {
    let markdown = "First.\n\nSecond.\n";
    let mut source_document = document(markdown);
    let first_id = source_document
        .blocks
        .iter()
        .find(|block| block.retrieval_text == "First.")
        .unwrap()
        .block_id
        .clone();
    let first_span = source_document
        .blocks
        .iter()
        .find(|block| block.block_id == first_id)
        .unwrap()
        .source_spans[0];
    let second_span = source_document
        .blocks
        .iter()
        .find(|block| block.retrieval_text == "Second.")
        .unwrap()
        .source_spans[0];
    let second_span = Span {
        start: second_span.start,
        end: second_span.end,
    };
    source_document
        .blocks
        .iter_mut()
        .find(|block| block.block_id == first_id)
        .unwrap()
        .source_spans[0]
        .end = second_span.start;

    let expansion = SectionIndex::new(&source_document, markdown)
        .unwrap()
        .expand(&exact_union(
            &source_document,
            Span {
                start: second_span.start,
                end: second_span.end,
            },
        ))
        .unwrap();
    let starts_at_boundary = expansion
        .window_plan(Span {
            start: second_span.start,
            end: second_span.end,
        })
        .unwrap();
    assert_eq!(starts_at_boundary.mandatory, second_span);

    let ends_at_boundary = Span {
        start: first_span.start,
        end: second_span.start,
    };
    assert_eq!(
        expansion.window_plan(ends_at_boundary).unwrap().mandatory,
        ends_at_boundary
    );
}

#[test]
fn a_shorter_section_wins_when_headings_share_a_source_start() {
    let markdown = concat!(
        "## Parent\n\n### Child\n\nChild body.\n\n",
        "### Sibling\n\nSibling body.\n"
    );
    let mut source_document = document(markdown);
    let parent_id = source_document.sections[0].section_id.clone();
    let child_id = source_document
        .sections
        .iter()
        .find(|section| section.title == "Child")
        .unwrap()
        .section_id
        .clone();
    share_heading_start(&mut source_document, "Parent", "Child");
    let seed = union(
        &source_document,
        markdown,
        "Child body.",
        "Child body.",
        Some(&parent_id),
    );

    assert_eq!(
        SectionIndex::new(&source_document, markdown)
            .unwrap()
            .expand(&seed)
            .unwrap()
            .section_id,
        Some(child_id)
    );
}

#[test]
fn equal_section_extents_keep_the_first_canonical_section() {
    let markdown = "## Parent\n\n### Child\n\nChild body.\n";
    let mut source_document = document(markdown);
    let parent_id = source_document.sections[0].section_id.clone();
    share_heading_start(&mut source_document, "Parent", "Child");
    let seed = union(
        &source_document,
        markdown,
        "Child body.",
        "Child body.",
        Some(&parent_id),
    );

    assert_eq!(
        SectionIndex::new(&source_document, markdown)
            .unwrap()
            .expand(&seed)
            .unwrap()
            .section_id,
        Some(parent_id)
    );
}
