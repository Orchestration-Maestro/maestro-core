//! Pins the published generation before any route touches Qdrant.

use super::routes::error::RouteError;
use maestro_kernel::{
    generation::{Generation, GenerationState},
    scope::ScopeSet,
    store::Database,
};

/// The collection's published generation visible to `scopes` at admission.
///
/// A caller retains the returned generation for the whole search, even after
/// a later publish moves the collection alias.
///
/// # Errors
///
/// [`RouteError::UnknownGeneration`] when no generation is recorded for the
/// collection, [`RouteError::UnpublishedGeneration`] when records exist but
/// none is published, and [`RouteError::GenerationLookup`] when the kernel
/// cannot read the records.
pub fn pin(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Generation, RouteError> {
    let generations = database
        .generations(scopes, collection)
        .map_err(RouteError::GenerationLookup)?;
    if let Some(generation) = generations
        .iter()
        .find(|generation| generation.state == GenerationState::Published)
    {
        return Ok(generation.clone());
    }
    match generations.last() {
        Some(generation) => Err(RouteError::UnpublishedGeneration {
            generation: generation.id,
            state: generation.state,
        }),
        None => Err(RouteError::UnknownGeneration {
            collection: collection.to_owned(),
        }),
    }
}
