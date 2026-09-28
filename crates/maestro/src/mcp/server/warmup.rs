//! Background startup warming for visible published embedder cards.

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
        None => spawn_blocking(move || published_embedder_cards(&open_kernel))
            .await
            .unwrap_or(Err(())),
    };
    let Ok(cards) = cards else {
        let mut warned = false;
        warn_once(&mut warned, "visible embedder cards could not be loaded");
        return;
    };
    warm_cards(cards, port.as_ref()).await;
}

/// Finds registered embedder cards named by visible currently published generations.
fn published_embedder_cards(open_kernel: &KernelOpener) -> Result<Vec<ModelCard>, ()> {
    let kernel = open_kernel().map_err(|_| ())?;
    let collections = kernel
        .database
        .collections(&kernel.scopes)
        .map_err(|_| ())?;
    let mut seen = BTreeSet::new();
    let mut cards = Vec::new();
    for collection in collections {
        let generation = kernel
            .database
            .published_generation(&kernel.scopes, &collection.id)
            .map_err(|_| ())?;
        let Some(digest) = generation
            .and_then(|generation| {
                generation
                    .embedding_profile
                    .strip_prefix(DENSE_PROFILE_PREFIX)
                    .map(str::to_owned)
            })
            .and_then(|text| Digest::parse(&text).ok())
        else {
            continue;
        };
        let Some(card) = kernel
            .database
            .model_card(&kernel.scopes, &collection.id, &digest)
            .map_err(|_| ())?
        else {
            continue;
        };
        if card.fields().role == Role::Embedder && seen.insert(card.digest().clone()) {
            cards.push(card);
        }
    }
    Ok(cards)
}

/// Warms unique cards one at a time without ever asking the router to unload a model.
async fn warm_cards<P: ModelPort>(cards: Vec<ModelCard>, port: &P) {
    let mut seen = BTreeSet::new();
    let mut warned = false;
    let inputs = [WARMUP_INPUT.to_owned()];
    for card in cards {
        if !seen.insert(card.digest().clone()) {
            continue;
        }
        match port.embed(&card, Room::Free, &inputs).await {
            Ok(_) => {}
            Err(Error::Unavailable { .. }) => {
                warn_once(&mut warned, "no free room for an embedder");
                break;
            }
            Err(_) => warn_once(&mut warned, "an embedder request failed"),
        }
    }
}

/// Emits at most one fixed, single-line startup warning.
fn warn_once(warned: &mut bool, reason: &'static str) {
    if !*warned {
        eprintln!("maestro: embedder warm-up skipped: {reason}");
        *warned = true;
    }
}

#[cfg(test)]
mod tests;
