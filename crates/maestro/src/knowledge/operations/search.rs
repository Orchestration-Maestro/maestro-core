//! Scoped search followed by the approved evidence assembly handoff.

use super::{
    super::requests::SearchRequest,
    implementation::{KnowledgeError, Scoped, ensure_current_scopes},
};
use crate::{
    kernel::{Kernel, pinned_embedder},
    knowledge::source_classes,
    settings::KnowledgeSettings,
};
use maestro_kernel::{
    binding::Bindings,
    evidence::{Bundle, RequestBudget},
    gateway::{ModelCard, ModelPort, Role},
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        HydeExpander, QueryExpander, Reranker, SearchContext, SearchError,
        SearchRequest as PipelineRequest, SourceClassifier,
        evidence::{EvidenceError, assemble_evidence},
        routes::{dense::Embedder, error::RouteError},
        search,
    },
};
use std::{path::Path, sync::Arc};
use tokio::{task::spawn_blocking, time::Instant};

/// The bundle plus its request-entry cutoff for bounded transport formatting.
pub(crate) struct SearchData {
    /// The canonical evidence assembled by T032.
    pub(crate) bundle: Bundle,
    /// T032's accepted deadline, enforced through formatting and delivery.
    pub(crate) deadline: Instant,
    /// The source classifier the search ranked with, for the text view's
    /// labels.
    pub(crate) source_classes: Option<Arc<dyn SourceClassifier>>,
}

/// The cards one search runs with, all optional.
#[derive(Clone, Copy, Default)]
pub(crate) struct SearchCards<'a> {
    /// The embedder of the generation's dense vectors.
    pub(crate) embedder: Option<&'a ModelCard>,
    /// The reranker.
    pub(crate) reranker: Option<&'a ModelCard>,
    /// The answerer that expands queries when a request turns expansion
    /// on; a card that cannot expand leaves search without an expander.
    pub(crate) intent: Option<&'a ModelCard>,
}

/// The local principal's search context over `database` and the projection, with
/// the models of `cards`, all served by `port`.
pub(crate) fn local_search_context<'a, P: ModelPort + Sync>(
    database: &Arc<Database>,
    qdrant: &'a Qdrant,
    port: &'a P,
    cards: SearchCards<'a>,
) -> SearchContext<'a, P> {
    SearchContext {
        intent_expander: cards
            .intent
            .and_then(|card| HydeExpander::new(port, card).ok())
            .map(|expander| Box::new(expander) as Box<dyn QueryExpander + 'a>),
        database: Arc::clone(database),
        principal: LOCAL,
        projection: qdrant,
        embedder: cards.embedder.map(|card| Embedder { port, card }),
        reranker: cards.reranker.map(|card| Reranker { port, card }),
        source_classes: None,
    }
}

/// Searches through the pinned generation under `settings` and assembles its
/// canonical evidence.
pub(crate) async fn search_with<P: ModelPort + Sync>(
    kernel: Kernel,
    request: &SearchRequest,
    settings: &KnowledgeSettings,
    model_port: &P,
    qdrant: &Qdrant,
) -> Result<Scoped<SearchData>, KnowledgeError> {
    let scopes = kernel.scopes.clone();
    let database = kernel.database.clone();
    let budget = request.budget();
    let card_database = database.clone();
    let card_scopes = scopes.clone();
    let artifacts = kernel.artifacts.clone();
    let collection = request.collection.clone();
    let config_dir = kernel.config_dir.clone();
    let (embedder, reranker, intent, source_classes) = spawn_blocking(move || {
        let generation = card_database
            .published_generation(&card_scopes, &collection)
            .map_err(|_| kernel_failure())?;
        let embedder = pinned_embedder(
            &artifacts,
            generation
                .as_ref()
                .map(|generation| generation.embedding_profile.as_str()),
        );
        let reranker = selected_reranker(&card_database, &card_scopes, &collection)?;
        let intent = selected_answerer(&card_database, &card_scopes, &collection)?;
        let source_classes = bound_source_classes(&config_dir)?;
        Ok::<_, KnowledgeError>((embedder, reranker, intent, source_classes))
    })
    .await
    .map_err(|_| kernel_failure())??;
    let mut context = local_search_context(
        &database,
        qdrant,
        model_port,
        SearchCards {
            embedder: embedder.as_ref(),
            reranker: reranker.as_ref(),
            intent: intent.as_ref(),
        },
    );
    context.source_classes.clone_from(&source_classes);
    let pipeline_request = pipeline_request(request, settings, budget);
    let input = search(&context, &pipeline_request)
        .await
        .map_err(|error| search_failure(&error))?;
    if input.scopes.as_ref() != &scopes {
        return Err(access_changed());
    }
    let deadline = input.deadline;
    let counter = input
        .evidence
        .counter()
        .map_err(|error| evidence_failure(&error))?;
    let bundle = assemble_evidence(database, input, counter)
        .await
        .map_err(|error| evidence_failure(&error))?;
    let check_scopes = scopes.clone();
    let kernel = spawn_blocking(move || {
        let mut kernel = kernel;
        ensure_current_scopes(&mut kernel, &check_scopes)?;
        Ok::<_, KnowledgeError>(kernel)
    })
    .await
    .map_err(|_| kernel_failure())??;
    if Instant::now() >= deadline {
        return Err(deadline_failure());
    }
    Ok(Scoped {
        data: SearchData {
            bundle,
            deadline,
            source_classes,
        },
        kernel,
        scopes,
    })
}

