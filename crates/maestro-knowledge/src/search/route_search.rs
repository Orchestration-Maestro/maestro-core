//! Cached original routes and additive fusion for one pinned search.

use super::{
    admission::AdmittedSearch,
    deadline::DISABLED_BY_CONFIGURATION,
    fusion::{Fused, Route, RouteList, fuse_weighted},
    intent::IntentTrigger,
    intent_routes::{self, IntentRoutes},
    query::Query,
    request::{SearchConfiguration, SearchContext, SearchObservations},
    route_execution::{
        add_route, dense_outcome, join_route_futures, lexical_outcome, observed_chunk_ids,
        revisions_for_fused, route_list, route_observations, route_outcome, structured_outcome,
    },
    routes::{
        identifier::{IdentifierMode, search_identifiers_as},
        outcome::{IdentifierOutcome, RouteOutcome, StructuredOutcome},
    },
};
use crate::{index::RetrievalProjectionPort, query::QueryKind};
use maestro_kernel::{
    evidence::{Inventory, RouteStatus},
    gateway::ModelPort,
    telemetry::{
        span,
        stage::{Count, Outcome, Stage},
    },
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
};

/// The dense and lexical routes' bounded candidate pool.
const DENSE_LIMIT: usize = 100;
/// The single reciprocal-rank fusion pool limit.
const FUSION_POOL: usize = 120;

/// Runs `route` as the stage `stage`, which records its hits and outcome.
async fn traced_route(stage: Stage, route: impl Future<Output = RouteOutcome>) -> RouteOutcome {
    traced(stage, route, |outcome| outcome).await
}

/// Runs `route` as the stage `stage`, which records the hits and outcome
/// of the route outcome `part` finds in its output.
async fn traced<O>(
    stage: Stage,
    route: impl Future<Output = O>,
    part: impl Fn(&O) -> &RouteOutcome,
) -> O {
    let output = stage.instrument(route).await;
    let outcome = part(&output);
    stage.count(Count::Candidates, outcome.hits.len());
    stage.finish(route_outcome(&outcome.status));
    output
}

/// The route votes, statuses and exact document inventory produced in parallel.
pub(super) struct RouteResults {
    /// The one fused list bounded for T031.
    pub(super) fused: Vec<Fused>,
    /// Revision IDs retained independently from fusion for kernel validation.
    pub(super) expected_revisions: HashMap<String, Vec<String>>,
    /// Each route's independent availability status.
    pub(super) routes: BTreeMap<String, RouteStatus>,
    /// The complete structured result, kept outside passage fusion.
    pub(super) inventory: Option<Inventory>,
    /// Every degradation known before evidence expansion.
    pub(super) known_gaps: Vec<String>,
    /// Scoped candidate ranks observed before fusion.
    pub(super) observations: SearchObservations,
    /// Candidates the intent votes added to the rerank beyond its depth.
    pub(super) rerank_extra: usize,
    /// Rerank scores a first pass already gave, reused by chunk ID.
    pub(super) known_scores: HashMap<String, f64>,
}

/// Runs the structured `route` as its stage, which records its hits and
/// outcome, when the question is `global`; else neither runs.
async fn traced_structured(
    global: bool,
    enabled: bool,
    route: impl Future<Output = StructuredOutcome>,
) -> Option<StructuredOutcome> {
    if !global {
        return None;
    }
    let stage = span::route_structured();
    let outcome = if enabled {
        stage.instrument(route).await
    } else {
        StructuredOutcome {
            route: RouteOutcome {
                hits: Vec::new(),
                status: RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
            },
            inventory: None,
        }
    };
    stage.count(Count::Candidates, outcome.route.hits.len());
    stage.finish(route_outcome(&outcome.route.status));
    Some(outcome)
}

/// Original bounded route votes, retained for a conditional second fusion.
pub(super) type Originals = (
    RouteOutcome,
    RouteOutcome,
    IdentifierOutcome,
    Option<StructuredOutcome>,
);

/// Runs unconditional expansion in parallel, but defers a confidence-triggered model.
pub(super) async fn execute<P: ModelPort, R: RetrievalProjectionPort>(
    context: &SearchContext<'_, P, R>,
    admitted: &AdmittedSearch,
) -> (Originals, IntentRoutes) {
    let intent = async {
        if admitted.configuration.intent_trigger == IntentTrigger::Always {
            intent_routes::execute(context, admitted).await
        } else {
            IntentRoutes {
                outcomes: Vec::new(),
                status: None,
            }
        }
    };
    tokio::join!(original_routes(context, admitted), intent)
}

