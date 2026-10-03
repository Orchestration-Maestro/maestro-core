//! Strict, digest-bound proof labels for construction and answer evaluation.

use maestro_kernel::artifact::Digest;
use std::{collections::BTreeMap, error::Error, fmt};

/// Evaluation phase determines whether missing independent review is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Editable development labels.
    Draft,
    /// Frozen acceptance labels.
    Frozen,
}
/// Safe source slice returned by the caller's authority store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Original {
    /// Pinned revision.
    pub revision_id: String,
    /// Exact UTF-8 source bytes.
    pub bytes: Vec<u8>,
}
/// Label question category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    /// Dependency relation.
    Dependency,
    /// Single relation/default.
    Relationship,
    /// Connected multi-relation proof.
    MultiHop,
    /// Version-qualified claim.
    VersionDifference,
    /// No supported answer.
    Unanswerable,
}
/// Refusal codes are fixed and safe to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelCode {
    /// Invalid JSON/schema.
    Malformed,
    /// Unsafe stable ID.
    UnsafeId,
    /// Expected suite item absent.
    MissingLabel,
    /// Unknown suite question.
    UnknownItem,
    /// Duplicate question.
    DuplicateItem,
    /// Duplicate independent family.
    DuplicateFamily,
    /// Input digest differs.
    DigestMismatch,
    /// Unknown source reference or digest.
    UnknownSource,
    /// Invalid UTF-8 boundary.
    SpanOffBoundary,
    /// Span outside text or empty.
    SpanOutOfRange,
    /// Invalid source quote digest.
    QuoteMismatch,
    /// Missing reviewer record.
    MissingReview,
    /// Reviewer flagged without ruling.
    UnresolvedReview,
    /// Owner rejected claim.
    RejectedReview,
    /// Invalid predicate or proof structure.
    Vocabulary,
    /// Missing source anchor.
    MissingAnchor,
    /// Suite language differs.
    LanguageMismatch,
    /// Answerability differs from suite.
    Answerability,
    /// Missing reason for unanswerable item.
    UnanswerableRationale,
}
impl LabelCode {
    /// Stable safe external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::UnsafeId => "unsafe_id",
            Self::MissingLabel => "missing_label",
            Self::UnknownItem => "unknown_item",
            Self::DuplicateItem => "duplicate_item",
            Self::DuplicateFamily => "duplicate_family",
            Self::DigestMismatch => "digest_mismatch",
            Self::UnknownSource => "unknown_source",
            Self::SpanOffBoundary => "span_off_boundary",
            Self::SpanOutOfRange => "span_out_of_range",
            Self::QuoteMismatch => "quote_mismatch",
            Self::MissingReview => "missing_review",
            Self::UnresolvedReview => "unresolved_review",
            Self::RejectedReview => "rejected_review",
            Self::Vocabulary => "vocabulary",
            Self::MissingAnchor => "missing_anchor",
            Self::LanguageMismatch => "language_mismatch",
            Self::Answerability => "answerability",
            Self::UnanswerableRationale => "unanswerable_rationale",
        }
    }
}
/// Safe line-scoped refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelError {
    /// Fixed code.
    pub code: LabelCode,
    /// One-based JSONL line (zero means file-level).
    pub line: usize,
    /// Optional safe item id.
    pub item: Option<String>,
}
impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code.as_str())?;
        if self.line > 0 {
            write!(f, " at line {}", self.line)?;
        }
        if let Some(id) = &self.item {
            write!(f, ", item {id}")?;
        }
        Ok(())
    }
}
impl Error for LabelError {}
/// Error retaining authority callback error type.
#[derive(Debug)]
pub enum CheckError<E> {
    /// Label refusal.
    Label(LabelError),
    /// Authority/source lookup failed.
    Source(E),
}
/// Validated proof labels and aggregate counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLabels {
    /// Checked questions.
    pub items: Vec<CheckedItem>,
    /// Safe label totals.
    pub summary: LabelSummary,
}
/// A checked item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedItem {
    /// Question id.
    pub id: String,
    /// Independent family id.
    pub family: String,
    /// Kind.
    pub kind: QuestionKind,
    /// Alternatives, each all required anchors.
    pub proofs: Vec<Vec<Located>>,
}
/// Safe totals for checked input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSummary {
    /// Total labels.
    pub items: usize,
    /// Answerable count.
    pub answerable: usize,
    /// Unanswerable count.
    pub unanswerable: usize,
    /// Independent families.
    pub families: usize,
    /// Relation links.
    pub links: usize,
    /// Unique anchors.
    pub anchors: usize,
    /// Draft labels without review.
    pub unreviewed: usize,
    /// Digest of exact JSONL bytes.
    pub digest: Digest,
    /// Counts by safe kind.
    pub kinds: BTreeMap<String, usize>,
    /// Counts by language.
    pub languages: BTreeMap<String, usize>,
}

/// A source span that must be delivered in full.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Located {
    /// The pinned source revision.
    pub revision_id: String,
    /// Half-open UTF-8 byte span.
    pub span: [usize; 2],
}
