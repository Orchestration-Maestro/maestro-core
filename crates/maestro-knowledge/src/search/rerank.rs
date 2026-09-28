//! Reranks the head of a fused list without truncating candidate text.

use super::{
    deadline::{DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION},
    fusion::Fused,
};
use crate::query::Understood;
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::{ModelCard, ModelPort, Role, Room},
};
use std::{cmp::Ordering, num::NonZeroUsize, time::Duration};
use tokio::time::{Instant, timeout};

/// Reserved model tokens for the query and document's special tokens.
const SPECIAL_TOKENS: usize = 16;

/// Fixed public reranker failure categories; backend text never leaves the route.
#[derive(Clone, Copy)]
enum RerankFailure {
    /// The selected card does not have the reranker role.
    InvalidModelCard,
    /// The router refused tokenization or scoring.
    ModelUnavailable,
    /// The router returned the wrong number or a non-finite score.
    InvalidResponse,
    /// Tokenization or scoring exceeded the accepted deadline.
    DeadlineExceeded,
    /// The query or document cannot fit the card's context window.
    ContextLimit,
    /// Candidate window boundaries violate UTF-8 invariants.
    InvalidCandidate,
}

impl RerankFailure {
    /// Stable code exposed in the evidence bundle.
    fn code(self) -> &'static str {
        match self {
            Self::InvalidModelCard => "invalid_model_card",
            Self::ModelUnavailable => "model_unavailable",
            Self::InvalidResponse => "invalid_response",
            Self::DeadlineExceeded => DEADLINE_EXCEEDED,
            Self::ContextLimit => "context_limit",
            Self::InvalidCandidate => "invalid_candidate",
        }
    }
}

/// The reranking depth the T037 ladder measured best: depth 80 overran the
/// 1.5 s search budget.
pub(super) const DEFAULT_DEPTH: NonZeroUsize =
    NonZeroUsize::new(30).expect("default depth is nonzero");

/// A fused candidate and the prepared text read by the reranker.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The candidate's fused identity, score and route ranks.
    pub fused: Fused,
    /// The chunk's indexed text, without truncation.
    pub text: String,
}

/// The model port and reranker card used for one reranking operation.
#[derive(Debug)]
pub struct Reranker<'a, P> {
    /// The model gateway.
    pub port: &'a P,
    /// The card that defines the reranker's role and context size.
    pub card: &'a ModelCard,
}

/// One candidate in the returned order and its reranker score, when available.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    /// The fused candidate, including its separate fusion score.
    pub candidate: Candidate,
    /// The rerank score, absent for candidates beyond the depth or on failure.
    pub score: Option<f64>,
}

/// The reranker score of the first of `ranked`, the highest one; none when
/// rerank did not run.
#[must_use]
pub fn top_rerank_score(ranked: &[Ranked]) -> Option<f64> {
    ranked.first().and_then(|item| item.score)
}

/// The highest fused score of `ranked`, whatever their order.
#[must_use]
pub fn top_fused_score(ranked: &[Ranked]) -> Option<f64> {
    ranked
        .iter()
        .map(|item| item.candidate.fused.score)
        .reduce(f64::max)
}

/// Candidates in final order and whether reranking was available.
#[derive(Clone, Debug, PartialEq)]
pub struct Reranked {
    /// Reranked head followed by the unchanged fused tail.
    pub ranked: Vec<Ranked>,
    /// The reranking route's status.
    pub status: RouteStatus,
}

