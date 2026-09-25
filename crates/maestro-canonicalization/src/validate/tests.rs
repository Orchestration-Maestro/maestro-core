//! Structural checks against documents altered one field at a time: each fault is reported
//! on its own, and malformed spans are reported rather than trusted.
use super::*;
use crate::{
    AssetStatus, CanonicalizeInput, ContentNode, ExtractorBlock, Inline, SourceAccounting,
    SourceRole, canonicalize,
};

/// The canonical document of some Markdown.
fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "validate.md")).unwrap()
}

/// Whether the structural checks report `code` with a message containing `part`.
fn reports(doc: &CanonicalDocument, markdown: &str, code: &str, part: &str) -> bool {
    validate_structure(doc, markdown)
        .iter()
        .any(|finding| finding.code == code && finding.message.contains(part))
}

/// The index of the first block of a type.
fn index(doc: &CanonicalDocument, kind: &BlockType) -> usize {
    doc.blocks
        .iter()
        .position(|block| block.block_type == *kind)
        .unwrap()
}

/// The first inline node among a block's children.
fn first_inline(block: &mut Block) -> &mut Inline {
    block
        .structured_content
        .children
        .iter_mut()
        .find_map(|child| match child {
            ContentNode::Inline { inline } => Some(inline),
            ContentNode::Block { .. } => None,
        })
        .unwrap()
}

#[test]
fn each_reference_field_is_checked_on_its_own() {
    let markdown = "Body\n";
    let doc = document(markdown);
    assert!(!reports(&doc, markdown, "reference_mismatch", ""));
    for field in 0..3 {
        let mut changed = doc.clone();
        match field {
            0 => changed.content_hash = "sha256:0".into(),
            1 => changed.original_markdown_reference.content_hash = "sha256:0".into(),
            _ => changed.original_markdown_reference.byte_length += 1,
        }
        assert!(
            reports(&changed, markdown, "reference_mismatch", ""),
            "{field}"
        );
    }
}

#[test]
fn each_source_ledger_fault_is_reported_at_its_part() {
    let markdown = "# Title\n\nBody text\n";
    let doc = document(markdown);
    assert!(!reports(&doc, markdown, "invalid_source_accounting", ""));
    let entry_index = doc
        .source_accounting
        .iter()
        .position(|entry| {
            entry.block_id.is_some() && entry.source_span.end - entry.source_span.start >= 2
        })
        .unwrap();
    let part = doc.source_accounting[entry_index].clone();
    let empty = SourceAccounting {
        source_span: SourceSpan {
            start: part.source_span.start,
            end: part.source_span.start,
        },
        ..part.clone()
    };
    for fault in 0..4 {
        let mut changed = doc.clone();
        let ledger = &mut changed.source_accounting;
        match fault {
            0 => ledger[entry_index].source_span.start += 1,
            1 => ledger.insert(entry_index, empty.clone()),
            2 => ledger[entry_index].role = SourceRole::Unaccounted,
            _ => ledger[entry_index].block_id = Some("unknown".into()),
        }
        let at = changed.source_accounting[entry_index].source_span;
        assert!(
            validate_structure(&changed, markdown)
                .iter()
                .any(|finding| finding.code == "invalid_source_accounting"
                    && finding.source_spans == [at]),
            "{fault}"
        );
    }
}

#[test]
fn a_block_of_another_revision_or_without_spans_is_invalid() {
    let markdown = "Body\n";
    let doc = document(markdown);
    let mut other = doc.clone();
    other.blocks[0].revision_id = "another revision".into();
    assert!(reports(
        &other,
        markdown,
        "invalid_block",
        "revision or type"
    ));
    let mut spanless = doc;
    spanless.blocks[0].source_spans.clear();
    assert!(reports(&spanless, markdown, "invalid_span", "absent"));
}

#[test]
fn a_child_must_name_its_parent_and_lie_inside_it() {
    let markdown = "# Title\n\n> Quote\n\nAfter\n";
    let doc = document(markdown);
    let message = "child reference, parent or span containment";
    assert!(!reports(&doc, markdown, "invalid_hierarchy", message));
    let (heading, quote) = (
        index(&doc, &BlockType::Heading),
        index(&doc, &BlockType::BlockQuote),
    );
    let child = quote + 1;
    let mut adopted = doc.clone();
    adopted.blocks[child].parent_block_id = Some(doc.blocks[heading].block_id.clone());
    assert!(reports(&adopted, markdown, "invalid_hierarchy", message));
    let mut overflowing = doc.clone();
    overflowing.blocks[child].source_spans[0].end = markdown.len();
    assert!(reports(
        &overflowing,
        markdown,
        "invalid_hierarchy",
        message
    ));
    let mut twice = doc;
    let reference = twice.blocks[quote].structured_content.children[0].clone();
    twice.blocks[quote]
        .structured_content
        .children
        .push(reference);
    assert!(reports(
        &twice,
        markdown,
        "invalid_hierarchy",
        "exactly once"
    ));
}

