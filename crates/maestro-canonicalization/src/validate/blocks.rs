//! Block checks: children, parents, assets, attributes, inline content and tables.
use super::{contains, issue};
use crate::metadata::finding;
use crate::{
    AssetStatus, Block, BlockAttributes, BlockType, CanonicalDocument, ContentNode, Finding,
    Inline, InlineKind, Severity, SourceSpan,
};
use std::collections::BTreeMap;

/// Every check of one block: revision and type, spans, children, parent reference, container gaps,
/// assets and type-specific attributes.
pub(super) fn validate_block(
    doc: &CanonicalDocument,
    markdown: &str,
    block: &Block,
    by_id: &BTreeMap<&str, &Block>,
    issues: &mut Vec<Finding>,
) {
    if block.revision_id != doc.revision_id
        || block.block_type != block.structured_content.attributes.block_type()
    {
        issue(
            issues,
            block,
            "invalid_block",
            "block revision or type is inconsistent",
            Severity::Error,
        );
    }
    if block.source_spans.is_empty()
        || block
            .source_spans
            .iter()
            .any(|span| !span.is_valid(markdown))
    {
        issue(
            issues,
            block,
            "invalid_span",
            "block has absent, out-of-bounds or non-UTF-8 spans",
            Severity::Error,
        );
    }
    check_children(block, markdown, by_id, issues);
    check_parent_reference(block, by_id, issues);
    validate_container_gaps(block, by_id, markdown, issues);
    check_assets(block, issues);
    check_attributes(block, by_id, markdown, issues);
}

/// Each child is a known block under this parent and inside its spans, or
/// inline content that validates.
fn check_children(
    block: &Block,
    markdown: &str,
    by_id: &BTreeMap<&str, &Block>,
    issues: &mut Vec<Finding>,
) {
    for node in &block.structured_content.children {
        match node {
            ContentNode::Block { block_id } => match by_id.get(block_id.as_str()) {
                Some(child)
                    if child.parent_block_id.as_ref() == Some(&block.block_id)
                        && child
                            .source_spans
                            .iter()
                            .all(|span| contains(&block.source_spans, *span)) => {}
                _ => issue(
                    issues,
                    block,
                    "invalid_hierarchy",
                    "child reference, parent or span containment is inconsistent",
                    Severity::Error,
                ),
            },
            ContentNode::Inline { inline } => validate_inline(inline, block, markdown, issues),
        }
    }
}

/// The parent references this block exactly once.
fn check_parent_reference(
    block: &Block,
    by_id: &BTreeMap<&str, &Block>,
    issues: &mut Vec<Finding>,
) {
    if let Some(parent) = block
        .parent_block_id
        .as_ref()
        .and_then(|parent_id| by_id.get(parent_id.as_str()))
    {
        let count = parent
            .structured_content
            .children
            .iter()
            .filter(|child| {
                matches!(child,
            ContentNode::Block { block_id } if block_id == &block.block_id)
            })
            .count();
        if count != 1 {
            issue(
                issues,
                block,
                "invalid_hierarchy",
                "parent must reference its child exactly once",
                Severity::Error,
            );
        }
    }
}

/// Missing, unchecked and out-of-root assets are warned about.
fn check_assets(block: &Block, issues: &mut Vec<Finding>) {
    for asset in &block.asset_references {
        let (code, message) = match asset.status {
            AssetStatus::Missing => ("missing_asset", "local asset was not found"),
            AssetStatus::Unchecked => ("asset_unchecked", "local asset availability is unknown"),
            AssetStatus::OutsideRoot => (
                "asset_outside_root",
                "asset is outside the allowed local root",
            ),
            _ => continue,
        };
        issue(issues, block, code, message, Severity::Warning);
    }
}

