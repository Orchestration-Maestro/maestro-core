//! The canonical document: the versioned record canonicalization returns, its heading sections
//! and the ledger that accounts for every original byte.
use crate::content::{Block, Link};
use crate::model::{
    AssetStatus, ExtractorBlock, Finding, ParserOptions, SourceMetadata, SourceSpan,
    ValidationStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Hash-checked reference to the exact original Markdown bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownReference {
    /// Library input reference, or snapshot path relative to the saved JSON.
    pub path: String,
    /// SHA-256 of the full unchanged Markdown, prefixed with `sha256:`.
    pub content_hash: String,
    /// UTF-8 byte length, not character count.
    pub byte_length: usize,
}

/// A versioned document independent of databases and later retrieval processing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalDocument {
    /// Version of this serialization contract.
    pub schema_version: String,
    /// Stable source identity, independent of equal content from other sources.
    pub document_id: String,
    /// Immutable identity of this document's Markdown and provenance revision.
    pub revision_id: String,
    /// Exact Markdown content hash; never used alone as document identity.
    pub content_hash: String,
    /// Original source reference, or null when unknown.
    pub source_reference: Option<String>,
    /// Retained source metadata including original frontmatter values.
    pub source_metadata: SourceMetadata,
    /// Supplied metadata before frontmatter merging, retained for deterministic validation replay.
    pub input_metadata: SourceMetadata,
    /// Explicit operational envelope, retained but excluded from source/revision/block identities.
    /// This is not source provenance or an authorization channel.
    pub operational_metadata: BTreeMap<String, Value>,
    /// Supplied access policy, or null. This is not an authorization engine.
    pub access_policy: Option<Value>,
    /// Location, length and hash of the unchanged Markdown reference.
    pub original_markdown_reference: MarkdownReference,
    /// Exact parser version and transformation profile.
    pub parser_version: String,
    /// Extensions used for this derived representation.
    pub parser_options: ParserOptions,
    /// Explicit availability snapshot used for local asset findings.
    pub asset_inventory: BTreeMap<String, AssetStatus>,
    /// Validation verdict; a failed document must not be consumed as trustworthy.
    pub validation_status: ValidationStatus,
    /// Source-ordered natural block arena, linked by identifiers rather than flattened.
    pub blocks: Vec<Block>,
    /// Half-open partition of all source bytes, separate from overlapping block spans.
    pub source_accounting: Vec<SourceAccounting>,
    /// Heading sections with lexical parent relationships.
    pub sections: Vec<Section>,
    /// Links and image references with exact source spans.
    pub links: Vec<Link>,
    /// Retained existing extractor structures and original-document mappings.
    pub extractor_blocks: Vec<ExtractorBlock>,
    /// All findings, including errors and explicit unknown information.
    pub warnings: Vec<Finding>,
}

/// A heading-derived section; missing heading levels are not synthesized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// The heading block's ID. Repeated titles have distinct section IDs.
    pub section_id: String,
    /// Closest preceding shallower heading in this lexical scope.
    pub parent_section_id: Option<String>,
    /// Actual Markdown heading level, from one through six.
    pub level: u8,
    /// Derived heading text, not invented metadata.
    pub title: String,
    /// Ancestor titles followed by this section's title.
    pub heading_path: Vec<String>,
}

/// What accounts for an original source-syntax range, not a normalized-text range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRole {
    /// Syntax contributing to a parsed textual value; may include escapes or inline delimiters.
    ParsedContent,
    /// Container delimiters, attributes, whitespace and other parser-recognized syntax.
    StructuralSyntax,
    /// Metadata, link destinations/titles, or reference definitions retained in typed fields.
    MetadataOrReference,
    /// Exact raw/fallback syntax, explicitly retained without interpreting its semantics.
    Unsupported,
    /// A source contribution lacking a representation; blocks document acceptance.
    Unaccounted,
}

/// One nonoverlapping segment in the exhaustive original-byte accounting ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAccounting {
    /// Inclusive start/exclusive end in the immutable original Markdown.
    pub source_span: SourceSpan,
    /// How this syntax is represented; never a statement about extraction accuracy.
    pub role: SourceRole,
    /// Innermost owning block, or null for inter-block whitespace/unaccounted source.
    pub block_id: Option<String>,
}
