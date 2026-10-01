//! Background startup warming for the embedder and reranker cards that
//! visible published collections search with.

use super::types::KernelOpener;
use maestro_kernel::{
    artifact::Digest,
    gateway::{Error, ModelCard, ModelPort, Role, Room},
};
use std::{collections::BTreeSet, sync::Arc};
use tokio::task::spawn_blocking;

/// Generation profile prefix that binds dense vectors to one embedder card.
const DENSE_PROFILE_PREFIX: &str = "dense/1:sha256:";
/// Small, non-user input that loads each card without exposing source data.
const WARMUP_INPUT: &str = "maestro warm-up";

/// Discovers cards off the executor, then warms each distinct card sequentially.
pub(super) async fn run<P: ModelPort + Send + Sync + 'static>(
    open_kernel: KernelOpener,
    port: Arc<P>,
    test_cards: Option<Vec<ModelCard>>,
) {
    let cards = match test_cards {
        Some(cards) => Ok(cards),
        None => spawn_blocking(move || published_model_cards(&open_kernel))
            .await
            .unwrap_or(Err(())),
    };
    let Ok(cards) = cards else {
        let mut warned = false;
        warn_once(&mut warned, "visible model cards could not be loaded");
        return;
    };
    warm_cards(cards, port.as_ref()).await;
}

/// Finds, for each visible collection with a published generation, the
/// registered embedder card its generation names, then the reranker card
/// selected for it. A selection that cannot be read skips only its own
/// reranker, with one warning.
fn published_model_cards(open_kernel: &KernelOpener) -> Result<Vec<ModelCard>, ()> {
    let kernel = open_kernel().map_err(|_| ())?;
    let collections = kernel
        .database
        .collections(&kernel.scopes)
        .map_err(|_| ())?;
    let mut embedders = Vec::new();
    let mut rerankers = Vec::new();
    let mut warned = false;
    for collection in collections {
        let Some(generation) = kernel
            .database
            .published_generation(&kernel.scopes, &collection.id)
            .map_err(|_| ())?
        else {
            continue;
        };
        if let Some(digest) = generation
            .embedding_profile
            .strip_prefix(DENSE_PROFILE_PREFIX)
            .and_then(|text| Digest::parse(text).ok())
            && let Some(card) = kernel
                .database
                .model_card(&kernel.scopes, &collection.id, &digest)
                .map_err(|_| ())?
            && card.fields().role == Role::Embedder
        {
            embedders.push(card);
        }
        match kernel
            .database
            .selected_model_card(&kernel.scopes, &collection.id, Role::Reranker)
        {
            Ok(Some(selected)) => rerankers.push(selected.card),
            Ok(None) => {}
            Err(_) => warn_once(&mut warned, "a reranker selection could not be read"),
        }
    }
    embedders.append(&mut rerankers);
    Ok(embedders)
}

/// Warms unique cards one at a time, an embedder by an embedding and a
/// reranker by a reranking, without ever asking the router to unload a model.
async fn warm_cards<P: ModelPort>(cards: Vec<ModelCard>, port: &P) {
    let mut seen = BTreeSet::new();
    let mut warned = false;
    let inputs = [WARMUP_INPUT.to_owned()];
    for card in cards {
        if !seen.insert(card.digest().clone()) {
            continue;
        }
        let warmed = if card.fields().role == Role::Reranker {
            port.rerank(&card, Room::Free, WARMUP_INPUT, &inputs)
                .await
                .map(drop)
        } else {
            port.embed(&card, Room::Free, &inputs).await.map(drop)
        };
        match warmed {
            Ok(()) => {}
            Err(Error::Unavailable { .. }) => {
                warn_once(&mut warned, "no free room for a model");
                break;
            }
            Err(_) => warn_once(&mut warned, "a model request failed"),
        }
    }
}

/// Emits at most one fixed, single-line startup warning.
fn warn_once(warned: &mut bool, reason: &'static str) {
    if !*warned {
        eprintln!("maestro: model warm-up skipped: {reason}");
        *warned = true;
    }
}

#[cfg(test)]
mod tests;
