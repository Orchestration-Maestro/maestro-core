//! A compound question ranked part by part. The whole question is ranked
//! as before; each part is also ranked, with its bridge's words, against
//! its own text, under the one admission: the same generation, scope
//! snapshot, cutoffs and configuration. The handoff is the whole question's,
//! with each part's best passage added; assembly reserves the whole
//! question's best passage, then each part's. A part that
//! ranks nothing or fails adds nothing, so the whole question's result
//! stands; a part that sees the caller's rights change fails the search.

use super::{
    admission::AdmittedSearch,
    intent_routes::expand,
    question_parts::{PartRecord, PartsRecord, QuestionParts, QuestionSplit, QuestionSplitter},
    ranked,
    relation_splitter::{MAX_PARTS, RelationSplitter},
    request::{EvidenceInput, SearchContext, SearchError, SearchRequest},
    rerank::Ranked,
    route_search::FUSION_POOL,
};
use crate::query::understand;
use maestro_kernel::{evidence::RouteStatus, gateway::ModelPort};
use std::{
    collections::HashSet,
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
};
use tokio::time::Instant;

/// The reason of a part that ranked nothing.
const PART_RANKED_NOTHING: &str = "part_ranked_nothing";
/// The reason of a part whose ranking failed.
const PART_FAILED: &str = "part_search_failed";

/// The admitted question's split, when its configuration asks for parts.
pub(super) fn parts<P>(
    context: &SearchContext<'_, P>,
    admitted: &AdmittedSearch,
) -> Option<QuestionSplit> {
    (admitted.configuration.question_parts == QuestionParts::Split).then(|| {
        context.question_splitter.as_deref().map_or_else(
            || RelationSplitter.split(&admitted.understood),
            |splitter| splitter.split(&admitted.understood),
        )
    })
}

/// Ranks the whole question and, when `split` has two parts or more, each
/// part, and records what each part did.
pub(super) async fn search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    split: QuestionSplit,
) -> Result<EvidenceInput, SearchError> {
    let parts = bounded(split.parts);
    if parts.len() < 2 {
        let mut input = ranked::search(context, request, admitted).await?;
        input.observations.question_parts = Some(PartsRecord {
            whole: None,
            parts: Vec::new(),
            unsplit: split.unsplit,
        });
        return Ok(input);
    }
    let passes = parts
        .iter()
        .map(|part| part_pass(context, request, &admitted, part))
        .collect();
    let (whole, passes) = tokio::join!(
        Box::pin(ranked::search(context, request, admitted.clone())),
        join_all(passes),
    );
    let mut input = whole?;
    let whole = input
        .ranked
        .first()
        .map(|best| best.candidate.fused.chunk_id.clone());
    let mut records = Vec::with_capacity(passes.len());
    let mut bests = Vec::with_capacity(passes.len());
    for pass in passes {
        let (record, best) = pass?;
        records.push(record);
        bests.extend(best);
    }
    let reserved = whole
        .iter()
        .chain(records.iter().filter_map(|record| record.best.as_ref()))
        .cloned()
        .collect::<HashSet<_>>();
    add_bests(
        &mut input.ranked,
        &mut input.observations.reranked_chunk_ids,
        bests,
        &reserved,
    );
    input.observations.question_parts = Some(PartsRecord {
        whole,
        parts: records,
        unsplit: split.unsplit,
    });
    Ok(input)
}

/// Appends each of `bests` that `ranked` lacks, and its chunk to `order`,
/// within the fusion pool's bound: at the bound, a best displaces the
/// lowest candidate that `reserved` does not hold.
pub(super) fn add_bests(
    ranked: &mut Vec<Ranked>,
    order: &mut Vec<String>,
    bests: Vec<Ranked>,
    reserved: &HashSet<String>,
) {
    for best in bests {
        let chunk = &best.candidate.fused.chunk_id;
        if ranked
            .iter()
            .any(|ranked| &ranked.candidate.fused.chunk_id == chunk)
        {
            continue;
        }
        if ranked.len() >= FUSION_POOL {
            let Some(lowest) = ranked
                .iter()
                .rposition(|ranked| !reserved.contains(&ranked.candidate.fused.chunk_id))
            else {
                continue;
            };
            let displaced = ranked.remove(lowest);
            order.retain(|chunk| chunk != &displaced.candidate.fused.chunk_id);
        }
        order.push(chunk.clone());
        ranked.push(best);
    }
}

/// `parts` without blanks or repeats, the ones past [`MAX_PARTS`] joined to
/// the last.
fn bounded(parts: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::with_capacity(MAX_PARTS);
    for part in parts {
        let part = part.trim();
        if part.is_empty() || kept.iter().any(|kept| kept == part) {
            continue;
        }
        match kept.get_mut(MAX_PARTS - 1) {
            Some(last) => {
                last.push(' ');
                last.push_str(part);
            }
            None => kept.push(part.to_owned()),
        }
    }
    kept
}

/// Ranks `part`, bridged, as one more search of the admitted question: its
/// record and its best passage, or the search's failure when the caller's
/// rights changed.
async fn part_pass<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
    admitted: &AdmittedSearch,
    part: &str,
) -> Result<(PartRecord, Option<Ranked>), SearchError> {
    // The bridge runs before the part's routes, so it keeps half of their
    // time for them.
    let now = Instant::now();
    let cutoff = now + admitted.cutoffs.routes_end.saturating_duration_since(now) / 2;
    let (bridge, bridge_status) = match context.part_bridge.as_deref() {
        None => (String::new(), None),
        Some(bridge) => match expand(
            Some(bridge),
            &understand(part),
            admitted.configuration.intent_deadline_ms,
            cutoff,
        )
        .await
        {
            Ok(expansion) => (expansion.keywords, Some(RouteStatus::Ok)),
            Err(failure) => (
                String::new(),
                Some(RouteStatus::Unavailable(failure.code().to_owned())),
            ),
        },
    };
    let text = if bridge.is_empty() {
        part.to_owned()
    } else {
        format!("{part} {bridge}")
    };
    let part_request = SearchRequest {
        text: &text,
        ..*request
    };
    let mut record = PartRecord {
        text: part.to_owned(),
        bridge,
        bridge_status,
        status: RouteStatus::Ok,
        best: None,
    };
    let best = match Box::pin(ranked::search(
        context,
        &part_request,
        admitted.for_text(&text),
    ))
    .await
    {
        Ok(input) => input.ranked.into_iter().next(),
        Err(SearchError::PermissionsChanged) => return Err(SearchError::PermissionsChanged),
        Err(_) => {
            record.status = RouteStatus::Unavailable(PART_FAILED.to_owned());
            return Ok((record, None));
        }
    };
    match &best {
        Some(best) => record.best = Some(best.candidate.fused.chunk_id.clone()),
        None => record.status = RouteStatus::Unavailable(PART_RANKED_NOTHING.to_owned()),
    }
    Ok((record, best))
}

/// The outputs of `futures`, run concurrently, in their order.
async fn join_all<F: Future>(futures: Vec<F>) -> Vec<F::Output> {
    let mut futures = futures
        .into_iter()
        .map(Box::pin)
        .collect::<Vec<Pin<Box<F>>>>();
    let mut outputs = futures.iter().map(|_| None).collect::<Vec<_>>();
    poll_fn(|context| {
        let mut pending = false;
        for (future, output) in futures.iter_mut().zip(&mut outputs) {
            if output.is_none() {
                match future.as_mut().poll(context) {
                    Poll::Ready(value) => *output = Some(value),
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    outputs.into_iter().flatten().collect()
}
