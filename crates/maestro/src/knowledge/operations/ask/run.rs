//! One local ask operation shared by CLI and MCP.

use super::super::{
    implementation::{KnowledgeError, Scoped, kernel_failure, kernel_open_failure},
    search::{evidence_failure, integrity_failure, search_failure},
};
use crate::{
    cli::health::{QDRANT_VARIABLE, ROUTER_VARIABLE, qdrant_url, router_url},
    failure::Failure,
    kernel::{Kernel, pinned_embedder},
};
use maestro_kernel::{
    gateway::{
        ModelCard, Role, RouterClient,
        card_v2::{Capability, ControlValue},
    },
    generation::Generation,
    scope::{LOCAL, ScopeSet},
};
use maestro_knowledge::{
    answer::{Answer, AnswerContext, AskError, AskRequest, RegisteredAnswerer, ask},
    index::Qdrant,
    search::{Reranker, SearchContext, routes::dense::Embedder},
};
use std::{env, sync::Arc};
use tokio::runtime::Builder;

/// Opens the local kernel, resolves its scoped cards and runs one buffered ask.
pub(crate) fn ask_with(
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
    request: &AskRequest,
) -> Result<Scoped<Answer>, KnowledgeError> {
    let kernel = open_kernel().map_err(|failure| kernel_open_failure(&failure))?;
    let scopes = kernel.scopes.clone();
    let current = kernel
        .database
        .published_generation(&scopes, &request.collection)
        .map_err(|_| kernel_failure())?;
    let embedder_card = pinned_embedder(
        &kernel.artifacts,
        current
            .as_ref()
            .map(|generation| generation.embedding_profile.as_str()),
    );
    let reranker_card = reranker_card(&kernel, &scopes, &request.collection, current.as_ref())?;
    let answerer = registered_answerer(&kernel, &scopes, request)?;
    let router_url = router_url(env::var_os(ROUTER_VARIABLE).as_deref()).map_err(|_| {
        KnowledgeError::Refused {
            code: "invalid_configuration",
            message: "the model router URL is invalid",
        }
    })?;
    let port = RouterClient::new(router_url).map_err(|_| kernel_failure())?;
    let qdrant_url = qdrant_url(env::var_os(QDRANT_VARIABLE).as_deref());
    let qdrant = Qdrant::new(&qdrant_url).map_err(|_| KnowledgeError::Refused {
        code: "invalid_configuration",
        message: "the search service URL is invalid",
    })?;
    let search = SearchContext {
        database: Arc::clone(&kernel.database),
        principal: LOCAL,
        qdrant: &qdrant,
        embedder: embedder_card
            .as_ref()
            .map(|card| Embedder { port: &port, card }),
        reranker: reranker_card
            .as_ref()
            .map(|card| Reranker { port: &port, card }),
    };
    let context = AnswerContext {
        search,
        port: &port,
        answerer,
    };
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| kernel_failure())?;
    let data = runtime
        .block_on(ask(&context, request))
        .map_err(|error| answer_failure(&error))?;
    Ok(Scoped {
        data,
        kernel,
        scopes,
    })
}

/// Loads the generation-matched selected reranker, if one is selected.
pub(super) fn reranker_card(
    kernel: &Kernel,
    scopes: &ScopeSet,
    collection: &str,
    generation: Option<&Generation>,
) -> Result<Option<ModelCard>, KnowledgeError> {
    let Some(generation) = generation else {
        return Ok(None);
    };
    let selected = kernel
        .database
        .selected_model_card(scopes, collection, Role::Reranker)
        .map_err(|_| kernel_failure())?;
    Ok(selected
        .filter(|selected| selected.evaluation.generation_id == Some(generation.id))
        .map(|selected| selected.card))
}

/// Resolves the latest registered answerer with the requested router entry
/// whose reasoning controls do not enable thinking: a thinking card answers
/// only where a ladder rung names it.
pub(crate) fn registered_answerer(
    kernel: &Kernel,
    scopes: &ScopeSet,
    request: &AskRequest,
) -> Result<Option<RegisteredAnswerer>, KnowledgeError> {
    let records = kernel
        .database
        .model_cards(scopes, &request.collection, Role::Answerer)
        .map_err(|_| kernel_failure())?;
    let mut found = None;
    for record in records {
        let card =
            ModelCard::load(&kernel.artifacts, &record.digest).map_err(|_| kernel_failure())?;
        if card.fields().router_entry.as_str() == request.model && !enables_thinking(&card) {
            found = Some(RegisteredAnswerer {
                id: record.id.to_string(),
                card,
            });
        }
    }
    Ok(found)
}

/// Whether `card`'s reasoning controls set `enable_thinking`.
fn enables_thinking(card: &ModelCard) -> bool {
    card.identity().is_some_and(|identity| {
        matches!(
            &identity.invocation.reasoning,
            Capability::Supported(controls)
                if controls.get("enable_thinking") == Some(&ControlValue::Boolean(true))
        )
    })
}

/// Keeps input and admission refusals distinct from local execution failures.
pub(super) fn answer_failure(error: &AskError) -> KnowledgeError {
    match error {
        AskError::InvalidRequest(reason) => KnowledgeError::Refused {
            code: "invalid_arguments",
            message: reason,
        },
        AskError::Search(error) => search_failure(error),
        AskError::Evidence(error) => evidence_failure(error),
        AskError::Backend(_) | AskError::TimedOut => answerer_unavailable(),
        AskError::EvidenceIntegrity => integrity_failure(),
        AskError::Json(_) => KnowledgeError::Failed {
            code: "invalid_configuration",
            message: "the answer request could not be prepared",
        },
    }
}

/// Refuses when an answerer cannot serve a registered generation request.
fn answerer_unavailable() -> KnowledgeError {
    KnowledgeError::Refused {
        code: "answerer_unavailable",
        message: "the answerer is unavailable",
    }
}
