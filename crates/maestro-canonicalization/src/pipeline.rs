//! The canonicalization pipeline: parse, merge metadata, assemble blocks, keep what the parser
//! dropped, account for every byte and validate the result.
use crate::accounting::account;
use crate::assemble::assemble;
use crate::document::{CanonicalDocument, MarkdownReference};
use crate::error::Error;
use crate::hashing::digest;
use crate::metadata::merge;
use crate::model::{CanonicalizeInput, Finding, Severity, SourceMetadata, ValidationStatus};
use crate::parse::{parse, preserve_gaps};
use crate::validate::validate_structure;

/// Serialization contract version, independent of source-document revisions.
pub const SCHEMA_VERSION: &str = "1.2.0";
/// Exact parser and transformation profile. Bump when derived output semantics change.
pub const PARSER_VERSION: &str = "canonicalization/0.3.0+pulldown-cmark/0.13.4+source-accounting";

/// Parse unchanged Markdown into a provenance-bearing canonical document.
///
/// No filesystem, clock, random-number generator, network or model is used.
/// A returned document can have `Failed` validation: inspect its findings before use.
/// Revision IDs include identity, exact Markdown hash and supplied provenance;
/// block IDs additionally include the parser profile/options and source order.
///
/// # Errors
/// Returns an error for absent identity, serialization failures or unsafe nesting.
/// Parser-omitted source is retained in explicit raw blocks with warnings.
pub fn canonicalize(input: CanonicalizeInput<'_>) -> Result<CanonicalDocument, Error> {
    if input.identity_key.trim().is_empty() {
        return Err(Error(
            "a stable local Markdown reference is required".into(),
        ));
    }
    if input.document_id.is_some_and(|id| id.trim().is_empty()) {
        return Err(Error("explicit document_id cannot be empty".into()));
    }
    let mut nodes = parse(input.markdown, &input.parser_options)?;
    let (source_metadata, warnings) = merge(&nodes, &input.metadata);
    let document_id = input.document_id.map_or_else(
        || derived_document_id(&source_metadata, input.identity_key),
        str::to_owned,
    );
    let content_hash = format!("sha256:{}", digest(input.markdown.as_bytes()));
    let revision_bytes = serde_json::to_vec(&(
        &document_id,
        &content_hash,
        &input.metadata,
        &source_metadata,
        &input.extractor_blocks,
    ))
    .map_err(|error| Error(error.to_string()))?;
    let revision_id = format!("rev-{}", digest(&revision_bytes));
    let mut doc = CanonicalDocument {
        schema_version: SCHEMA_VERSION.into(),
        document_id,
        revision_id,
        source_reference: source_metadata.source_reference.clone(),
        access_policy: source_metadata.access_policy.clone(),
        source_metadata,
        input_metadata: input.metadata,
        operational_metadata: input.operational_metadata,
        original_markdown_reference: MarkdownReference {
            path: input.identity_key.into(),
            content_hash: content_hash.clone(),
            byte_length: input.markdown.len(),
        },
        content_hash,
        parser_version: PARSER_VERSION.into(),
        parser_options: input.parser_options,
        asset_inventory: input.assets,
        validation_status: ValidationStatus::Valid,
        blocks: Vec::new(),
        source_accounting: Vec::new(),
        sections: Vec::new(),
        links: Vec::new(),
        warnings,
        extractor_blocks: input.extractor_blocks,
    };
    assemble(&nodes, &mut doc)?;
    let gaps: Vec<_> = validate_structure(&doc, input.markdown)
        .into_iter()
        .filter(|finding| {
            matches!(
                finding.code.as_str(),
                "unparsed_content" | "table_content_loss"
            )
        })
        .flat_map(|finding| {
            finding
                .source_spans
                .into_iter()
                .map(move |span| (span, finding.code.clone()))
        })
        .collect();
    if !gaps.is_empty() {
        preserve_gaps(&mut nodes, input.markdown, gaps);
        doc.blocks.clear();
        doc.sections.clear();
        doc.links.clear();
        assemble(&nodes, &mut doc)?;
    }
    doc.source_accounting = account(&doc, input.markdown);
    doc.warnings
        .extend(validate_structure(&doc, input.markdown));
    doc.validation_status = validation_status(&doc.warnings);
    Ok(doc)
}

/// The document identifier when none is supplied: the digest of the source reference, or of the
/// local identity key when the source is unknown, each in its own namespace.
fn derived_document_id(source_metadata: &SourceMetadata, identity_key: &str) -> String {
    let identity = source_metadata
        .source_reference
        .as_deref()
        .unwrap_or(identity_key);
    let namespace = if source_metadata.source_reference.is_some() {
        "source"
    } else {
        "local"
    };
    format!(
        "doc-{}",
        digest(format!("{namespace}\0{identity}").as_bytes())
    )
}

/// The verdict the findings give: failed on any error, else valid with or without warnings.
fn validation_status(warnings: &[Finding]) -> ValidationStatus {
    if warnings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        ValidationStatus::Failed
    } else if warnings.is_empty() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::ValidWithWarnings
    }
}
