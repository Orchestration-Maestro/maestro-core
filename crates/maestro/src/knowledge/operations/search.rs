//! Scoped search followed by the approved evidence assembly handoff.

use super::{
    super::requests::SearchRequest,
    implementation::{KnowledgeError, Scoped, ensure_current_scopes},
};
use crate::kernel::Kernel;
use maestro_kernel::{
    artifact::Digest,
    evidence::Bundle,
    gateway::{ModelCard, ModelPort, Role},
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        Reranker, SearchContext, SearchError, SearchRequest as PipelineRequest,
        evidence::{EvidenceCounter, EvidenceError, assemble_evidence},
        routes::{dense::Embedder, error::RouteError},
        search,
    },
};
use tokio::{task::spawn_blocking, time::Instant};
/// The profile prefix that binds dense vectors to their registered embedder.
const DENSE_PROFILE_PREFIX: &str = "dense/1:sha256:";

/// The bundle plus its request-entry cutoff for bounded transport formatting.
pub(crate) struct SearchData {
    /// The canonical evidence assembled by T032.
    pub(crate) bundle: Bundle,
    /// T032's accepted deadline, enforced through formatting and delivery.
    pub(crate) deadline: Instant,
}

/// Searches through the pinned generation and assembles its canonical evidence.
pub(crate) async fn search_with<P: ModelPort>(
    kernel: Kernel,
    request: &SearchRequest,
    model_port: &P,
    qdrant: &Qdrant,
) -> Result<Scoped<SearchData>, KnowledgeError> {
    let scopes = kernel.scopes.clone();
    let database = kernel.database.clone();
    let budget = request.budget();
    let card_database = database.clone();
    let card_scopes = scopes.clone();
    let collection = request.collection.clone();
    let (embedder, reranker) = spawn_blocking(move || {
        let embedder = generation_embedder(&card_database, &card_scopes, &collection)?;
        let reranker = selected_reranker(&card_database, &card_scopes, &collection)?;
        Ok::<_, KnowledgeError>((embedder, reranker))
    })
    .await
    .map_err(|_| kernel_failure())??;
    let context = SearchContext {
        database: database.clone(),
        principal: LOCAL,
        qdrant,
        embedder: embedder.as_ref().map(|card| Embedder {
            port: model_port,
            card,
        }),
        reranker: reranker.as_ref().map(|card| Reranker {
            port: model_port,
            card,
        }),
    };
    let pipeline_request = PipelineRequest::new(
        &request.collection,
        &request.query,
        request.version.as_deref(),
        budget,
    );
    let input = search(&context, &pipeline_request)
        .await
        .map_err(|error| search_failure(&error))?;
    if input.scopes.as_ref() != &scopes {
        return Err(access_changed());
    }
    let deadline = input.deadline;
    let bundle = assemble_evidence(database, input, EvidenceCounter::Utf8Bytes)
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
        data: SearchData { bundle, deadline },
        kernel,
        scopes,
    })
}

/// Finds only the registered card named by a visible current generation.
fn generation_embedder(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Option<ModelCard>, KnowledgeError> {
    let Some(generation) = database
        .published_generation(scopes, collection)
        .map_err(|_| kernel_failure())?
    else {
        return Ok(None);
    };
    let Some(digest) = generation
        .embedding_profile
        .strip_prefix(DENSE_PROFILE_PREFIX)
        .and_then(|text| Digest::parse(text).ok())
    else {
        return Ok(None);
    };
    database
        .model_card(scopes, collection, &digest)
        .map_err(|_| kernel_failure())
}

/// Freezes the configured real reranker once for this request.
fn selected_reranker(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Option<ModelCard>, KnowledgeError> {
    Ok(database
        .selected_model_card(scopes, collection, Role::Reranker)
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
