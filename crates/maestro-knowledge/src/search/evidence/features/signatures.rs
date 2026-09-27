//! Computes case-sensitive shingles and source features for evidence ranking.

use maestro_canonicalization::{BlockType, CanonicalDocument, ContentNode, Inline, InlineKind};
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

use super::super::sections::{block_span, contains, valid_span};

/// Source facts whose differences disable a diversity penalty.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SignaturePart {
    /// A whitespace token containing a Unicode numeric character.
    NumericToken(String),
    /// Exact source bytes from inline or fenced code.
    Code(String),
    /// Exact source bytes from a conditional or negation block.
    ConditionalBlock(String),
}

/// A source fact paired with its source-order position.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PositionedPart {
    /// UTF-8 byte offset of the fact in the original document.
    start: usize,
    /// Protected fact used for pairwise comparison.
    part: SignaturePart,
}

/// Precomputed features used to score evidence diversity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DiversityFeatures {
    /// Case-sensitive consecutive word shingles.
    shingles: BTreeSet<Vec<String>>,
    /// Literal version metadata for the candidate.
    version: Option<String>,
    /// Ordered protected source facts.
    signature: Vec<SignaturePart>,
}

/// Builds case-sensitive three-word shingles, or one whole shingle if shorter.
pub(crate) fn word_shingles(text: &str) -> BTreeSet<Vec<String>> {
    let words: Vec<_> = text.split_whitespace().map(str::to_owned).collect();
    if words.is_empty() {
        return BTreeSet::new();
    }
    let width = words.len().min(3);
    words.windows(width).map(<[String]>::to_vec).collect()
}

/// Extracts diversity shingles and protected facts from a complete extent.
pub(crate) fn diversity_features(
    markdown: &str,
    document: &CanonicalDocument,
    extent: Span,
    version: Option<String>,
) -> Result<DiversityFeatures, String> {
    if extent.start >= extent.end || !valid_span(extent, markdown) {
        return Err("section feature extent is invalid".to_owned());
    }
    let text = markdown
        .get(extent.start..extent.end)
        .ok_or_else(|| "section feature extent is not valid UTF-8".to_owned())?;
    Ok(DiversityFeatures {
        shingles: word_shingles(text),
        version,
        signature: protected_signature(markdown, document, extent, text)?,
    })
}

/// Returns Jaccard similarity unless versions or protected facts differ.
pub(crate) fn diversity_similarity(
    left: &DiversityFeatures,
    right: &DiversityFeatures,
) -> Result<f64, String> {
    if left.version != right.version || left.signature != right.signature {
        return Ok(0.0);
    }
    let intersection = left.shingles.intersection(&right.shingles).count();
    let union = left.shingles.len() + right.shingles.len() - intersection;
    if union == 0 {
        return Ok(0.0);
    }
    let numerator = u32::try_from(intersection)
        .map_err(|_| "shingle intersection exceeds supported count".to_owned())?;
    let denominator =
        u32::try_from(union).map_err(|_| "shingle union exceeds supported count".to_owned())?;
    Ok(f64::from(numerator) / f64::from(denominator))
}

/// Scores one candidate by ordinal relevance and its strongest redundancy.
pub(crate) fn mmr_score(rank: usize, max_similarity: f64) -> Result<f64, String> {
    if !max_similarity.is_finite() || !(0.0..=1.0).contains(&max_similarity) {
        return Err("MMR similarity is outside its valid range".to_owned());
    }
    let rank = u32::try_from(rank).map_err(|_| "MMR rank exceeds supported count".to_owned())?;
    let rank = rank
        .checked_add(1)
        .ok_or_else(|| "MMR rank exceeds supported count".to_owned())?;
    Ok(0.7 / f64::from(rank) - 0.3 * max_similarity)
}

