//! Independent additive routes over guarded model-generated retrieval inputs.

use super::{
    admission::AdmittedSearch,
    deadline::until,
    fusion::Route,
    intent::{Expansion, ExpansionFailure, IntentExpansion, QueryExpander},
    intent_guard::validate,
    query::Query,
    request::SearchContext,
    route_execution::{dense_outcome, lexical_outcome},
    routes::outcome::RouteOutcome,
};
use crate::{index::RetrievalProjectionPort, query::Understood};
use maestro_kernel::{evidence::RouteStatus, gateway::ModelPort};
use std::time::Duration;
use tokio::time::Instant;

/// Expanded route results and a stable expansion trace, absent when disabled.
pub(super) struct IntentRoutes {
    /// Independently weighted and traced dense/lexical expansion votes.
    pub outcomes: Vec<(Route, RouteOutcome)>,
    /// The expansion model/guard status; off produces no new trace entry.
    pub status: Option<RouteStatus>,
}

/// Runs beside original retrieval, so a failed expansion cannot consume its window.
pub(super) async fn execute<P: ModelPort, R: RetrievalProjectionPort>(
    context: &SearchContext<'_, P, R>,
    admitted: &AdmittedSearch,
) -> IntentRoutes {
    if admitted.configuration.intent_expansion == IntentExpansion::Off {
        return IntentRoutes {
            outcomes: Vec::new(),
            status: None,
        };
    }
    // The expansion and its routes end at the routes' end, so the rerank
    // keeps its reserve whatever the expansion costs.
    let cutoffs = &admitted.cutoffs;
    let expansion = match expand(
        context.intent_expander.as_deref(),
        &admitted.understood,
        admitted.configuration.intent_deadline_ms,
        cutoffs.routes_end,
    )
    .await
    {
        Ok(expansion) => expansion,
        Err(failure) => {
            return IntentRoutes {
                outcomes: Vec::new(),
                status: Some(RouteStatus::Unavailable(failure.code().to_owned())),
            };
        }
    };
    let dense_query = Query {
        generation: &admitted.generation,
        scopes: &admitted.scopes,
        text: &expansion.passage,
        limit: admitted.configuration.routes_limit,
        identifier_limit: admitted.configuration.identifier_limit,
        version: admitted.version.as_deref(),
        projection: context.projection,
        clock: &admitted.clock,
    };
    let lexical_query = Query {
        text: &expansion.keywords,
        ..dense_query
    };
    let (dense, lexical) = tokio::join!(
        dense_outcome(
            admitted.configuration.dense_enabled,
            &dense_query,
            context.embedder.as_ref(),
            cutoffs
        ),
        lexical_outcome(
            admitted.configuration.lexical_enabled,
            &lexical_query,
            cutoffs.route_after(Instant::now())
        ),
    );
    IntentRoutes {
        outcomes: vec![(Route::DenseIntent, dense), (Route::LexicalIntent, lexical)],
        status: Some(RouteStatus::Ok),
    }
}

/// Runs `expander` on `question` until `deadline_ms` from now, and never
/// past `cutoff`, then guards its expansion.
pub(super) async fn expand(
    expander: Option<&dyn QueryExpander>,
    question: &Understood,
    deadline_ms: u32,
    cutoff: Instant,
) -> Result<Expansion, ExpansionFailure> {
    let expander = expander.ok_or(ExpansionFailure::ModelUnavailable)?;
    let deadline = cutoff.min(Instant::now() + Duration::from_millis(u64::from(deadline_ms)));
    let expansion = until(deadline, expander.expand(question))
        .await
        .map_err(|_| ExpansionFailure::DeadlineExceeded)??;
    validate(question, &expansion)?;
    Ok(expansion)
}
