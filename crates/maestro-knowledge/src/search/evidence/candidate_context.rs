//! Reuses authoritative evidence sections for reranker-only context.

use super::{
    source::{EvidenceSource, SourceCache},
    spans::{SeedSpan, SpanUnion},
};
use maestro_kernel::{chunk_set::Chunk, evidence::Span};
use std::collections::BTreeSet;

/// Returns the section heading path and optional whole-unit bounded source text.
/// An absent text means the original prepared chunk must be retained.
pub(in crate::search) fn context(
    source: &EvidenceSource,
    chunk: &Chunk,
    max_bytes: Option<usize>,
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
    let path = expansion.section_path.join(" / ");
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
    let text = source
        .markdown
        .get(span.start..span.end)
        .ok_or_else(|| "invalid context span".to_owned())?;
    Ok((path, Some(format!("{prefix}{text}"))))
}