/// Collects numeric, code, and conditional source facts in order.
fn protected_signature(
    markdown: &str,
    document: &CanonicalDocument,
    extent: Span,
    text: &str,
) -> Result<Vec<SignaturePart>, String> {
    let mut parts = Vec::new();
    add_numeric_tokens(text, extent.start, &mut parts);
    for block in &document.blocks {
        let Some(span) = block_span(block, markdown)? else {
            continue;
        };
        if !contains(extent, span) {
            continue;
        }
        let source = markdown
            .get(span.start..span.end)
            .ok_or_else(|| "canonical block span is not valid UTF-8".to_owned())?;
        if matches!(&block.block_type, BlockType::Code) {
            parts.push(PositionedPart {
                start: span.start,
                part: SignaturePart::Code(source.to_owned()),
            });
        }
        if !matches!(&block.block_type, BlockType::List) && has_condition_or_negation(source) {
            parts.push(PositionedPart {
                start: span.start,
                part: SignaturePart::ConditionalBlock(source.to_owned()),
            });
        }
        for node in &block.structured_content.children {
            if let ContentNode::Inline { inline } = node {
                add_inline_code(inline, markdown, extent, &mut parts)?;
            }
        }
    }
    parts.sort_by_key(|part| part.start);
    Ok(parts.into_iter().map(|part| part.part).collect())
}

/// Adds each numeric whitespace token with its absolute source offset.
fn add_numeric_tokens(text: &str, base: usize, parts: &mut Vec<PositionedPart>) {
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(token_start) = start.take() {
                add_numeric_token(text, base, token_start, index, parts);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        add_numeric_token(text, base, token_start, text.len(), parts);
    }
}

/// Records one numeric token when it contains a Unicode numeric character.
fn add_numeric_token(
    text: &str,
    base: usize,
    start: usize,
    end: usize,
    parts: &mut Vec<PositionedPart>,
) {
    let token = &text[start..end];
    if token.chars().any(char::is_numeric) {
        parts.push(PositionedPart {
            start: base + start,
            part: SignaturePart::NumericToken(token.to_owned()),
        });
    }
}

/// Adds source bytes for inline-code spans nested in a canonical block.
fn add_inline_code(
    inline: &Inline,
    markdown: &str,
    extent: Span,
    parts: &mut Vec<PositionedPart>,
) -> Result<(), String> {
    let span = inline.source_span;
    if !span.is_valid(markdown) {
        return Err("canonical inline span is invalid".to_owned());
    }
    let inline_range = Span {
        start: span.start,
        end: span.end,
    };
    if matches!(&inline.content, InlineKind::Code { .. }) && contains(extent, inline_range) {
        let source = markdown
            .get(span.start..span.end)
            .ok_or_else(|| "canonical inline span is not valid UTF-8".to_owned())?;
        parts.push(PositionedPart {
            start: span.start,
            part: SignaturePart::Code(source.to_owned()),
        });
    }
    for child in &inline.children {
        add_inline_code(child, markdown, extent, parts)?;
    }
    Ok(())
}

/// Detects the brief's explicit conditional and negation marker vocabulary.
pub(in crate::search::evidence) fn has_condition_or_negation(source: &str) -> bool {
    const MARKERS: &[&str] = &[
        "if",
        "unless",
        "when",
        "only",
        "except",
        "provided",
        "requires",
        "require",
        "must",
        "before",
        "after",
        "not",
        "no",
        "never",
        "without",
        "cannot",
        "neither",
        "nor",
        "si",
        "sauf",
        "lorsque",
        "quand",
        "uniquement",
        "avant",
        "après",
        "doit",
        "doivent",
        "nécessite",
        "ne",
        "pas",
        "jamais",
        "sans",
        "aucun",
        "aucune",
        "ni",
    ];
    let lowercase = source.to_lowercase();
    lowercase
        .split(|character: char| !character.is_alphabetic())
        .any(|word| MARKERS.contains(&word))
        || lowercase.split_whitespace().any(|word| {
            let word = word.trim_end_matches(|character: char| !character.is_alphanumeric());
            word.ends_with("n't") || word.ends_with("n’t")
        })
}
