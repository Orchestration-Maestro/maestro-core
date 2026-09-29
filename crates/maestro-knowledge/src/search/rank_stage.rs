//! The candidate stage of a search: exact loading, optional enrichment,
//! the rerank and the rank policies, in the order every search runs them.

use super::{
    admission::AdmittedSearch,
    candidates::{self, Failure, Penalized},
    fusion::Fused,
    rank_policy,
    request::SearchConfiguration,
    rerank::{Candidate, Ranked, Reranker, rerank_candidates},
    route_execution::route_outcome,
};
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::ModelPort,
    store::Database,
    telemetry::{span, stage::Count},
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    num::NonZeroUsize,
    sync::Arc,
};

/// The routes' fused list and the revisions each route claimed.
pub(super) struct Pool {
    /// The single fused list in its RRF order.
    pub(super) fused: Vec<Fused>,
    /// Every route revision that must agree with each kernel chunk.
    pub(super) expected_revisions: HashMap<String, Vec<String>>,
    /// Candidates reranked beyond the configured depth.
    pub(super) rerank_extra: usize,
    /// Scores an earlier rerank of this search gave, by chunk ID.
    pub(super) known_scores: HashMap<String, f64>,
}

/// The final order and what the stage observed on the way.
#[derive(Debug)]
pub(super) struct Ranking {
    /// Candidates in their final order.
    pub(super) ranked: Vec<Ranked>,
    /// The rerank's availability.
    pub(super) status: RouteStatus,
    /// Source-loading wall time of the optional enrichment.
    pub(super) source_load_micros: u64,
    /// Candidates that kept their indexed chunk under bounded context.
    pub(super) fallbacks: Vec<String>,
    /// Candidates whose optional enrichment was unavailable.
    pub(super) context_unavailable: usize,
}

impl Ranking {
    /// The known gap to report when optional enrichment was unavailable.
    pub(super) fn context_gap(&self) -> Option<String> {
        (self.context_unavailable > 0).then(|| {
            format!(
                "reranker context unavailable for {} candidates; their chunk text was used",
                self.context_unavailable
            )
        })
    }
}

/// Loads the pool's exact candidates, reranks them until the setup cutoff
/// and applies the configured rank policies.
///
/// # Errors
///
/// Returns the candidate failure when the exact texts cannot be loaded.
pub(super) async fn rank<P: ModelPort>(
    database: Arc<Database>,
    reranker: Option<&Reranker<'_, P>>,
    admitted: &AdmittedSearch,
    query: &str,
    pool: Pool,
) -> Result<Ranking, Failure> {
    let configuration = admitted.configuration;
    let loaded = candidates::load(
        database,
        candidates::Request {
            generation: admitted.generation.clone(),
            scopes: admitted.scopes.as_ref().clone(),
            version: admitted.version.clone(),
            fused: pool.fused,
            expected_revisions: pool.expected_revisions,
            deadline: admitted.cutoffs.work,
            context_deadline: admitted.cutoffs.enrichment(),
            configuration,
            query: query.to_owned(),
            source_classes: admitted.source_classes.clone(),
        },
    )
    .await?;
    let fused_ids = loaded
        .candidates
        .iter()
        .map(|item| item.fused.chunk_id.clone())
        .collect::<Vec<_>>();
    let stage = span::rerank();
    let (mut ranked, status) = stage
        .instrument(rerank_reusing(
            admitted,
            loaded.candidates,
            reranker,
            pool.rerank_extra,
            &pool.known_scores,
        ))
        .await;
    apply_rank_policies(&mut ranked, &fused_ids, configuration, &loaded.penalized);
    stage.count(Count::Candidates, ranked.len());
    stage.finish(route_outcome(&status));
    Ok(Ranking {
        ranked,
        status,
        source_load_micros: loaded.source_load_micros,
        fallbacks: loaded.fallbacks,
        context_unavailable: loaded.context_unavailable,
    })
}

/// Reranks the configured depth and `extra` more candidates until the
/// setup cutoff, as [`rerank_candidates`] does, but sends the reranker only
/// those without a score in `known`: a second pass scores only what the
/// first did not.
async fn rerank_reusing<P: ModelPort>(
    admitted: &AdmittedSearch,
    candidates: Vec<Candidate>,
    reranker: Option<&Reranker<'_, P>>,
    extra: usize,
    known: &HashMap<String, f64>,
) -> (Vec<Ranked>, RouteStatus) {
    let configuration = admitted.configuration;
    let (understood, deadline) = (&admitted.understood, admitted.cutoffs.setup);
    let depth = configuration
        .rerank_enabled
        .then(|| configuration.rerank_depth.saturating_add(extra));
    let Some(head) = depth.filter(|_| !known.is_empty() && !candidates.is_empty()) else {
        return rerank_candidates(understood, candidates, reranker, depth, deadline).await;
    };
    let (fresh, mut items): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            let score = known
                .get(&candidate.fused.chunk_id)
                .copied()
                .filter(|_| index < head.get());
            (index, Ranked { candidate, score })
        })
        .partition(|(index, item)| *index < head.get() && item.score.is_none());
    let mut status = RouteStatus::Ok;
    if let Some(count) = NonZeroUsize::new(fresh.len()) {
        let indices = fresh
            .iter()
            .map(|(index, item)| (item.candidate.fused.chunk_id.clone(), *index))
            .collect::<HashMap<_, _>>();
        let candidates = fresh.into_iter().map(|(_, item)| item.candidate).collect();
        let (ranked, fresh_status) =
            rerank_candidates(understood, candidates, reranker, Some(count), deadline).await;
        status = fresh_status;
        items.extend(ranked.into_iter().map(|item| {
            let index = indices.get(&item.candidate.fused.chunk_id).copied();
            (index.unwrap_or(usize::MAX), item)
        }));
    }
    if status != RouteStatus::Ok {
        // As a failed rerank does: every candidate in fused order.
        for (_, item) in &mut items {
            item.score = None;
        }
    }
    items.sort_by(by_score_then_fused);
    (items.into_iter().map(|(_, item)| item).collect(), status)
}

/// The rerank's order: scored candidates by descending score, then the
/// rest; ties keep the fused order.
fn by_score_then_fused(
    (left_index, left): &(usize, Ranked),
    (right_index, right): &(usize, Ranked),
) -> Ordering {
    match (left.score, right.score) {
        (Some(left_score), Some(right_score)) => right_score
            .total_cmp(&left_score)
            .then_with(|| left_index.cmp(right_index)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left_index.cmp(right_index),
    }
}

/// Blends, applies the soft priors in one demotion, then enforces final
/// top-ten demotion bounds.
pub(super) fn apply_rank_policies(
    ranked: &mut [Ranked],
    fused_ids: &[String],
    configuration: SearchConfiguration,
    penalized: &Penalized,
) {
    rank_policy::blend(
        ranked,
        fused_ids,
        configuration.rerank_blend,
        configuration.rrf_k,
    );
    let mut multipliers = BTreeMap::new();
    for (multiplier, ids) in [
        (configuration.section_prior.multiplier(), &penalized.section),
        (configuration.source_prior.multiplier(), &penalized.source),
    ] {
        let Some(multiplier) = multiplier else {
            continue;
        };
        for id in ids {
            *multipliers.entry(id.clone()).or_insert(1.0) *= multiplier;
        }
    }
    rank_policy::demote(ranked, &multipliers);
    rank_policy::cap_demotion(ranked, fused_ids, configuration.rerank_demotion_cap);
}
