//! Block checks: children, parents, assets, attributes, inline content and tables.
use super::report::issue;
use crate::content::{Block, BlockAttributes, BlockType, ContentNode, Inline, InlineKind};
use crate::document::CanonicalDocument;
use crate::metadata::finding;
use crate::model::{AssetStatus, Finding, Severity, SourceSpan};
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
    let mut check = BlockCheck {
        block,
        by_id,
        markdown,
        issues,
    };
    if block.revision_id != doc.revision_id
        || block.block_type != block.structured_content.attributes.block_type()
    {
        check.refuse("invalid_block", "block revision or type is inconsistent");
    }
    if block.source_spans.is_empty()
        || block
            .source_spans
            .iter()
            .any(|span| !span.is_valid(markdown))
    {
        check.refuse(
            "invalid_span",
            "block has absent, out-of-bounds or non-UTF-8 spans",
        );
    }
    check.children();
    check.parent_reference();
    check.container_gaps();
    check.assets();
    check.attributes();
}

/// One block under check: what every check of it reads, and where the checks record findings.
struct BlockCheck<'doc> {
    /// The block checked.
    block: &'doc Block,
    /// Every block of the document, by id.
    by_id: &'doc BTreeMap<&'doc str, &'doc Block>,
    /// The document's Markdown source.
    markdown: &'doc str,
    /// The findings recorded so far.
    issues: &'doc mut Vec<Finding>,
}