/// Code must equal the parser text; tables, raw source and HTML get their own checks.
fn check_attributes(
    block: &Block,
    by_id: &BTreeMap<&str, &Block>,
    markdown: &str,
    issues: &mut Vec<Finding>,
) {
    match &block.structured_content.attributes {
        BlockAttributes::Code { .. } => {
            let code: String = block
                .structured_content
                .children
                .iter()
                .filter_map(|child| match child {
                    ContentNode::Inline {
                        inline:
                            Inline {
                                content: InlineKind::Text { text },
                                ..
                            },
                    } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if code != block.retrieval_text {
                issue(
                    issues,
                    block,
                    "code_changed",
                    "derived code differs from parser text",
                    Severity::Error,
                );
            }
        }
        BlockAttributes::Table { alignments } => {
            validate_table(block, alignments.len(), by_id, markdown, issues);
        }
        BlockAttributes::Raw { .. } => issue(
            issues,
            block,
            "source_fallback",
            "source omitted from parser events is retained verbatim; semantics are not inferred",
            Severity::Warning,
        ),
        BlockAttributes::Html => issue(
            issues,
            block,
            "raw_html",
            "raw HTML retained; its internal semantics and assets are not interpreted",
            Severity::Warning,
        ),
        _ => {}
    }
}

/// Source a container holds outside its children may only be container syntax, such as quote or
/// list markers and heading marks.
fn validate_container_gaps(
    block: &Block,
    by_id: &BTreeMap<&str, &Block>,
    markdown: &str,
    issues: &mut Vec<Finding>,
) {
    if !matches!(
        block.block_type,
        BlockType::BlockQuote
            | BlockType::List
            | BlockType::ListItem
            | BlockType::FootnoteDefinition
            | BlockType::DefinitionList
            | BlockType::DefinitionDescription
            | BlockType::Heading
    ) {
        return;
    }
    let Some(span) = block
        .source_spans
        .first()
        .filter(|span| span.is_valid(markdown))
    else {
        return;
    };
    let mut children: Vec<SourceSpan> = block
        .structured_content
        .children
        .iter()
        .flat_map(|child| match child {
            ContentNode::Block { block_id } => by_id
                .get(block_id.as_str())
                .map(|child_block| child_block.source_spans.clone())
                .unwrap_or_default(),
            ContentNode::Inline { inline } => vec![inline.source_span],
        })
        .filter(|child_span| child_span.is_valid(markdown) && contains(&[*span], *child_span))
        .collect();
    children.sort_by_key(|child_span| child_span.start);
    children.push(SourceSpan {
        start: span.end,
        end: span.end,
    });
    let mut cursor = span.start;
    for child in children {
        if cursor < child.start {
            let mut gap = markdown[cursor..child.start].to_owned();
            if let BlockAttributes::FootnoteDefinition { label } =
                &block.structured_content.attributes
            {
                gap = gap.replace(&format!("[^{label}]:"), "");
            }
            if cursor == span.start {
                if let BlockAttributes::BlockQuote { alert: Some(alert) } =
                    &block.structured_content.attributes
                {
                    gap = gap.to_ascii_lowercase().replacen(
                        &format!("[!{}]", alert.to_ascii_lowercase()),
                        "",
                        1,
                    );
                }
            }
            // Only container syntax can be outside children. Duplicate reference
            // definitions carry labels/URLs and must not hide inside an outer span.
            if gap.chars().any(|character| {
                !(character.is_whitespace()
                    || character.is_ascii_digit()
                    || matches!(character, '>' | '-' | '+' | '*' | '.' | ')' | ':')
                    || (block.block_type == BlockType::Heading && matches!(character, '#' | '=')))
            }) {
                let mut gap_finding = finding(
                    "unparsed_content",
                    "container contains source material omitted from child blocks",
                    Severity::Error,
                    Some(SourceSpan {
                        start: cursor,
                        end: child.start,
                    }),
                );
                gap_finding.block_id = Some(block.block_id.clone());
                issues.push(gap_finding);
            }
        }
        cursor = cursor.max(child.end);
    }
}

/// An inline node lies inside its block's spans and holds its children; raw HTML outside an HTML
/// block is reported.
fn validate_inline(inline: &Inline, block: &Block, markdown: &str, issues: &mut Vec<Finding>) {
    if !inline.source_span.is_valid(markdown) || !contains(&block.source_spans, inline.source_span)
    {
        issue(
            issues,
            block,
            "invalid_inline_span",
            "inline source span is invalid or outside its block",
            Severity::Error,
        );
    }
    if matches!(inline.content, InlineKind::Html { .. }) && block.block_type != BlockType::Html {
        issue(
            issues,
            block,
            "raw_html",
            "inline HTML retained without interpreting its internal semantics",
            Severity::Warning,
        );
    }
    for child in &inline.children {
        if !contains(&[inline.source_span], child.source_span) {
            issue(
                issues,
                block,
                "invalid_inline_span",
                "inline child is outside its parent",
                Severity::Error,
            );
        }
        validate_inline(child, block, markdown, issues);
    }
}

/// Every row of a table has one cell per column, and the source around its cells is only table
/// syntax.
fn validate_table(
    block: &Block,
    width: usize,
    by_id: &BTreeMap<&str, &Block>,
    markdown: &str,
    issues: &mut Vec<Finding>,
) {
    for node in &block.structured_content.children {
        let ContentNode::Block { block_id } = node else {
            continue;
        };
        let Some(row) = by_id.get(block_id.as_str()) else {
            continue;
        };
        let cells: Vec<_> = row
            .structured_content
            .children
            .iter()
            .filter_map(|child| match child {
                ContentNode::Block { block_id } => by_id.get(block_id.as_str()).copied(),
                ContentNode::Inline { .. } => None,
            })
            .filter(|cell| cell.block_type == BlockType::TableCell)
            .collect();
        if cells.len() != width {
            issue(
                issues,
                block,
                "invalid_table",
                "table header and row widths differ",
                Severity::Error,
            );
        }
        // GFM permits surplus cells that the parser omits. Raw children preserve
        // those ranges without inventing column names or rejecting valid syntax.
        if let Some(span) = row
            .source_spans
            .first()
            .filter(|span| span.is_valid(markdown))
        {
            let mut represented: Vec<_> = row
                .structured_content
                .children
                .iter()
                .filter_map(|child| match child {
                    ContentNode::Block { block_id } => by_id.get(block_id.as_str()),
                    ContentNode::Inline { .. } => None,
                })
                .flat_map(|child| child.source_spans.iter().copied())
                .filter(|cell_span| cell_span.is_valid(markdown))
                .collect();
            represented.sort_by_key(|cell_span| cell_span.start);
            let mut cursor = span.start;
            for source in represented {
                if cursor <= source.start {
                    table_gap(markdown, cursor, source.start, block, issues);
                }
                cursor = cursor.max(source.end);
            }
            if cursor <= span.end {
                table_gap(markdown, cursor, span.end, block, issues);
            }
        }
    }
}

/// Source between table cells may hold only pipes and whitespace; anything else is content lost.
fn table_gap(markdown: &str, start: usize, end: usize, block: &Block, issues: &mut Vec<Finding>) {
    if markdown[start..end]
        .chars()
        .any(|character| !character.is_whitespace() && character != '|')
    {
        let mut gap_finding = finding(
            "table_content_loss",
            "table source contains material not represented by cells",
            Severity::Error,
            Some(SourceSpan { start, end }),
        );
        gap_finding.block_id = Some(block.block_id.clone());
        issues.push(gap_finding);
    }
}
