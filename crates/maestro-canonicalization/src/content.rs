//! Typed natural blocks and nested inline content.
use crate::{AssetReference, SourceSpan};
use serde::{Deserialize, Serialize};

/// Natural Markdown block category, not a retrieval chunk type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    /// ATX or Setext heading.
    Heading,
    /// Prose paragraph.
    Paragraph,
    /// Ordered or unordered list container.
    List,
    /// List entry that can contain further blocks.
    ListItem,
    /// Fenced or indented code block.
    Code,
    /// Table container with column alignments.
    Table,
    /// Header row of a table.
    TableHead,
    /// Body row of a table.
    TableRow,
    /// A header or body cell.
    TableCell,
    /// Block quotation, optionally a GFM alert.
    BlockQuote,
    /// Named footnote content.
    FootnoteDefinition,
    /// Horizontal rule.
    ThematicBreak,
    /// Uninterpreted HTML retained with a warning.
    Html,
    /// YAML frontmatter retained with its original span.
    Metadata,
    /// Definition list container.
    DefinitionList,
    /// Term in a definition list.
    DefinitionTerm,
    /// Description in a definition list.
    DefinitionDescription,
    /// Link reference definition normally omitted from parser events.
    ReferenceDefinition,
    /// Exact source omitted from parser events, retained without guessed semantics.
    Raw,
}
/// Original Markdown code-block syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeKind {
    /// Backtick or tilde fence.
    Fenced,
    /// Indented `CommonMark` code.
    Indented,
}

/// Typed structural attributes; exact syntax remains in the source span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockAttributes {
    /// Heading structure and explicitly supplied attributes.
    Heading {
        /// Actual heading level, without inserting skipped levels.
        level: u8,
        /// Explicit Markdown heading anchor, or null.
        explicit_id: Option<String>,
        /// Explicit heading classes, in source order.
        classes: Vec<String>,
        /// Explicit key/value attributes, in source order.
        attributes: Vec<(String, Option<String>)>,
    },
    /// Prose paragraph.
    Paragraph,
    /// List container.
    List {
        /// Starting number; null denotes an unordered list.
        start: Option<u64>,
    },
    /// List entry containing ordered inline/block children.
    ListItem,
    /// Code structure; code bytes after syntax removal live in child text nodes.
    Code {
        /// Fenced or indented syntax.
        style: CodeKind,
        /// Full fence info string, without discarding extra labels.
        info: Option<String>,
        /// First info-string token, if supplied; never detected or guessed.
        language: Option<String>,
    },
    /// Table container.
    Table {
        /// One of none/left/center/right per column, in source order.
        alignments: Vec<String>,
    },
    /// Table header row.
    TableHead,
    /// Table body row.
    TableRow,
    /// Table cell; headers are distinguished by the parent row type.
    TableCell,
    /// Block quotation with optional GFM alert label.
    BlockQuote {
        /// Supplied note/tip/important/warning/caution marker, or null.
        alert: Option<String>,
    },
    /// Footnote definition.
    FootnoteDefinition {
        /// Named reference label.
        label: String,
    },
    /// Horizontal rule.
    ThematicBreak,
    /// Uninterpreted HTML block.
    Html,
    /// YAML frontmatter block.
    Metadata,
    /// Definition list container.
    DefinitionList,
    /// Definition list term.
    DefinitionTerm,
    /// Definition list description.
    DefinitionDescription,
    /// Uninterpreted source contribution retained instead of rejecting valid dialect syntax.
    Raw {
        /// Stable reason code describing why parser events did not represent the source.
        reason: String,
    },
    /// Source-positioned link reference definition.
    ReferenceDefinition {
        /// Parser-normalized reference label.
        label: String,
        /// Supplied destination, without fetching it.
        destination: String,
        /// Supplied optional title, empty when absent.
        title: String,
    },
}
impl BlockAttributes {
    /// Return the corresponding public block category.
    #[must_use]
    pub fn block_type(&self) -> BlockType {
        match self {
            Self::Heading { .. } => BlockType::Heading,
            Self::Paragraph => BlockType::Paragraph,
            Self::List { .. } => BlockType::List,
            Self::ListItem => BlockType::ListItem,
            Self::Code { .. } => BlockType::Code,
            Self::Table { .. } => BlockType::Table,
            Self::TableHead => BlockType::TableHead,
            Self::TableRow => BlockType::TableRow,
            Self::TableCell => BlockType::TableCell,
            Self::BlockQuote { .. } => BlockType::BlockQuote,
            Self::FootnoteDefinition { .. } => BlockType::FootnoteDefinition,
            Self::ThematicBreak => BlockType::ThematicBreak,
            Self::Html => BlockType::Html,
            Self::Metadata => BlockType::Metadata,
            Self::DefinitionList => BlockType::DefinitionList,
            Self::DefinitionTerm => BlockType::DefinitionTerm,
            Self::DefinitionDescription => BlockType::DefinitionDescription,
            Self::ReferenceDefinition { .. } => BlockType::ReferenceDefinition,
            Self::Raw { .. } => BlockType::Raw,
        }
    }
}
/// Structural attributes and ordered child nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredContent {
    /// Type-specific structure, without presenting raw source offsets as text offsets.
    pub attributes: BlockAttributes,
    /// Ordered interleaving preserves tight list text before/after nested blocks.
    pub children: Vec<ContentNode>,
}
/// An ordered child, either a block-arena reference or a nested inline tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentNode {
    /// Reference to a nested block.
    Block {
        /// Existing child block identifier in the same document revision.
        block_id: String,
    },
    /// Inline content stored in place.
    Inline {
        /// Typed inline tree with original byte spans.
        inline: Inline,
    },
}
/// Natural Markdown block with revision, section and exact source context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Deterministic within the document revision and parser configuration.
    pub block_id: String,
    /// Owning document revision; permits provenance even when a block is exported.
    pub revision_id: String,
    /// Structural block category.
    pub block_type: BlockType,
    /// Container block, or null for a document-root block.
    pub parent_block_id: Option<String>,
    /// Containing section; for headings, the shallower parent section.
    pub parent_section_id: Option<String>,
    /// Actual ancestor heading titles; a heading includes its own title.
    pub heading_path: Vec<String>,
    /// Source-syntax spans into the preserved Markdown, never normalized text.
    pub source_spans: Vec<SourceSpan>,
    /// Deterministic derived readable text, with code indentation and table boundaries.
    pub retrieval_text: String,
    /// Typed structure with ordered child references and inline trees.
    pub structured_content: StructuredContent,
    /// Links/images directly owned by this block, not duplicated from child blocks.
    pub asset_references: Vec<AssetReference>,
    /// Supplied extractor blocks whose Markdown anchors overlap this block.
    pub extractor_block_ids: Vec<String>,
}

