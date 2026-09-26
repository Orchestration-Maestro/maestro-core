//! What the projection's tests share: a publication of a kernel's chunk set
//! through a server, the names its generations take there, and the ID of a
//! chunk's point, derived here apart from the library.

use super::{backends::Backend, kernel::Kernel, models::Embedder};
use maestro_kernel::{gateway::ModelCard, generation::GenerationState};
use maestro_knowledge::index::{Error, Projection, Qdrant, Report};

/// The projection of `kernel` into `qdrant`, embedding through `embedder`
/// under `card`.
pub(super) fn projection<'a>(
    kernel: &'a Kernel,
    qdrant: &'a Qdrant,
    embedder: &'a Embedder,
    card: &'a ModelCard,
) -> Projection<'a, Embedder> {
    Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        qdrant,
        port: embedder,
        card,
    }
}

/// Publishes the complete chunk set of `kernel` into `backend`, embedding
/// through `embedder` under `card`.
pub(super) async fn publish(
    kernel: &Kernel,
    backend: &Backend,
    embedder: &Embedder,
    card: &ModelCard,
) -> Result<Report, Error> {
    let qdrant = backend.client();
    projection(kernel, &qdrant, embedder, card)
        .publish(&kernel.chunk_set)
        .await
}

/// The Qdrant collection of the generation `generation` of `kernel`'s
/// collection.
pub(super) fn collection_of(kernel: &Kernel, generation: i64) -> String {
    format!("maestro-{}-g{generation}", kernel.collection)
}

/// The alias of `kernel`'s collection.
pub(super) fn alias_of(kernel: &Kernel) -> String {
    format!("maestro-{}", kernel.collection)
}

/// The state of the generation `generation` of `kernel`.
pub(super) fn state_of(kernel: &Kernel, generation: i64) -> GenerationState {
    kernel
        .database
        .generation(&kernel.scopes, generation)
        .unwrap()
        .unwrap()
        .state
}

/// The ID of the point of the chunk `chunk`, one the tests name, as Python's
/// `uuid` computes it: `uuid5(uuid5(NAMESPACE_URL,
/// "https://github.com/Orchestration-Maestro/maestro-core#qdrant-point"),
/// chunk)`.
pub(super) fn point_id(chunk: &str) -> &'static str {
    match chunk {
        "chunk-0-1" => "ee0aded0-959d-5c2a-b129-653cff53921c",
        "chunk-0-lead" => "04607665-0bc4-50c3-b9b2-ac5da7f6de08",
        "chunk-1-2" => "cae1bb50-4b7b-566f-a9cd-a460f6a5f5b8",
        other => panic!("no test names the point of {other}"),
    }
}
