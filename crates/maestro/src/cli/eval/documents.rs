//! The documents a ladder row scores: a search's ranked documents, the first
//! [`RANKED_DOCUMENTS`] after reranking, and the documents of its evidence
//! bundle in retrieval rank.

use maestro_kernel::evidence::Bundle;
use std::collections::BTreeMap;

/// The most documents a search's ranked list holds: the floors score its
/// first 10.
const RANKED_DOCUMENTS: usize = 10;

/// The distinct documents of `order`, the chunks after reranking, or in fused
/// order when no rerank ran, before evidence assembly: each at its best
/// chunk's rank, the first [`RANKED_DOCUMENTS`]. `document_of` names each
/// chunk's document; a chunk it does not know is skipped.
pub(super) fn ranked_documents<'set>(
    order: &[String],
    document_of: impl Fn(&str) -> Option<&'set str>,
) -> Vec<String> {
    let mut ranked: Vec<String> = Vec::with_capacity(RANKED_DOCUMENTS);
    for document in order.iter().filter_map(|chunk| document_of(chunk)) {
        if ranked.len() == RANKED_DOCUMENTS {
            break;
        }
        if !ranked.iter().any(|seen| seen == document) {
            ranked.push(document.to_owned());
        }
    }
    ranked
}

/// The documents of `bundle`'s passages in retrieval rank, as `order`, the
/// chunks after reranking, ranks them: each passage at the best rank of its
/// chunks, a passage without a ranked chunk last, ties by passage number.
pub(super) fn bundle_documents(bundle: &Bundle, order: &[String]) -> Vec<String> {
    let rank: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .rev()
        .map(|(index, chunk)| (chunk.as_str(), index))
        .collect();
    let mut passages: Vec<(usize, u32, &str)> = bundle
        .passages
        .iter()
        .map(|passage| {
            let best = bundle
                .trace
                .iter()
                .filter(|trace| trace.n == passage.n)
                .flat_map(|trace| &trace.chunk_ids)
                .filter_map(|chunk| rank.get(chunk.as_str()).copied())
                .min()
                .unwrap_or(usize::MAX);
            (best, passage.n, passage.document_id.as_str())
        })
        .collect();
    passages.sort_unstable();
    passages
        .into_iter()
        .map(|(_, _, document)| document.to_owned())
        .collect()
}
