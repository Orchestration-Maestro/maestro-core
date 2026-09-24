//! Local, deterministic canonical documents. No networking or inference.
//! Source replay verifies consistency, not the authenticity of caller-supplied metadata.
//! Failed documents are inspectable evidence, never trusted retrieval input.
//!
//! ```
//! use maestro_canonicalization::{canonicalize, CanonicalizeInput};
//! let doc = canonicalize(CanonicalizeInput::new("# Hello\n", "notes/hello.md"))?;
//! assert_eq!(doc.blocks[0].retrieval_text, "Hello");
//! # Ok::<(), maestro_canonicalization::Error>(())
//! ```
#![forbid(unsafe_code)]
mod accounting;
mod assemble;
mod chunk_mapping;
mod chunk_split;
mod chunks;
mod content;
mod dedup;
mod metadata;
mod model;
mod parse;
mod store;
mod tokenizer;
mod validate;
pub use accounting::{SourceAccounting, SourceRole};
pub use chunks::*;
pub use content::*;
pub use dedup::*;
pub use model::*;
use sha2::{Digest, Sha256};
pub use store::{load_document, save_document};
pub use tokenizer::NativeTokenizer;
pub use validate::validate_document;

/// Serialization contract version, independent of source-document revisions.
pub const SCHEMA_VERSION: &str = "1.2.0";
/// Exact parser and transformation profile. Bump when derived output semantics change.
pub const PARSER_VERSION: &str = "canonicalization/0.3.0+pulldown-cmark/0.13.4+source-accounting";

/// An input or execution error that prevents canonicalization.
#[derive(Debug)]
pub struct Error(
    /// Human-readable refusal, without source-document contents.
    pub String,
);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// Lower-case hexadecimal SHA-256 of the bytes.
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

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
    let mut nodes = parse::parse(input.markdown, &input.parser_options)?;
    let (source_metadata, warnings) = metadata::merge(&nodes, &input.metadata);
    let identity = source_metadata
        .source_reference
        .as_deref()
        .unwrap_or(input.identity_key);
    let document_id = input.document_id.map_or_else(
        || {
            let namespace = if source_metadata.source_reference.is_some() {
                "source"
            } else {
                "local"
            };
            format!(
                "doc-{}",
                digest(format!("{namespace}\0{identity}").as_bytes())
            )
        },
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
    .map_err(|e| Error(e.to_string()))?;
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
    assemble::assemble(&nodes, &mut doc)?;
    let gaps: Vec<_> = validate::validate_structure(&doc, input.markdown)
        .into_iter()
        .filter(|f| matches!(f.code.as_str(), "unparsed_content" | "table_content_loss"))
        .flat_map(|f| {
            f.source_spans
                .into_iter()
                .map(move |span| (span, f.code.clone()))
        })
        .collect();
    if !gaps.is_empty() {
        parse::preserve_gaps(&mut nodes, input.markdown, gaps);
        doc.blocks.clear();
        doc.sections.clear();
        doc.links.clear();
        assemble::assemble(&nodes, &mut doc)?;
    }
    doc.source_accounting = accounting::account(&doc, input.markdown);
    doc.warnings
        .extend(validate::validate_structure(&doc, input.markdown));
    doc.validation_status = if doc.warnings.iter().any(|w| w.severity == Severity::Error) {
        ValidationStatus::Failed
    } else if doc.warnings.is_empty() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::ValidWithWarnings
    };
    Ok(doc)
}
