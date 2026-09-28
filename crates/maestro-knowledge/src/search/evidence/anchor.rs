//! The source anchor of each passage of an assembled bundle: where its text
//! lies, without the text, in the shape S2's G06 records.

use maestro_kernel::evidence::Passage;
use serde::Serialize;

/// A passage of an assembled bundle, as its source pins it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anchor {
    /// Where its document comes from.
    pub source_ref: String,
    /// Its document, pinned.
    pub doc_id: String,
    /// Its revision, pinned.
    pub revision_id: String,
    /// Its section, when it belongs to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// Its half-open byte span in the revision's original Markdown.
    pub span: [usize; 2],
    /// The SHA-256 of its text, written `sha256:` and 64 hexadecimal
    /// characters, as the evidence wire format writes it.
    pub digest: String,
}

impl From<&Passage> for Anchor {
    fn from(passage: &Passage) -> Self {
        Self {
            source_ref: passage.source_ref.clone(),
            doc_id: passage.document_id.clone(),
            revision_id: passage.revision_id.clone(),
            section_id: passage.section_id.clone(),
            span: [passage.span.start, passage.span.end],
            digest: format!("sha256:{}", passage.digest.as_str()),
        }
    }
}
