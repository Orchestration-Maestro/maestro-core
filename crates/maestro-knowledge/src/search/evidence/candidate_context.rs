//! Reuses authoritative evidence sections for reranker-only context.

use super::{
    source::{EvidenceSource, SourceCache},
    spans::{SeedSpan, SpanUnion},
};
use maestro_canonicalization::{ChunkProfile, SourceSpan, indexed_title};
use maestro_kernel::{chunk_set::Chunk, evidence::Span};
use std::collections::BTreeSet;

/// What the chunk set a candidate comes from left out of its indexed text.
#[derive(Clone, Copy)]
pub(in crate::search) struct Indexing<'a> {
    /// The chunking profile, whose heading titles leave their labels out.
    pub(in crate::search) profile: ChunkProfile,
    /// The source's page chrome under that profile, sorted.
    pub(in crate::search) chrome: &'a [SourceSpan],
}

/// Returns the section heading path and optional whole-unit bounded source text,
/// both as `indexing`'s profile indexed them, without their page chrome.
/// An absent text means the original prepared chunk must be retained.
pub(in crate::search) fn context(
    source: &EvidenceSource,
    chunk: &Chunk,
    max_bytes: Option<usize>,
    indexing: Indexing<'_>,
) -> Result<(String, Option<String>), String> {
    SourceCache::validate_chunk_span(source, chunk.span)
        .map_err(|_| "invalid candidate span".to_owned())?;
    let union = SpanUnion {
        revision_id: chunk.revision_id.clone(),
        span: chunk.span,
        seeds: vec![SeedSpan {
            chunk_id: chunk.id.clone(),
            revision_id: chunk.revision_id.clone(),
            section_id: chunk.section_id.clone(),
            span: chunk.span,
            input_position: 0,
            score: None,
            routes: BTreeSet::new(),
        }],
    };
    let expansion = source.sections.expand(&union)?;
    let path = expansion
        .section_path
        .iter()
        .map(|title| indexed_title(indexing.profile, title))
        .collect::<Vec<_>>()
        .join(" / ");
    let Some(max_bytes) = max_bytes else {
        return Ok((path, None));
    };
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path}\n")
    };
    let budget = max_bytes.saturating_sub(prefix.len());
    let mut span = expansion.extent;
    if span.end - span.start > budget {
        let plan = expansion.window_plan(chunk.span)?;
        span = plan.mandatory;
        if span.end - span.start > budget {
            return Ok((path, None));
        }
        for neighbor in plan.neighbors() {
            let enlarged = Span {
                start: span.start.min(neighbor.start),
                end: span.end.max(neighbor.end),
            };
            if enlarged.end - enlarged.start <= budget {
                span = enlarged;
            }
        }
    }
    let text = without_chrome(&source.markdown, span, indexing.chrome)
        .ok_or_else(|| "invalid context span".to_owned())?;
    Ok((path, Some(format!("{prefix}{text}"))))
}

/// The text of `span` in `markdown` with the parts of `chrome`, sorted, it
/// holds cut out; none for a span that is not in `markdown`.
fn without_chrome(markdown: &str, span: Span, chrome: &[SourceSpan]) -> Option<String> {
    let mut text = String::new();
    let mut cursor = span.start;
    for cut in chrome {
        let start = cut.start.clamp(cursor, span.end);
        let end = cut.end.clamp(start, span.end);
        text.push_str(markdown.get(cursor..start)?);
        cursor = end;
    }
    text.push_str(markdown.get(cursor..span.end)?);
    Some(text)
}
