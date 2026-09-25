//! Validation by replay: canonicalize the reference bytes again with a document's recorded inputs
//! and compare the result with the document.
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::metadata::finding;
use crate::model::{CanonicalizeInput, Finding, Severity};
use crate::pipeline::canonicalize;

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
fn replay(doc: &CanonicalDocument, markdown: &str) -> Result<CanonicalDocument, Error> {
    let mut input = CanonicalizeInput::new(markdown, &doc.original_markdown_reference.path);
    input.document_id = Some(&doc.document_id);
    input.metadata = doc.input_metadata.clone();
    input.operational_metadata = doc.operational_metadata.clone();
    input.extractor_blocks.clone_from(&doc.extractor_blocks);
    input.parser_options = doc.parser_options.clone();
    input.assets = doc.asset_inventory.clone();
    canonicalize(input)
}
