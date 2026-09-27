use super::super::conflicts::detect::{ConflictContext, detect_conflicts};
use super::conflicts::{document, source, table};
use maestro_canonicalization::{BlockType, ContentNode, Inline, InlineKind, SourceSpan};
use std::collections::BTreeSet;

fn table_index(document: &maestro_canonicalization::CanonicalDocument) -> usize {
    document
        .blocks
        .iter()
        .position(|block| {
            matches!(
                &block.structured_content.attributes,
                maestro_canonicalization::BlockAttributes::Table { .. }
            )
        })
        .unwrap()
}

fn table_header_id(
    document: &maestro_canonicalization::CanonicalDocument,
    table_index: usize,
) -> String {
    document.blocks[table_index]
        .structured_content
        .children
        .iter()
        .find_map(|node| match node {
            ContentNode::Block { block_id }
                if document
                    .blocks
                    .iter()
                    .find(|block| block.block_id == *block_id)
                    .is_some_and(|block| matches!(&block.block_type, BlockType::TableHead)) =>
            {
                Some(block_id.clone())
            }
            ContentNode::Block { .. } | ContentNode::Inline { .. } => None,
        })
        .unwrap()
}

fn duplicate_header(
    mut document: maestro_canonicalization::CanonicalDocument,
) -> maestro_canonicalization::CanonicalDocument {
    let index = table_index(&document);
    let header_id = table_header_id(&document, index);
    document.blocks[index]
        .structured_content
        .children
        .push(ContentNode::Block {
            block_id: header_id,
        });
    document
}

fn malformed_row_child(
    mut document: maestro_canonicalization::CanonicalDocument,
    extra_inline: bool,
) -> maestro_canonicalization::CanonicalDocument {
    let row_index = document
        .blocks
        .iter()
        .position(|block| matches!(&block.block_type, BlockType::TableRow))
        .unwrap();
    if extra_inline {
        document.blocks[row_index]
            .structured_content
            .children
            .push(ContentNode::Inline {
                inline: Inline {
                    content: InlineKind::Text {
                        text: "ignored".to_owned(),
                    },
                    source_span: SourceSpan { start: 0, end: 0 },
                    children: Vec::new(),
                },
            });
    } else {
        let cell_id = document.blocks[row_index]
            .structured_content
            .children
            .iter()
            .find_map(|node| match node {
                ContentNode::Block { block_id } => Some(block_id.clone()),
                ContentNode::Inline { .. } => None,
            })
            .unwrap();
        document
            .blocks
            .iter_mut()
            .find(|block| block.block_id == cell_id)
            .unwrap()
            .block_type = BlockType::Paragraph;
    }
    document
}

fn invalid_header_cell(
    mut document: maestro_canonicalization::CanonicalDocument,
) -> maestro_canonicalization::CanonicalDocument {
    let header_id = table_header_id(&document, table_index(&document));
    let cell_id = document
        .blocks
        .iter()
        .find(|block| block.block_id == header_id)
        .unwrap()
        .structured_content
        .children
        .iter()
        .find_map(|node| match node {
            ContentNode::Block { block_id } => Some(block_id.clone()),
            ContentNode::Inline { .. } => None,
        })
        .unwrap();
    document
        .blocks
        .iter_mut()
        .find(|block| block.block_id == cell_id)
        .unwrap()
        .block_type = BlockType::Paragraph;
    document
}

#[test]
fn malformed_canonical_table_structure_is_ignored() {
    let baseline = table("Agent", "Port", "7005");
    let malformed = table("Agent", "Port", "7006");
    let baseline_document = document(&baseline, "baseline.md");
    let malformed_documents = [
        duplicate_header(document(&malformed, "duplicate-header.md")),
        invalid_header_cell(document(&malformed, "wrong-header-cell.md")),
        malformed_row_child(document(&malformed, "extra-inline-row-child.md"), true),
        malformed_row_child(document(&malformed, "wrong-row-cell.md"), false),
    ];
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    for malformed_document in malformed_documents {
        assert!(
            detect_conflicts(&[
                source(0, &baseline_document, &baseline, &group, &context),
                source(1, &malformed_document, &malformed, &group, &context),
            ])
            .unwrap()
            .is_empty()
        );
    }
}
