//! The registered cards a ladder rung may name by digest: found in the
//! collection with the role the rung needs, and never selected.

use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    artifact::Digest,
    gateway::{ModelCard, Role},
};
use maestro_knowledge::answer::RegisteredAnswerer;

/// The reranker card of `digest`, registered in `collection`, without
/// selecting it; none when `digest` is none.
///
/// # Errors
///
/// [`Failure::Refused`] for a card the collection has not registered, or
/// whose role is not reranker; [`Failure::Failed`] when the kernel cannot be
/// read.
pub(super) fn candidate_reranker(
    kernel: &Kernel,
    collection: &str,
    digest: Option<Digest>,
) -> Result<Option<ModelCard>, Failure> {
    let Some(digest) = digest else {
        return Ok(None);
    };
    let card = kernel
        .database
        .model_card(&kernel.scopes, collection, &digest)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused("a rung's reranker card is not registered in the collection")
        })?;
    if card.fields().role != Role::Reranker {
        return Err(Failure::refused(
            "a rung's reranker card does not have the reranker role",
        ));
    }
    Ok(Some(card))
}

/// The answerer card of `digest`, registered in `collection` with the
/// answerer role, without selecting it.
///
/// # Errors
///
/// [`Failure::Refused`] for a card the collection has not registered, or
/// whose role is not answerer; [`Failure::Failed`] when the kernel or the
/// card cannot be read.
pub(super) fn candidate_answerer(
    kernel: &Kernel,
    collection: &str,
    digest: &Digest,
) -> Result<RegisteredAnswerer, Failure> {
    let answerers = kernel
        .database
        .model_cards(&kernel.scopes, collection, Role::Answerer)
        .map_err(|error| Failure::failed_by(&error))?;
    if let Some(record) = answerers
        .into_iter()
        .find(|record| record.digest == *digest)
    {
        let card = ModelCard::load(&kernel.artifacts, &record.digest)
            .map_err(|error| Failure::failed_by(&error))?;
        return Ok(RegisteredAnswerer {
            id: record.id.to_string(),
            card,
        });
    }
    let registered = kernel
        .database
        .model_card(&kernel.scopes, collection, digest)
        .map_err(|error| Failure::failed_by(&error))?;
    Err(Failure::refused(if registered.is_some() {
        "a rung's answerer card does not have the answerer role"
    } else {
        "a rung's answerer card is not registered in the collection"
    }))
}
