//! The prepared embedding input: its parts, primary fragments and table windows.
use super::mapping::{MappingRun, TextRange};
use serde::Serialize;

/// Explicit role of a piece of the complete embedding input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    /// Nonoverlapping primary rendered content.
    SourceContent,
    /// Repeated actual heading ancestry.
    HeadingContext,
    /// Corresponding column labels for selected table windows.
    TableHeaderContext,
    /// Full directly owned ancestor-item content, not nested item subtrees.
    ParentListContext,
    /// Supplied structural attributes or repeated meaning-bearing wrappers.
    StructuralContext,
    /// Enumerated generated layout, never an original quotation.
    FormattingSeparator,
}

/// A selection in one mapped source unit's derived coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Contribution {
    /// Index into the document's mapped units.
    pub unit_index: usize,
    /// Original unit-relative range, not part-relative or Markdown-relative.
    pub range: TextRange,
}

/// One verbatim-concatenated piece of the prepared input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputPart {
    /// Why this text is included.
    pub role: InputRole,
    /// Exact bytes sent to the qualified tokenizer.
    pub text: String,
    /// Location in the complete prepared input.
    pub prepared_range: TextRange,
    /// Source-unit selections; context copies do not add primary coverage.
    pub contributions: Vec<Contribution>,
    /// Mappings in this part's own text coordinates.
    pub mappings: Vec<MappingRun>,
    /// True only for generated formatting inside the primary body.
    pub body_layout: bool,
}

/// Honest continuation boundary, without claiming linguistic completeness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitKind {
    /// Complete natural contribution.
    Whole,
    /// Structural subdivision of a larger container.
    Structural,
    /// Punctuation-plus-whitespace heuristic, not sentence understanding.
    Sentence,
    /// Whitespace boundary.
    Whitespace,
    /// Unicode scalar boundary, not a byte cut or overlap.
    Scalar,
    /// Code-line boundary.
    CodeLine,
    /// Explicitly within a code line.
    CodeLineFragment,
    /// Explicitly within a table cell.
    CellFragment,
}

/// One primary unit selection; repeated contexts never become fragments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fragment {
    /// Original mapped unit selection.
    pub contribution: Contribution,
    /// Zero-based continuation ordinal for this unit across the document.
    pub part_ordinal: usize,
    /// Structural or textual boundary used for this piece.
    pub split: SplitKind,
}

/// Explicit table membership, including which columns were actually selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TableWindow {
    /// Canonical table identity.
    pub table_id: String,
    /// Canonical row identity, not invented flattened-row numbering.
    pub row_id: String,
    /// Zero-based row position, with the header at zero.
    pub row_index: usize,
    /// Selected zero-based columns; omitted columns are not fabricated blanks.
    pub columns: Vec<usize>,
}

/// Prepared chunk body, context, exact count and structural provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChunkContent {
    /// Actual owning section; a heading owns the section it introduces.
    pub section_id: Option<String>,
    /// Canonical structural containers, retaining all source relationships.
    pub container_ids: Vec<String>,
    /// Actual source heading titles; no metadata title is injected.
    pub heading_path: Vec<String>,
    /// Nonoverlapping primary contributions.
    pub fragments: Vec<Fragment>,
    /// Explicit row/column windows for table content.
    pub table_windows: Vec<TableWindow>,
    /// Primary body plus its generated layout, excluding contextual copies.
    pub body_text: String,
    /// Complete input serialization, with every separator represented.
    pub input_parts: Vec<InputPart>,
    /// Verbatim concatenation of all input parts.
    pub prepared_input: String,
    /// Complete-input count; only the verified native API certifies this value.
    pub token_count: usize,
}

/// Version of context and formatting preparation rules.
pub const PREPARATION_PROFILE: &str = "canonical-context-parts/v1";
