//! Structural checks against the preserved bytes; no guessed repairs.
use self::blocks::validate_block;
use self::sections::{validate_extractor, validate_sections};
use crate::metadata::finding;
use crate::{Block, BlockType, CanonicalDocument, Finding, Severity, SourceSpan, digest};
use std::collections::{BTreeMap, BTreeSet};

mod blocks;
mod sections;

/// Replay canonicalization against the reference bytes and retained input metadata.
/// Returns all original findings plus a blocking error if any derived field was altered.
/// This detects inconsistent JSON, not a malicious party replacing all inputs and hashes;
/// authenticity of the original source requires a separately trusted reference.
#[must_use]
pub fn validate_document(doc: &CanonicalDocument, markdown: &str) -> Vec<Finding> {
    match replay(doc, markdown) {
        Ok(expected) => {
            let mut issues = expected.warnings.clone();
            if expected != *doc {
                issues.push(finding(
                    "canonical_mismatch",
                    "canonical JSON differs from deterministic source replay",
                    Severity::Error,
                    None,
                ));
            }
            issues
        }
        Err(_) => vec![finding(
            "replay_error",
            "canonical document cannot be reproduced with the recorded inputs",
            Severity::Error,
            None,
        )],
    }
}

/// Canonicalize the Markdown again with the document's recorded inputs.
fn replay(doc: &CanonicalDocument, markdown: &str) -> Result<CanonicalDocument, crate::Error> {
    let mut input = crate::CanonicalizeInput::new(markdown, &doc.original_markdown_reference.path);
    input.document_id = Some(&doc.document_id);
    input.metadata = doc.input_metadata.clone();
    input.operational_metadata = doc.operational_metadata.clone();
    input.extractor_blocks.clone_from(&doc.extractor_blocks);
    input.parser_options = doc.parser_options.clone();
    input.assets = doc.asset_inventory.clone();
    crate::canonicalize(input)
}

/// Every structural check of a document against its Markdown, findings in a fixed order.
pub(crate) fn validate_structure(doc: &CanonicalDocument, markdown: &str) -> Vec<Finding> {
    let mut issues = Vec::new();
    check_reference(doc, markdown, &mut issues);
    let by_id: BTreeMap<_, _> = doc
        .blocks
        .iter()
        .map(|b| (b.block_id.as_str(), b))
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
            .is_some_and(|p| !seen.contains(p.as_str()))
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
            || part.role == crate::SourceRole::Unaccounted
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
    let usable = doc.blocks.iter().any(|b| {
        !matches!(
            b.block_type,
            BlockType::Metadata | BlockType::ThematicBreak | BlockType::ReferenceDefinition
        ) && !b.retrieval_text.trim().is_empty()
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
        .flat_map(|b| b.source_spans.iter().copied())
        .filter(|s| s.is_valid(markdown))
        .collect();
    spans.sort_by_key(|s| s.start);
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

/// Whether one of the spans contains the span.
fn contains(spans: &[SourceSpan], span: SourceSpan) -> bool {
    spans
        .iter()
        .any(|s| s.start <= span.start && span.end <= s.end)
}

/// Record a finding about a block, located at its spans.
fn issue(issues: &mut Vec<Finding>, block: &Block, code: &str, message: &str, severity: Severity) {
    issues.push(Finding {
        severity,
        code: code.into(),
        message: message.into(),
        block_id: Some(block.block_id.clone()),
        source_spans: block.source_spans.clone(),
    });
}

#[cfg(test)]
mod tests;
