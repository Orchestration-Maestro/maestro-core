//! A single buffered ask against a registered answerer, live: an explicit
//! local run, never a pass during the normal test suite. It reads
//! `MAESTRO_SEARCH_COLLECTION` and `MAESTRO_ASK_QUESTION` from this machine's
//! kernel; set `XDG_DATA_HOME` to a copy of the kernel to leave it unchanged.
//!
//! It uses only the registered `qwen3-4b` answerer card. If that card is not
//! registered for the collection, it reports the T030b prerequisite and makes
//! no router or Qdrant request. Run explicitly with:
//! `cargo test -p maestro-knowledge --test it answer_live -- --ignored --nocapture`

#![cfg(test)]

use super::{
    live_router::required,
    publish_live::{card_of, kernel},
};
use maestro_kernel::{
    artifact::Store,
    gateway::{ModelCard, Role, RouterClient, Url},
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    answer::{AnswerContext, AskBudget, AskRequest, RegisteredAnswerer, ask},
    index::Qdrant,
    search::{Reranker, SearchContext, pin, routes::dense::Embedder},
};
use std::{path::Path, sync::Arc};

#[tokio::test]
#[ignore = "asks this machine's live collection and router; run explicitly"]
async fn asks_with_the_registered_qwen3_4b_answerer() {
    let collection = required("MAESTRO_SEARCH_COLLECTION");
    let (database, scopes, data) = kernel();
    let answerer = registered_answerer(&database, &scopes, &data, &collection);
    let Some(answerer) = answerer else {
        eprintln!("no registered answerer card for qwen3-4b; run T030b qualification");
        return;
    };

    let question = required("MAESTRO_ASK_QUESTION");
    let qdrant = Qdrant::new(&required("MAESTRO_QDRANT_URL")).unwrap();
    let router = Url::parse(&required("MAESTRO_ROUTER_URL")).unwrap();
    let generation = pin(&database, &scopes, &collection).unwrap();
    let set = database
        .chunk_set(&scopes, &generation.chunk_set_id)
        .unwrap()
        .unwrap();
    let embedder_card = card_of(&set, &data);
    let reranker_card = database
        .selected_model_card(&scopes, &collection, Role::Reranker)
        .unwrap()
        .filter(|selected| selected.evaluation.generation_id == Some(generation.id))
        .map(|selected| selected.card);
    let port = RouterClient::new(router).unwrap();
    let database = Arc::new(database);
    let context = AnswerContext {
        search: SearchContext {
            database,
            principal: LOCAL,
            qdrant: &qdrant,
            embedder: Some(Embedder {
                port: &port,
                card: &embedder_card,
            }),
            reranker: reranker_card
                .as_ref()
                .map(|card| Reranker { port: &port, card }),
        },
        port: &port,
        answerer: Some(answerer),
    };
    let result = Box::pin(ask(
        &context,
        &AskRequest {
            collection,
            question,
            model: "qwen3-4b".to_owned(),
            version: None,
            budget: AskBudget::default(),
        },
    ))
    .await
    .unwrap();

    assert!(result.uncalibrated);
    assert_eq!(result.model.router_entry, "qwen3-4b");
    assert!(result.model.card_id.is_some());
    if result.refusal.is_some() {
        assert!(result.answer.is_empty());
        assert!(result.citations.is_empty());
    } else {
        assert!(!result.answer.is_empty());
        assert!(!result.citations.is_empty());
    }
}

/// Loads, but never creates, a registered answerer card for `collection`.
fn registered_answerer(
    database: &Database,
    scopes: &ScopeSet,
    data: &Path,
    collection: &str,
) -> Option<RegisteredAnswerer> {
    let store = Store::new(data.join("artifacts"));
    let mut registered = database
        .model_cards(scopes, collection, Role::Answerer)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            let card = ModelCard::load(&store, &record.digest).unwrap();
            (card.fields().router_entry.as_str() == "qwen3-4b").then_some((record, card))
        });
    let (record, card) = registered.next_back()?;
    Some(RegisteredAnswerer {
        id: record.id.to_string(),
        card,
    })
}
