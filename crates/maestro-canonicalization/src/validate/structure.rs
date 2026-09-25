//! Whole-document structural checks: reference, block hierarchy, source ledger and coverage.
use super::blocks::validate_block;
use super::report::issue;
use super::sections::{validate_extractor, validate_sections};
use crate::content::{Block, BlockType};
use crate::metadata::finding;
use crate::model::{Finding, Severity, SourceSpan};
use crate::{CanonicalDocument, SourceRole, digest};
use std::collections::{BTreeMap, BTreeSet};

/// Every structural check of a document against its Markdown, findings in a fixed order.
pub(crate) fn validate_structure(doc: &CanonicalDocument, markdown: &str) -> Vec<Finding> {
    let mut issues = Vec::new();
    check_reference(doc, markdown, &mut issues);
    let by_id: BTreeMap<_, _> = doc
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    if by_id.len() != doc.blocks.len() {
        issues.push(finding(
            "duplicate_block_id",
            "block IDs are not unique",
            Severity::Error,
            None,
        ));
    }
    let mut seen = BTreeSet::new();
    for block in &doc.blocks {
        validate_block(doc, markdown, block, &by_id, &mut issues);
        if block
            .parent_block_id
            .as_ref()
            .is_some_and(|parent_id| !seen.contains(parent_id.as_str()))
        {
            issue(
                &mut issues,
                block,
                "invalid_hierarchy",
                "parent block must precede its child",
                Severity::Error,
            );
        }
        seen.insert(block.block_id.as_str());
    }
    validate_sections(doc, &by_id, &mut issues);
    validate_extractor(doc, markdown, &mut issues);
    check_source_ledger(doc, markdown, &by_id, &mut issues);
    check_usable(doc, &mut issues);
    check_artifacts(markdown, &mut issues);
    check_uncovered(doc, markdown, &mut issues);
    issues
}

/// The Markdown reference must match the recorded hash and length.
fn check_reference(doc: &CanonicalDocument, markdown: &str, issues: &mut Vec<Finding>) {
    let hash = format!("sha256:{}", digest(markdown.as_bytes()));
    if doc.content_hash != hash
        || doc.original_markdown_reference.content_hash != hash
        || doc.original_markdown_reference.byte_length != markdown.len()
    {
        issues.push(finding(
            "reference_mismatch",
            "Markdown reference bytes do not match the recorded hash/length",
            Severity::Error,
            None,
        ));
    }
}

/// The source ledger covers every byte once, in order, with known blocks.
fn check_source_ledger(
    doc: &CanonicalDocument,
    markdown: &str,
    by_id: &BTreeMap<&str, &Block>,
    issues: &mut Vec<Finding>,
) {
    let mut cursor = 0;
    for part in &doc.source_accounting {
        if part.source_span.start != cursor
            || !part.source_span.is_valid(markdown)
            || part.source_span.start == part.source_span.end
            || part.role == SourceRole::Unaccounted
            || part
                .block_id
                .as_deref()
                .is_some_and(|id| !by_id.contains_key(id))
        {
            issues.push(finding(
                "invalid_source_accounting",
                "source ledger is incomplete or inconsistent",
                Severity::Error,
                Some(part.source_span),
            ));
        }
        cursor = part.source_span.end;
    }
    if cursor != markdown.len() {
        issues.push(finding(
            "invalid_source_accounting",
            "source ledger does not cover the entire reference",
            Severity::Error,
            None,
        ));
    }
}

/// A document needs at least one block with searchable content.
fn check_usable(doc: &CanonicalDocument, issues: &mut Vec<Finding>) {
    let usable = doc.blocks.iter().any(|block| {
        !matches!(
            block.block_type,
            BlockType::Metadata | BlockType::ThematicBreak | BlockType::ReferenceDefinition
        ) && !block.retrieval_text.trim().is_empty()
    });
    if !usable {
        issues.push(finding(
            "empty_document",
            "document has no usable Markdown content",
            Severity::Error,
            None,
        ));
    }
}

/// NUL, replacement characters and leftover extraction markers are warned about.
fn check_artifacts(markdown: &str, issues: &mut Vec<Finding>) {
    for artifact in ["\0", "\u{fffd}", "DOCLINGCODE", "DOCLINGBREAK"] {
        if let Some(start) = markdown.find(artifact) {
            issues.push(finding(
                "extraction_artifact",
                "source contains NUL, a replacement character, or an un-restored extraction marker",
                Severity::Warning,
                Some(SourceSpan {
                    start,
                    end: start + artifact.len(),
                }),
            ));
        }
    }
}

/// Anything outside block spans must be whitespace, not content swallowed by a parser.
fn check_uncovered(doc: &CanonicalDocument, markdown: &str, issues: &mut Vec<Finding>) {
    let mut spans: Vec<_> = doc
        .blocks
        .iter()
        .flat_map(|block| block.source_spans.iter().copied())
        .filter(|span| span.is_valid(markdown))
        .collect();
    spans.sort_by_key(|span| span.start);
    let mut cursor = 0;
    for span in spans {
        if cursor < span.start {
            check_gap(markdown, cursor, span.start, issues);
        }
        cursor = cursor.max(span.end);
    }
    check_gap(markdown, cursor, markdown.len(), issues);
}

/// Non-whitespace source outside every block span is unparsed content.
fn check_gap(markdown: &str, start: usize, end: usize, issues: &mut Vec<Finding>) {
    if start < end && !markdown[start..end].trim().is_empty() {
        issues.push(finding(
            "unparsed_content",
            "non-whitespace source is not represented by a block; bytes retained in reference",
            Severity::Error,
            Some(SourceSpan { start, end }),
        ));
    }
}