/// Fuses original cached votes and any additional model-generated routes.
/// Intent votes never push an original candidate out of the rerank: the
/// original fused top-depth stays in it, and at most the configured number
/// of candidates the intent votes lifted into the combined top-depth join.
pub(super) fn results(
    admitted: &AdmittedSearch,
    originals: &Originals,
    intent: IntentRoutes,
) -> RouteResults {
    let (dense, lexical, identifier, structured) = originals;
    let configuration = admitted.configuration;
    let guard = configuration.identifier_noise_guard;
    let mut lists = original_lists(originals, guard);
    let mut observations =
        route_observations(dense, lexical, &identifier.route, structured.as_ref());
    observations
        .identifiers_dropped
        .clone_from(&identifier.dropped);
    let mut rerank_extra = 0;
    let fused = if intent.outcomes.is_empty() {
        traced_fuse(&lists, configuration, FUSION_POOL)
    } else {
        let depth = configuration.rerank_depth.get();
        // The original order alone, untraced: only its top-depth is kept.
        let kept = fuse_weighted(&lists, depth, configuration.rrf_k, |route| {
            configuration.weight(route)
        })
        .into_iter()
        .map(|candidate| candidate.chunk_id)
        .collect::<HashSet<_>>();
        lists.extend(
            intent
                .outcomes
                .iter()
                .map(|(route, outcome)| route_list(*route, outcome, guard)),
        );
        let set = rerank_set(
            &kept,
            traced_fuse(&lists, configuration, usize::MAX),
            depth,
            configuration.intent_rerank_additions,
        );
        rerank_extra = set.added;
        observations.intent_displaced = Some(set.displaced);
        for (route, outcome) in &intent.outcomes {
            observations
                .route_ranks
                .insert(*route, observed_chunk_ids(outcome));
        }
        set.fused
    };
    let expected_revisions = revisions_for_fused(
        &fused,
        [dense, lexical, &identifier.route]
            .into_iter()
            .chain(structured.iter().map(|outcome| &outcome.route))
            .chain(intent.outcomes.iter().map(|(_, outcome)| outcome)),
    );
    let mut known_gaps = Vec::new();
    if !admitted.version_documented
        && let Some(version) = admitted.version.as_deref()
    {
        known_gaps.push(format!(
            "requested version {version:?} has no documents in the pinned generation"
        ));
    }
    let (mut routes, known_gaps, inventory) = route_metadata(
        dense,
        lexical,
        &identifier.route,
        structured.as_ref(),
        known_gaps,
    );
    if let Some(status) = intent.status {
        routes.insert("intent_expansion".to_owned(), status);
    }
    for (route, outcome) in intent.outcomes {
        routes.insert(route.name().to_owned(), outcome.status);
    }
    RouteResults {
        fused,
        expected_revisions,
        routes,
        inventory,
        known_gaps,
        observations,
        rerank_extra,
        known_scores: HashMap::new(),
    }
}

/// The original routes' fusion lists; with `drop_unavailable`, an
/// unavailable route's list is empty.
fn original_lists(originals: &Originals, drop_unavailable: bool) -> Vec<RouteList> {
    let (dense, lexical, identifier, structured) = originals;
    let mut lists = vec![
        route_list(Route::Dense, dense, drop_unavailable),
        route_list(Route::Lexical, lexical, drop_unavailable),
        route_list(Route::Identifier, &identifier.route, drop_unavailable),
    ];
    if let Some(structured) = structured {
        lists.push(route_list(
            Route::Structured,
            &structured.route,
            drop_unavailable,
        ));
    }
    lists
}

/// The combined pool in rerank order, and what the intent votes changed.
struct RerankSet {
    /// The rerank set in combined fused order, then the rest of the pool.
    fused: Vec<Fused>,
    /// Candidates added to the rerank beyond its depth.
    added: usize,
    /// Kept candidates the combined order put below the depth.
    displaced: usize,
}