/// Reranks the first `depth` fused candidates, keeping the rest in fused order.
///
/// A deadline, port failure or invalid response returns all candidates in
/// their original fused order with no rerank scores and a fixed safe reason.
pub async fn rerank<P: ModelPort>(
    query: &str,
    candidates: Vec<Candidate>,
    reranker: &Reranker<'_, P>,
    depth: NonZeroUsize,
    deadline: Duration,
) -> Reranked {
    if reranker.card.fields().role != Role::Reranker {
        return unavailable(candidates, RerankFailure::InvalidModelCard);
    }

    let rerank_count = depth.get().min(candidates.len());
    if rerank_count == 0 {
        return Reranked {
            ranked: Vec::new(),
            status: RouteStatus::Ok,
        };
    }

    let response = timeout(deadline, async {
        let prepared = prepare_documents(query, &candidates, rerank_count, reranker).await?;
        if prepared.documents.is_empty() {
            return Ok(vec![None; rerank_count]);
        }

        let scores = reranker
            .port
            .rerank(reranker.card, Room::Free, query, &prepared.documents)
            .await
            .map_err(|_| RerankFailure::ModelUnavailable)?;
        if scores.len() != prepared.owners.len() || scores.iter().any(|score| !score.is_finite()) {
            return Err(RerankFailure::InvalidResponse);
        }

        let mut best_scores = vec![None; rerank_count];
        for (owner, score) in prepared.owners.into_iter().zip(scores) {
            let Some(best) = best_scores.get_mut(owner) else {
                return Err(RerankFailure::InvalidResponse);
            };
            *best = Some(best.map_or(score, |current: f64| current.max(score)));
        }
        Ok(best_scores)
    })
    .await;

    let scores = match response {
        Ok(Ok(scores)) => scores,
        Ok(Err(reason)) => return unavailable(candidates, reason),
        Err(_) => return unavailable(candidates, RerankFailure::DeadlineExceeded),
    };

    let mut ranked = candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| Ranked {
            candidate,
            score: scores.get(index).copied().flatten(),
        })
        .enumerate()
        .collect::<Vec<_>>();
    ranked.sort_by(
        |(left_index, left), (right_index, right)| match (left.score, right.score) {
            (Some(left_score), Some(right_score)) => right_score
                .partial_cmp(&left_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left_index.cmp(right_index)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => left_index.cmp(right_index),
        },
    );

    Reranked {
        ranked: ranked.into_iter().map(|(_, item)| item).collect(),
        status: RouteStatus::Ok,
    }
}

/// Documents sent to the model and their owning candidate indices.
struct PreparedDocuments {
    /// Prepared text, with one entry per reranker document.
    documents: Vec<String>,
    /// Candidate index corresponding to each document.
    owners: Vec<usize>,
}

/// Builds the model documents for the candidates inside the rerank depth.
async fn prepare_documents<P: ModelPort>(
    query: &str,
    candidates: &[Candidate],
    rerank_count: usize,
    reranker: &Reranker<'_, P>,
) -> Result<PreparedDocuments, RerankFailure> {
    let context_tokens = usize::try_from(reranker.card.fields().limits.context_tokens.get())
        .map_err(|_| RerankFailure::ContextLimit)?;
    let mut query_tokens = None;
    let mut prepared = PreparedDocuments {
        documents: Vec::new(),
        owners: Vec::new(),
    };

    for (owner, candidate) in candidates.iter().take(rerank_count).enumerate() {
        if query
            .len()
            .saturating_add(candidate.text.len())
            .saturating_add(SPECIAL_TOKENS)
            <= context_tokens
        {
            add_document(&mut prepared, owner, candidate.text.clone());
            continue;
        }

        let count = if let Some(count) = query_tokens {
            count
        } else {
            let count = reranker
                .port
                .tokenize(reranker.card, Room::Free, query)
                .await
                .map_err(|_| RerankFailure::ModelUnavailable)?
                .len();
            query_tokens = Some(count);
            count
        };
        let text_budget = context_tokens
            .checked_sub(count.saturating_add(SPECIAL_TOKENS))
            .ok_or(RerankFailure::ContextLimit)?;
        let text_tokens = reranker
            .port
            .tokenize(reranker.card, Room::Free, &candidate.text)
            .await
            .map_err(|_| RerankFailure::ModelUnavailable)?
            .len();
        if text_tokens <= text_budget {
            add_document(&mut prepared, owner, candidate.text.clone());
        } else {
            for window in split_text(&candidate.text, text_budget, reranker).await? {
                add_document(&mut prepared, owner, window);
            }
        }
    }

    Ok(prepared)
}

/// Adds one reranker document and records its candidate.
fn add_document(prepared: &mut PreparedDocuments, owner: usize, text: String) {
    prepared.documents.push(text);
    prepared.owners.push(owner);
}