#[test]
fn outside_assets_changed_code_and_inline_html_are_reported() {
    let markdown = "![x](pic.svg) <b>bold</b>\n\n```\ncode\n```\n";
    let doc = document(markdown);
    assert!(reports(&doc, markdown, "raw_html", "inline HTML"));
    assert!(!reports(&doc, markdown, "asset_outside_root", ""));
    assert!(!reports(&doc, markdown, "code_changed", ""));
    let mut changed = doc.clone();
    changed.blocks[0].asset_references[0].status = AssetStatus::OutsideRoot;
    let code = index(&doc, &BlockType::Code);
    changed.blocks[code].retrieval_text = "other".into();
    assert!(reports(&changed, markdown, "asset_outside_root", ""));
    assert!(reports(&changed, markdown, "code_changed", ""));
}

#[test]
fn an_alert_marker_is_quote_syntax_not_lost_content() {
    // Lost content would come back as a raw fallback block with its own finding.
    let doc = document("> [!NOTE]\n> Remember this.\n");
    let kinds: Vec<_> = doc
        .blocks
        .iter()
        .map(|block| block.block_type.clone())
        .collect();
    assert_eq!(kinds, [BlockType::BlockQuote, BlockType::Paragraph]);
    assert!(
        doc.warnings
            .iter()
            .all(|finding| finding.code == "missing_metadata")
    );
}

#[test]
fn malformed_inline_spans_are_reported_not_trusted() {
    let markdown = "# Été\n\nBody\n\nOther\n";
    let doc = document(markdown);
    let message = "invalid or outside its block";
    assert!(!reports(&doc, markdown, "invalid_inline_span", message));
    let (heading, body) = (
        index(&doc, &BlockType::Heading),
        index(&doc, &BlockType::Paragraph),
    );
    let mut outside = doc.clone();
    first_inline(&mut outside.blocks[body]).source_span = doc.blocks[body + 1].source_spans[0];
    assert!(reports(&outside, markdown, "invalid_inline_span", message));
    // Inside the heading, but splitting the two-byte `É`: never sliced, only reported.
    let mut split = doc;
    first_inline(&mut split.blocks[heading]).source_span.start += 1;
    assert!(reports(&split, markdown, "invalid_inline_span", message));
}

#[test]
fn table_source_left_between_cells_is_reported() {
    let markdown = "| a | b |\n|---|---|\n| c | d |\n";
    let doc = document(markdown);
    assert!(!reports(&doc, markdown, "table_content_loss", ""));
    let cell = doc
        .blocks
        .iter()
        .position(|block| block.block_type == BlockType::TableCell && block.retrieval_text == "c")
        .unwrap();
    let mut changed = doc;
    let span = &mut changed.blocks[cell].source_spans[0];
    span.start = span.end;
    assert!(reports(&changed, markdown, "table_content_loss", ""));
}

#[test]
fn section_paths_and_block_contexts_are_checked() {
    let markdown = "# Title\n\n## Part\n\nBody\n";
    let doc = document(markdown);
    assert!(!reports(&doc, markdown, "invalid_section", ""));
    assert!(!reports(&doc, markdown, "invalid_section_context", ""));
    let mut path = doc.clone();
    path.sections[1].heading_path = vec!["Other".into()];
    assert!(reports(
        &path,
        markdown,
        "invalid_section",
        "path, heading or level"
    ));
    let mut level = doc.clone();
    level.sections[1].level = 7;
    assert!(reports(
        &level,
        markdown,
        "invalid_section",
        "path, heading or level"
    ));
    let mut context = doc.clone();
    let body = index(&doc, &BlockType::Paragraph);
    context.blocks[body].heading_path = vec!["Other".into()];
    assert!(reports(&context, markdown, "invalid_section_context", ""));
}

#[test]
fn extractor_ids_must_be_unique() {
    let markdown = "Body\n";
    let mut doc = document(markdown);
    let block = ExtractorBlock {
        extractor_id: "page-1".into(),
        markdown_spans: vec![SourceSpan { start: 0, end: 4 }],
        original_locations: Vec::new(),
        structured_content: serde_json::json!({}),
    };
    doc.extractor_blocks = vec![block.clone()];
    assert!(!reports(&doc, markdown, "invalid_extractor_id", ""));
    doc.extractor_blocks.push(block);
    assert!(reports(&doc, markdown, "invalid_extractor_id", ""));
}