/// Orders `combined` for the rerank: every `kept` candidate and at most
/// `additions` others of the combined top-`depth`, in combined order, then
/// the rest, within the fusion pool.
fn rerank_set(
    kept: &HashSet<String>,
    combined: Vec<Fused>,
    depth: usize,
    additions: usize,
) -> RerankSet {
    let displaced = combined
        .iter()
        .skip(depth)
        .filter(|candidate| kept.contains(&candidate.chunk_id))
        .count();
    let added = combined
        .iter()
        .take(depth)
        .filter(|candidate| !kept.contains(&candidate.chunk_id))
        .take(additions)
        .map(|candidate| candidate.chunk_id.clone())
        .collect::<HashSet<_>>();
    let (mut fused, rest): (Vec<_>, Vec<_>) = combined.into_iter().partition(|candidate| {
        kept.contains(&candidate.chunk_id) || added.contains(&candidate.chunk_id)
    });
    let room = FUSION_POOL.saturating_sub(fused.len());
    fused.extend(rest.into_iter().take(room));
    RerankSet {
        fused,
        added: added.len(),
        displaced,
    }
}

/// Runs unchanged original routes independently of optional expansion.
async fn original_routes<P: ModelPort, R: RetrievalProjectionPort>(
    context: &SearchContext<'_, P, R>,
    admitted: &AdmittedSearch,
) -> (
    RouteOutcome,
    RouteOutcome,
    IdentifierOutcome,
    Option<StructuredOutcome>,
) {
    let query = Query {
        generation: &admitted.generation,
        scopes: &admitted.scopes,
        text: &admitted.understood.normalized,
        limit: DENSE_LIMIT,
        version: admitted.version.as_deref(),
        qdrant: context.qdrant,
    };
    let structured_request = match admitted.structured_error.as_deref() {
        Some(error) => Err(error),
        None => Ok(admitted.structured_request.as_ref()),
    };
    let configuration = admitted.configuration;
    // Boxed: an async function keeps a future it takes by value beside the
    // copy it polls, so each wrapper below would double the routes' futures,
    // and a debug build copies them through its poll frames on the stack.
    join_route_futures(
        traced_route(
            span::route_dense(),
            Box::pin(dense_outcome(
                configuration.dense_enabled,
                &query,
                context.embedder.as_ref(),
                &admitted.cutoffs,
            )),
        ),
        traced_route(
            span::route_lexical(),
            Box::pin(lexical_outcome(
                configuration.lexical_enabled,
                &query,
                admitted.cutoffs.routes,
            )),
        ),
        traced(
            span::route_identifier(),
            Box::pin(search_identifiers_as(
                identifier_mode(configuration),
                &query,
                context.database.clone(),
                &admitted.understood,
                admitted.cutoffs.routes,
            )),
            |outcome: &IdentifierOutcome| &outcome.route,
        ),
        traced_structured(
            admitted.understood.kind == QueryKind::Global,
            configuration.structured_enabled,
            structured_outcome(
                &query,
                context.database.clone(),
                structured_request,
                admitted.cutoffs.routes,
            ),
        ),
    )
    .await
}

/// How `configuration` runs the identifier route.
const fn identifier_mode(configuration: SearchConfiguration) -> IdentifierMode {
    match (
        configuration.identifier_enabled,
        configuration.identifier_noise_guard,
    ) {
        (false, _) => IdentifierMode::Off,
        (true, false) => IdentifierMode::On,
        (true, true) => IdentifierMode::Guarded,
    }
}

/// Records each route's status, its degradation gaps and the structured inventory.
fn route_metadata(
    dense: &RouteOutcome,
    lexical: &RouteOutcome,
    identifier: &RouteOutcome,
    structured: Option<&StructuredOutcome>,
    mut known_gaps: Vec<String>,
) -> (
    BTreeMap<String, RouteStatus>,
    Vec<String>,
    Option<Inventory>,
) {
    let mut routes = BTreeMap::new();
    for (route, outcome) in [
        (Route::Dense, dense),
        (Route::Lexical, lexical),
        (Route::Identifier, identifier),
    ] {
        add_route(&mut routes, &mut known_gaps, route, &outcome.status);
    }
    let inventory = structured.and_then(|outcome| outcome.inventory.clone());
    if let Some(outcome) = structured {
        add_route(
            &mut routes,
            &mut known_gaps,
            Route::Structured,
            &outcome.route.status,
        );
    }
    (routes, known_gaps, inventory)
}

/// Fuses the routes' `lists` into a pool of at most `limit`, as the fusion
/// stage.
fn traced_fuse(
    lists: &[RouteList],
    configuration: SearchConfiguration,
    limit: usize,
) -> Vec<Fused> {
    let stage = span::fuse();
    let fused = stage.in_scope(|| {
        fuse_weighted(lists, limit, configuration.rrf_k, |route| {
            configuration.weight(route)
        })
    });
    stage.count(Count::Candidates, fused.len());
    stage.finish(Outcome::Ok);
    fused
}