/// Splits text into consecutive token-bounded windows, preferring whitespace.
async fn split_text<P: ModelPort>(
    text: &str,
    token_budget: usize,
    reranker: &Reranker<'_, P>,
) -> Result<Vec<String>, RerankFailure> {
    let mut windows = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let ends = text
            .get(start..)
            .ok_or(RerankFailure::InvalidCandidate)?
            .char_indices()
            .map(|(offset, character)| start + offset + character.len_utf8())
            .collect::<Vec<_>>();
        let mut low = 0;
        let mut high = ends.len();
        while low < high {
            let middle = low + (high - low) / 2;
            let end = ends
                .get(middle)
                .copied()
                .ok_or(RerankFailure::InvalidCandidate)?;
            let window = text
                .get(start..end)
                .ok_or(RerankFailure::InvalidCandidate)?;
            let count = reranker
                .port
                .tokenize(reranker.card, Room::Free, window)
                .await
                .map_err(|_| RerankFailure::ModelUnavailable)?
                .len();
            if count <= token_budget {
                low = middle + 1;
            } else {
                high = middle;
            }
        }

        let mut end = low
            .checked_sub(1)
            .and_then(|index| ends.get(index))
            .copied()
            .ok_or(RerankFailure::ContextLimit)?;
        let fitting = text
            .get(start..end)
            .ok_or(RerankFailure::InvalidCandidate)?;
        if let Some(whitespace_end) = fitting
            .char_indices()
            .filter_map(|(offset, character)| {
                character
                    .is_whitespace()
                    .then_some(start + offset + character.len_utf8())
            })
            .next_back()
        {
            end = whitespace_end;
        }

        let mut window = text
            .get(start..end)
            .ok_or(RerankFailure::InvalidCandidate)?;
        let mut count = reranker
            .port
            .tokenize(reranker.card, Room::Free, window)
            .await
            .map_err(|_| RerankFailure::ModelUnavailable)?
            .len();
        if count > token_budget {
            end = low
                .checked_sub(1)
                .and_then(|index| ends.get(index))
                .copied()
                .ok_or(RerankFailure::ContextLimit)?;
            window = text
                .get(start..end)
                .ok_or(RerankFailure::InvalidCandidate)?;
            count = reranker
                .port
                .tokenize(reranker.card, Room::Free, window)
                .await
                .map_err(|_| RerankFailure::ModelUnavailable)?
                .len();
            if count > token_budget {
                return Err(RerankFailure::ContextLimit);
            }
        }
        windows.push(window.to_owned());
        start = end;
    }

    Ok(windows)
}

/// Returns every candidate in fused order when reranking cannot complete.
fn unavailable(candidates: Vec<Candidate>, reason: RerankFailure) -> Reranked {
    Reranked {
        ranked: candidates
            .into_iter()
            .map(|candidate| Ranked {
                candidate,
                score: None,
            })
            .collect(),
        status: RouteStatus::Unavailable(reason.code().to_owned()),
    }
}

/// Reranks loaded candidates or returns the safe fused-order fallback.
pub(super) async fn rerank_candidates<P: ModelPort>(
    understood: &Understood,
    candidates: Vec<Candidate>,
    reranker: Option<&Reranker<'_, P>>,
    depth: Option<NonZeroUsize>,
    deadline: Instant,
) -> (Vec<Ranked>, RouteStatus) {
    if candidates.is_empty() {
        return (
            Vec::new(),
            RouteStatus::Unavailable(NO_FUSED_CANDIDATES.to_owned()),
        );
    }
    let Some(depth) = depth else {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
        );
    };
    let Some(reranker) = reranker else {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable("no reranker configured".to_owned()),
        );
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
        );
    }
    let result = rerank(
        &understood.normalized,
        candidates,
        reranker,
        depth,
        remaining,
    )
    .await;
    (result.ranked, result.status)
}

/// Keeps the fused order when no reranker can run.
fn fused_order(candidates: Vec<Candidate>) -> Vec<Ranked> {
    candidates
        .into_iter()
        .map(|candidate| Ranked {
            candidate,
            score: None,
        })
        .collect()
}

/// The rerank status reason when fusion gave no candidate to rerank.
pub const NO_FUSED_CANDIDATES: &str = "no fused candidates";
