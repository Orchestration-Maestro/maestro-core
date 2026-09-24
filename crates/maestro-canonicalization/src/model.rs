//! Database-independent provenance and versioning contract.
use crate::{Block, Link};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Half-open UTF-8 byte range into the entire unchanged Markdown, not rendered text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Inclusive byte offset.
    pub start: usize,
    /// Exclusive byte offset.
    pub end: usize,
}
impl SourceSpan {
    /// Check ordering, bounds and UTF-8 boundaries against the reference bytes.
    #[must_use]
    pub fn is_valid(self, markdown: &str) -> bool {
        self.start <= self.end
            && self.end <= markdown.len()
            && markdown.is_char_boundary(self.start)
            && markdown.is_char_boundary(self.end)
    }
}

/// Supplied provenance, with unknown values represented as null rather than inferred.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceMetadata {
    /// Original URL, document identifier or path supplied by the source.
    pub source_reference: Option<String>,
    /// Supplied title; headings are not used to fabricate missing metadata.
    pub title: Option<String>,
    /// Supplied language identifier; no language detection is performed.
    pub language: Option<String>,
    /// Supplied extraction details, retained without inventing an extractor history.
    pub extraction: Option<Value>,
    /// Opaque supplied policy, never inferred from availability or URL.
    pub access_policy: Option<Value>,
    /// Uninterpreted metadata; `markdown_frontmatter` is reserved for the original YAML map.
    pub extra: BTreeMap<String, Value>,
}

/// Recorded Markdown extension choices. Smart punctuation is always disabled.
/// YAML frontmatter support is fixed and recorded in the parser profile version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is an independent, serialized parser switch that identities include"
)]
pub struct ParserOptions {
    /// Recognize pipe tables with headers and alignment.
    pub tables: bool,
    /// Recognize named footnote references and definitions.
    pub footnotes: bool,
    /// Recognize strikethrough without dropping the deletion meaning.
    pub strikethrough: bool,
    /// Recognize checked and unchecked task-list markers.
    pub tasklists: bool,
    /// Preserve explicit heading IDs, classes and attributes.
    pub heading_attributes: bool,
    /// Recognize definition terms and descriptions.
    pub definition_lists: bool,
    /// Preserve inline and display math as uninterpreted mathematical text.
    pub math: bool,
    /// Recognize GFM blockquote alerts.
    pub gfm: bool,
}
impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            tables: true,
            footnotes: true,
            strikethrough: true,
            tasklists: true,
            heading_attributes: true,
            definition_lists: true,
            math: true,
            gfm: true,
        }
    }
}

/// Resolution observation, not an authorization or ownership claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    /// A local file exists in the allowed root.
    Available,
    /// The local file is absent.
    Missing,
    /// No trustworthy resolution observation is available.
    Unchecked,
    /// An external destination; never fetched by this stage.
    Remote,
    /// A same-document fragment; anchor existence is not inferred.
    Fragment,
    /// A local destination escapes the allowed asset root.
    OutsideRoot,
}

/// A link or image destination and its location in the Markdown reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetReference {
    /// Original parsed destination, including query and fragment when present.
    pub destination: String,
    /// Availability observation from the explicit asset inventory.
    pub status: AssetStatus,
    /// Span of the original reference syntax, not the rendered label.
    pub source_span: SourceSpan,
}

/// Original-document coordinates supplied by an extractor, not inferred here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginalLocation {
    /// Source to which the coordinates refer, or null when unavailable.
    pub source_reference: Option<String>,
    /// Supplied one-based page number. Markdown never supplies a PDF page number.
    pub page: Option<u32>,
    /// Original locator or bounding-box representation, retained as supplied.
    pub locator: Option<Value>,
}

/// An existing extractor block retained alongside the Markdown-derived structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractorBlock {
    /// Unique identifier supplied by the extractor or input adapter.
    pub extractor_id: String,
    /// Supplied anchors in the completed Markdown; empty means unavailable.
    pub markdown_spans: Vec<SourceSpan>,
    /// Supplied original-document coordinates; empty means unavailable.
    pub original_locations: Vec<OriginalLocation>,
    /// Original extractor structure, retained verbatim as JSON, never flattened.
    pub structured_content: Value,
}

/// Explicit inputs to the pure canonicalizer. Equal inputs yield equal outputs.
#[derive(Debug, Clone)]
pub struct CanonicalizeInput<'a> {
    /// Complete reference Markdown, including any frontmatter and original line endings.
    pub markdown: &'a str,
    /// Stable local reference used only as the identity fallback, not content equality.
    pub identity_key: &'a str,
    /// Explicit identity override. Recommended when source paths can move.
    pub document_id: Option<&'a str>,
    /// Available source metadata. Conflicting frontmatter values cause validation failure.
    pub metadata: SourceMetadata,
    /// Run IDs, processing timestamps and other operational observations, excluded from identities.
    /// Actual source provenance belongs in `metadata`, even if its keys have similar names.
    pub operational_metadata: BTreeMap<String, Value>,
    /// Existing extractor structures/mappings to preserve and anchor.
    pub extractor_blocks: Vec<ExtractorBlock>,
    /// Explicit Markdown extension selection.
    pub parser_options: ParserOptions,
    /// Local asset observations; the library performs no filesystem I/O.
    pub assets: BTreeMap<String, AssetStatus>,
}
impl<'a> CanonicalizeInput<'a> {
    /// Start with unknown provenance, default extensions and unchecked local assets.
    #[must_use]
    pub fn new(markdown: &'a str, identity_key: &'a str) -> Self {
        Self {
            markdown,
            identity_key,
            document_id: None,
            metadata: SourceMetadata::default(),
            operational_metadata: BTreeMap::new(),
            extractor_blocks: Vec::new(),
            parser_options: ParserOptions::default(),
            assets: BTreeMap::new(),
        }
    }
}

/// Whether the document is structurally trustworthy for subsequent stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    /// All implemented checks passed with no findings.
    Valid,
    /// Recoverable issues remain explicit; no blocking errors were detected.
    ValidWithWarnings,
    /// Blocking issues prevent treating the result as a trustworthy canonical document.
    Failed,
}
/// Finding severity; an error fails the document without discarding the original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A recoverable issue or an explicitly unverified property.
    Warning,
    /// A failed invariant or untrustworthy input.
    Error,
}
/// Structured validation evidence, including errors despite the `warnings` container name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Recoverable warning or blocking error.
    pub severity: Severity,
    /// Machine-readable finding category.
    pub code: String,
    /// Human-readable explanation, without attempting a repair.
    pub message: String,
    /// Affected canonical block, if one can be identified.
    pub block_id: Option<String>,
    /// Affected Markdown spans, or empty when unavailable.
    pub source_spans: Vec<SourceSpan>,
}

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
    pub source_accounting: Vec<crate::SourceAccounting>,
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