impl BlockCheck<'_> {
    /// Record an error about the block, located at its spans.
    fn refuse(&mut self, code: &str, message: &str) {
        issue(self.issues, self.block, code, message, Severity::Error);
    }

    /// Record a warning about the block, located at its spans.
    fn warn(&mut self, code: &str, message: &str) {
        issue(self.issues, self.block, code, message, Severity::Warning);
    }

    /// Record an error about the block, located at one stretch of source.
    fn refuse_at(&mut self, code: &str, message: &str, span: SourceSpan) {
        let mut lost = finding(code, message, Severity::Error, Some(span));
        lost.block_id = Some(self.block.block_id.clone());
        self.issues.push(lost);
    }

    /// Each child is a known block under this parent and inside its spans, or
    /// inline content that validates.
    fn children(&mut self) {
        let block = self.block;
        for node in &block.structured_content.children {
            match node {
                ContentNode::Block { block_id } => match self.by_id.get(block_id.as_str()) {
                    Some(child)
                        if child.parent_block_id.as_ref() == Some(&block.block_id)
                            && child
                                .source_spans
                                .iter()
                                .all(|span| contains(&block.source_spans, *span)) => {}
                    _ => self.refuse(
                        "invalid_hierarchy",
                        "child reference, parent or span containment is inconsistent",
                    ),
                },
                ContentNode::Inline { inline } => self.inline(inline),
            }
        }
    }

    /// The parent references this block exactly once.
    fn parent_reference(&mut self) {
        let Some(parent) = self
            .block
            .parent_block_id
            .as_ref()
            .and_then(|parent_id| self.by_id.get(parent_id.as_str()))
        else {
            return;
        };
        if child_block_ids(parent)
            .filter(|child_id| *child_id == self.block.block_id)
            .count()
            != 1
        {
            self.refuse(
                "invalid_hierarchy",
                "parent must reference its child exactly once",
            );
        }
    }

    /// Missing, unchecked and out-of-root assets are warned about.
    fn assets(&mut self) {
        for asset in &self.block.asset_references {
            let (code, message) = match asset.status {
                AssetStatus::Missing => ("missing_asset", "local asset was not found"),
                AssetStatus::Unchecked => {
                    ("asset_unchecked", "local asset availability is unknown")
                }
                AssetStatus::OutsideRoot => (
                    "asset_outside_root",
                    "asset is outside the allowed local root",
                ),
                _ => continue,
            };
            self.warn(code, message);
        }
    }

    /// Code must equal the parser text; tables, raw source and HTML get their own checks.
    fn attributes(&mut self) {
        match &self.block.structured_content.attributes {
            BlockAttributes::Code { .. } => {
                if inline_text(self.block) != self.block.retrieval_text {
                    self.refuse("code_changed", "derived code differs from parser text");
                }
            }
            BlockAttributes::Table { alignments } => self.table(alignments.len()),
            BlockAttributes::Raw { .. } => self.warn(
                "source_fallback",
                "source omitted from parser events is retained verbatim; \
                 semantics are not inferred",
            ),
            BlockAttributes::Html => self.warn(
                "raw_html",
                "raw HTML retained; its internal semantics and assets are not interpreted",
            ),
            _ => {}
        }
    }

    /// Source a container holds outside its children may only be container syntax, such as
    /// quote or list markers and heading marks.
    fn container_gaps(&mut self) {
        if !matches!(
            self.block.block_type,
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
        let Some(span) = self
            .block
            .source_spans
            .first()
            .filter(|first| first.is_valid(self.markdown))
        else {
            return;
        };
        let mut children: Vec<SourceSpan> = self
            .block
            .structured_content
            .children
            .iter()
            .flat_map(|node| match node {
                ContentNode::Block { block_id } => self
                    .by_id
                    .get(block_id.as_str())
                    .map(|child| child.source_spans.clone())
                    .unwrap_or_default(),
                ContentNode::Inline { inline } => vec![inline.source_span],
            })
            .filter(|candidate| candidate.is_valid(self.markdown) && contains(&[*span], *candidate))
            .collect();
        children.sort_by_key(|child_span| child_span.start);
        children.push(SourceSpan {
            start: span.end,
            end: span.end,
        });
        let mut cursor = span.start;
        for child in children {
            if cursor < child.start {
                let between = SourceSpan {
                    start: cursor,
                    end: child.start,
                };
                self.container_gap(between, cursor == span.start);
            }
            cursor = cursor.max(child.end);
        }
    }

    /// One stretch of container source between children, less its footnote label and, when it
    /// leads the container, its quote alert, may only be container syntax.
    fn container_gap(&mut self, between: SourceSpan, leading: bool) {
        let attributes = &self.block.structured_content.attributes;
        let mut gap = self.markdown[between.start..between.end].to_owned();
        if let BlockAttributes::FootnoteDefinition { label } = attributes {
            gap = gap.replace(&format!("[^{label}]:"), "");
        }
        if leading {
            if let BlockAttributes::BlockQuote { alert: Some(alert) } = attributes {
                gap = gap.to_ascii_lowercase().replacen(
                    &format!("[!{}]", alert.to_ascii_lowercase()),
                    "",
                    1,
                );
            }
        }
        // Only container syntax can be outside children. Duplicate reference
        // definitions carry labels/URLs and must not hide inside an outer span.
        let heading = self.block.block_type == BlockType::Heading;
        if gap.chars().any(|character| {
            !(character.is_whitespace()
                || character.is_ascii_digit()
                || matches!(character, '>' | '-' | '+' | '*' | '.' | ')' | ':')
                || (heading && matches!(character, '#' | '=')))
        }) {
            self.refuse_at(
                "unparsed_content",
                "container contains source material omitted from child blocks",
                between,
            );
        }
    }

    /// An inline node lies inside its block's spans and holds its children; raw HTML outside an
    /// HTML block is reported.
    fn inline(&mut self, inline: &Inline) {
        if !inline.source_span.is_valid(self.markdown)
            || !contains(&self.block.source_spans, inline.source_span)
        {
            self.refuse(
                "invalid_inline_span",
                "inline source span is invalid or outside its block",
            );
        }
        if matches!(inline.content, InlineKind::Html { .. })
            && self.block.block_type != BlockType::Html
        {
            self.warn(
                "raw_html",
                "inline HTML retained without interpreting its internal semantics",
            );
        }
        for child in &inline.children {
            if !contains(&[inline.source_span], child.source_span) {
                self.refuse("invalid_inline_span", "inline child is outside its parent");
            }
            self.inline(child);
        }
    }

    /// Every row of a table has one cell per column, and the source around its cells is only
    /// table syntax.
    fn table(&mut self, width: usize) {
        for row in child_blocks(self.block, self.by_id) {
            self.table_row(row, width);
        }
    }

    /// One row of the table has one cell per column, and the source around its cells is only
    /// table syntax.
    fn table_row(&mut self, row: &Block, width: usize) {
        let cells = child_blocks(row, self.by_id)
            .filter(|cell| cell.block_type == BlockType::TableCell)
            .count();
        if cells != width {
            self.refuse("invalid_table", "table header and row widths differ");
        }
        // GFM permits surplus cells that the parser omits. Raw children preserve
        // those ranges without inventing column names or rejecting valid syntax.
        let Some(span) = row
            .source_spans
            .first()
            .filter(|row_span| row_span.is_valid(self.markdown))
        else {
            return;
        };
        let mut represented: Vec<_> = child_blocks(row, self.by_id)
            .flat_map(|child| child.source_spans.iter().copied())
            .filter(|cell_span| cell_span.is_valid(self.markdown))
            .collect();
        represented.sort_by_key(|cell_span| cell_span.start);
        let mut cursor = span.start;
        for source in represented {
            if cursor <= source.start {
                self.table_gap(cursor, source.start);
            }
            cursor = cursor.max(source.end);
        }
        if cursor <= span.end {
            self.table_gap(cursor, span.end);
        }
    }

    /// Source between table cells may hold only pipes and whitespace; anything else is content
    /// lost.
    fn table_gap(&mut self, start: usize, end: usize) {
        if self.markdown[start..end]
            .chars()
            .any(|character| !character.is_whitespace() && character != '|')
        {
            self.refuse_at(
                "table_content_loss",
                "table source contains material not represented by cells",
                SourceSpan { start, end },
            );
        }
    }
}

/// The text of a block's inline text children, joined in order: what a code block derives.
fn inline_text(block: &Block) -> String {
    block
        .structured_content
        .children
        .iter()
        .filter_map(|node| match node {
            ContentNode::Inline {
                inline:
                    Inline {
                        content: InlineKind::Text { text },
                        ..
                    },
            } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The ids of the blocks among a block's children, in order.
fn child_block_ids(block: &Block) -> impl Iterator<Item = &str> {
    block
        .structured_content
        .children
        .iter()
        .filter_map(|node| match node {
            ContentNode::Block { block_id } => Some(block_id.as_str()),
            ContentNode::Inline { .. } => None,
        })
}

/// The known blocks among a block's children, in order; an unknown id is skipped.
fn child_blocks<'doc>(
    block: &'doc Block,
    by_id: &'doc BTreeMap<&str, &'doc Block>,
) -> impl Iterator<Item = &'doc Block> {
    child_block_ids(block).filter_map(|child_id| by_id.get(child_id).copied())
}

/// Whether one of the spans contains the span.
fn contains(spans: &[SourceSpan], span: SourceSpan) -> bool {
    spans
        .iter()
        .any(|outer| outer.start <= span.start && span.end <= outer.end)
}
