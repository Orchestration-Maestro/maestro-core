use super::super::sections::SectionIndex;
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ContentNode, SourceSpan, canonicalize,
};

const MARKDOWN: &str = "## Guide\n\n> Quoted text.\n";

fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "fixture.md")).unwrap()
}

fn nested_ids(document: &CanonicalDocument) -> (String, String) {
    let child = document
        .blocks
        .iter()
        .find(|block| block.parent_block_id.is_some())
        .unwrap();
    (
        child.block_id.clone(),
        child.parent_block_id.clone().unwrap(),
    )
}

fn index_error(document: &CanonicalDocument, markdown: &str) -> String {
    let error = SectionIndex::new(document, markdown).unwrap_err();
    assert_eq!(
        SectionIndex::new_from_length(document, markdown.len()).unwrap_err(),
        error
    );
    error
}

#[test]
fn canonical_child_link_refusals_are_specific() {
    let mut missing_child = document(MARKDOWN);
    let (_, parent_id) = nested_ids(&missing_child);
    missing_child
        .blocks
        .iter_mut()
        .find(|block| block.block_id == parent_id)
        .unwrap()
        .structured_content
        .children
        .push(ContentNode::Block {
            block_id: "missing-child".to_owned(),
        });
    assert_eq!(
        index_error(&missing_child, MARKDOWN),
        "canonical block has a missing child link"
    );

    let mut mismatched_child = document(MARKDOWN);
    let (child_id, _) = nested_ids(&mismatched_child);
    mismatched_child
        .blocks
        .iter_mut()
        .find(|block| block.block_id == child_id)
        .unwrap()
        .parent_block_id = None;
    assert_eq!(
        index_error(&mismatched_child, MARKDOWN),
        "canonical child and parent links disagree"
    );
}

#[test]
fn canonical_parent_link_refusals_are_specific() {
    let mut missing_parent = document(MARKDOWN);
    let (child_id, parent_id) = nested_ids(&missing_parent);
    let parent = missing_parent
        .blocks
        .iter_mut()
        .find(|block| block.block_id == parent_id)
        .unwrap();
    parent
        .structured_content
        .children
        .retain(|node| !matches!(node, ContentNode::Block { block_id } if block_id == &child_id));
    missing_parent
        .blocks
        .iter_mut()
        .find(|block| block.block_id == child_id)
        .unwrap()
        .parent_block_id = Some("missing-parent".to_owned());
    assert_eq!(
        index_error(&missing_parent, MARKDOWN),
        "canonical block has a missing parent link"
    );

    let mut parent_without_span = document(MARKDOWN);
    let (_, parent_id) = nested_ids(&parent_without_span);
    parent_without_span
        .blocks
        .iter_mut()
        .find(|block| block.block_id == parent_id)
        .unwrap()
        .source_spans
        .clear();
    assert_eq!(
        index_error(&parent_without_span, MARKDOWN),
        "canonical parent block has no source span"
    );

    let mut parent_outside_child = document(MARKDOWN);
    let (child_id, parent_id) = nested_ids(&parent_outside_child);
    let child_start = parent_outside_child
        .blocks
        .iter()
        .find(|block| block.block_id == child_id)
        .unwrap()
        .source_spans[0]
        .start;
    parent_outside_child
        .blocks
        .iter_mut()
        .find(|block| block.block_id == parent_id)
        .unwrap()
        .source_spans = vec![SourceSpan {
        start: child_start,
        end: child_start,
    }];
    assert_eq!(
        index_error(&parent_outside_child, MARKDOWN),
        "canonical parent does not contain its child"
    );

    let mut cyclic = document(MARKDOWN);
    let (child_id, parent_id) = nested_ids(&cyclic);
    cyclic
        .blocks
        .iter_mut()
        .find(|block| block.block_id == parent_id)
        .unwrap()
        .structured_content
        .children
        .retain(|node| !matches!(node, ContentNode::Block { block_id } if block_id == &child_id));
    let child = cyclic
        .blocks
        .iter_mut()
        .find(|block| block.block_id == child_id)
        .unwrap();
    child.parent_block_id = Some(child_id.clone());
    child
        .structured_content
        .children
        .push(ContentNode::Block { block_id: child_id });
    assert_eq!(
        index_error(&cyclic, MARKDOWN),
        "canonical block links are cyclic"
    );
}

#[test]
fn a_section_refuses_a_sibling_that_crosses_its_end() {
    let markdown = "## First\n\nFirst body.\n\n## Second\n\nSecond body.\n";
    let mut malformed = document(markdown);
    let second_heading = markdown.find("## Second").unwrap();
    let body = malformed
        .blocks
        .iter_mut()
        .find(|block| block.retrieval_text.contains("First body."))
        .unwrap();
    body.source_spans[0].end = second_heading + 1;

    assert_eq!(
        index_error(&malformed, markdown),
        "canonical section extent is invalid"
    );
}

#[test]
fn canonical_section_link_refusals_are_specific() {
    let markdown = "## Guide\n\nText.\n";
    let mut missing_parent = document(markdown);
    missing_parent.sections[0].parent_section_id = Some("missing-parent".to_owned());
    assert_eq!(
        index_error(&missing_parent, markdown),
        "canonical section has a missing parent link"
    );

    let mut cyclic = document(markdown);
    let section_id = cyclic.sections[0].section_id.clone();
    cyclic.sections[0].parent_section_id = Some(section_id);
    assert_eq!(
        index_error(&cyclic, markdown),
        "canonical section links are cyclic"
    );

    let mut mismatched_heading = document(markdown);
    mismatched_heading.sections[0].level = 3;
    assert_eq!(
        index_error(&mismatched_heading, markdown),
        "canonical section and heading disagree"
    );

    let mut missing_heading = document(markdown);
    let old_id = missing_heading.sections[0].section_id.clone();
    missing_heading.sections[0].section_id = "missing-heading".to_owned();
    for block in &mut missing_heading.blocks {
        if block.parent_section_id.as_deref() == Some(old_id.as_str()) {
            block.parent_section_id = Some("missing-heading".to_owned());
        }
    }
    assert_eq!(
        index_error(&missing_heading, markdown),
        "canonical section has no heading block"
    );

    let mut missing_block_section = document(markdown);
    missing_block_section.blocks[0].parent_section_id = Some("missing-section".to_owned());
    assert_eq!(
        index_error(&missing_block_section, markdown),
        "canonical block names a missing section"
    );
}

#[test]
fn additional_valid_block_spans_are_accepted() {
    let markdown = "## Guide\n\n> Quoted text.\n";
    let mut source_document = document(markdown);
    let block = source_document
        .blocks
        .iter_mut()
        .find(|block| !block.source_spans.is_empty())
        .unwrap();
    block.source_spans.push(block.source_spans[0]);

    assert!(SectionIndex::new(&source_document, markdown).is_ok());
}

#[test]
fn text_and_length_indexes_agree_for_every_canonical_section() {
    let markdown = "# Guide\n\n## Parent\n\nText.\n\n### Child\n\nNested.\n\n## Last\n";
    let document = document(markdown);
    let text_index = SectionIndex::new(&document, markdown).unwrap();
    let length_index = SectionIndex::new_from_length(&document, markdown.len()).unwrap();

    for section in &document.sections {
        assert_eq!(
            text_index.section_extent(&section.section_id),
            length_index.section_extent(&section.section_id)
        );
    }
}