/// Builds the search pipeline input with the caller's effective settings.
pub(super) fn pipeline_request<'a>(
    request: &'a SearchRequest,
    settings: &KnowledgeSettings,
    budget: RequestBudget,
) -> PipelineRequest<'a> {
    PipelineRequest {
        configuration: settings.search,
        evidence: settings.evidence,
        ..PipelineRequest::new(
            &request.collection,
            &request.query,
            request.version.as_deref(),
            budget,
        )
    }
}

/// The table the `source_classes` binding names, if any.
///
/// # Errors
///
/// `invalid_configuration` when the bindings file is invalid, or the bound
/// table cannot be read or is invalid, each with its own message.
pub(super) fn bound_source_classes(
    config_dir: &Path,
) -> Result<Option<Arc<dyn SourceClassifier>>, KnowledgeError> {
    Bindings::load(config_dir).map_err(|_| KnowledgeError::Refused {
        code: "invalid_configuration",
        message: "the bindings file is invalid; run `maestro doctor`",
    })?;
    match source_classes::load(config_dir) {
        Ok(table) => Ok(table.map(|table| table as Arc<dyn SourceClassifier>)),
        Err(_) => Err(KnowledgeError::Refused {
            code: "invalid_configuration",
            message: "the source-class table is invalid",
        }),
    }
}

/// Freezes the configured real reranker once for this request.
pub(super) fn selected_reranker(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Option<ModelCard>, KnowledgeError> {
    selected_card(database, scopes, collection, Role::Reranker)
}

/// Freezes the collection's selected answerer once for this request: the
/// card that expands queries when a request turns expansion on.
pub(super) fn selected_answerer(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Option<ModelCard>, KnowledgeError> {
    selected_card(database, scopes, collection, Role::Answerer)
}

/// The card selected for `role` in `collection`, if any.
fn selected_card(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    role: Role,
) -> Result<Option<ModelCard>, KnowledgeError> {
    Ok(database
        .selected_model_card(scopes, collection, role)
        .map_err(|_| kernel_failure())?
        .map(|selected| selected.card))
}

/// Maps search refusals and failures without exposing internal error chains.
pub(super) fn search_failure(error: &SearchError) -> KnowledgeError {
    match error {
        SearchError::InvalidRequest { .. } => KnowledgeError::Refused {
            code: "invalid_arguments",
            message: "search arguments are outside their accepted bounds",
        },
        SearchError::Admission(error) => match error {
            RouteError::UnknownGeneration { .. } | RouteError::UnpublishedGeneration { .. } => {
                KnowledgeError::Refused {
                    code: "not_found",
                    message: "collection is unknown or has no published generation",
                }
            }
            RouteError::GenerationLookup(_) => kernel_failure(),
            _ => search_failure_generic(),
        },
        SearchError::Kernel(_) | SearchError::WorkerFailed => kernel_failure(),
        SearchError::AdmissionTimedOut | SearchError::PermissionCheckTimedOut => deadline_failure(),
        SearchError::EvidenceLoad { .. } => integrity_failure(),
        SearchError::PermissionsChanged => access_changed(),
    }
}

/// Maps authoritative evidence failures without exposing source or backend data.
pub(super) fn evidence_failure(error: &EvidenceError) -> KnowledgeError {
    match error {
        EvidenceError::NotVisible => KnowledgeError::Refused {
            code: "not_found",
            message: "collection is unknown or has no published generation",
        },
        EvidenceError::Integrity(_) | EvidenceError::InvalidRequest(_) => integrity_failure(),
        EvidenceError::TimedOut => deadline_failure(),
        EvidenceError::PermissionsChanged => access_changed(),
        EvidenceError::Generation(_)
        | EvidenceError::ChunkSet(_)
        | EvidenceError::Records(_)
        | EvidenceError::Store(_)
        | EvidenceError::Kernel(_)
        | EvidenceError::WorkerFailed => kernel_failure(),
        EvidenceError::Counter(_) | EvidenceError::Json(_) => search_failure_generic(),
    }
}

/// Refuses to deliver a result after the caller's grants changed.
fn access_changed() -> KnowledgeError {
    KnowledgeError::Refused {
        code: "access_changed",
        message: "permissions changed during the request; no result was delivered",
    }
}

/// Reports a local kernel read or worker failure without its source chain.
fn kernel_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "kernel_unavailable",
        message: "the local knowledge store is unavailable",
    }
}

/// Reports corrupt evidence without exposing its source or artifact path.
pub(super) fn integrity_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "integrity_error",
        message: "the source integrity check failed",
    }
}

/// Reports expiry of T032's accepted request deadline.
fn deadline_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "deadline_exceeded",
        message: "search exceeded its accepted deadline",
    }
}

/// Reports a search failure that has no more specific safe public category.
fn search_failure_generic() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "search_failed",
        message: "the search could not be safely completed",
    }
}
