//! The candidate stage of a search: exact loading, optional enrichment,
//! the rerank and the rank policies, in the order every search runs them.

use super::{
    admission::AdmittedSearch,
    candidates::{self, Failure},
    fusion::Fused,
    rank_policy,
    request::SearchConfiguration,
    rerank::{Ranked, Reranker, rerank_candidates},
    route_execution::route_outcome,
};
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::ModelPort,
    store::Database,
    telemetry::{span, stage::Count},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

/// The routes' fused list and the revisions each route claimed.
pub(super) struct Pool {
    /// The single fused list in its RRF order.
    pub(super) fused: Vec<Fused>,
    /// Every route revision that must agree with each kernel chunk.
    pub(super) expected_revisions: HashMap<String, Vec<String>>,
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
        .instrument(rerank_candidates(
            &admitted.understood,
            loaded.candidates,
            reranker,
            configuration
                .rerank_enabled
                .then_some(configuration.rerank_depth),
            admitted.cutoffs.setup,
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

/// Blends, applies the soft prior, then enforces final top-ten demotion bounds.
pub(super) fn apply_rank_policies(
    ranked: &mut [Ranked],
    fused_ids: &[String],
    configuration: SearchConfiguration,
    penalized: &BTreeSet<String>,
) {
    rank_policy::blend(
        ranked,
        fused_ids,
        configuration.rerank_blend,
        configuration.rrf_k,
    );
    configuration.section_prior.apply(ranked, penalized);
    rank_policy::cap_demotion(ranked, fused_ids, configuration.rerank_demotion_cap);
}