/// Inline semantics. Presentation-only syntax is removed only in derived text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineKind {
    /// Parsed text, with `CommonMark` escape/entity handling.
    Text {
        /// Text value; its span still points into original source syntax.
        text: String,
    },
    /// Inline code, distinct from prose.
    Code {
        /// Parser-provided code value.
        text: String,
    },
    /// Emphasized child content.
    Emphasis,
    /// Strongly emphasized child content.
    Strong,
    /// Deleted content; deletion meaning is retained in derived text.
    Strikethrough,
    /// Superscript child content.
    Superscript,
    /// Subscript child content.
    Subscript,
    /// Link with a nested label.
    Link {
        /// Original parsed URL/path.
        destination: String,
        /// Optional title, empty when absent.
        title: String,
        /// Reference label, empty for inline links.
        reference: String,
        /// pulldown-cmark's reference/autolink/inline classification.
        link_type: String,
    },
    /// Image with nested alternative text.
    Image {
        /// Original parsed asset URL/path.
        destination: String,
        /// Optional image title, empty when absent.
        title: String,
        /// Reference label, empty for inline images.
        reference: String,
        /// pulldown-cmark's link classification.
        link_type: String,
    },
    /// Named footnote reference.
    FootnoteReference {
        /// Target definition label.
        label: String,
    },
    /// Soft line boundary retained as a newline in derived text.
    SoftBreak,
    /// Explicit Markdown line break.
    HardBreak,
    /// Task-list checkbox.
    TaskMarker {
        /// Whether the checkbox is checked.
        checked: bool,
    },
    /// HTML retained verbatim, not executed or heuristically stripped.
    Html {
        /// Original HTML text.
        raw: String,
    },
    /// Math retained without interpreting or evaluating it.
    Math {
        /// Mathematical source text.
        text: String,
        /// Whether this is display rather than inline math.
        display: bool,
    },
}
/// Inline node and children, all traceable to original Markdown syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inline {
    /// Typed inline semantics.
    pub content: InlineKind,
    /// Original source-syntax byte range.
    pub source_span: SourceSpan,
    /// Nested inline nodes in source order.
    pub children: Vec<Inline>,
}
/// Document-level index of links and image references, not graph relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// Block directly containing the reference.
    pub block_id: String,
    /// Parsed destination; not fetched or rewritten.
    pub destination: String,
    /// Optional source title, empty when absent.
    pub title: String,
    /// Derived visible label or alternative text.
    pub label: String,
    /// True for images, false for ordinary links.
    pub image: bool,
    /// Span of the original reference syntax.
    pub source_span: SourceSpan,
}
