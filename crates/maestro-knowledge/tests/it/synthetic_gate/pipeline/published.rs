//! The synthetic collection imported, prepared and published against a fake
//! Qdrant, for the tests that search it through the production path.

use super::super::{failure::Failure, models::SyntheticModels};
use super::{
    contract::COLLECTION,
    driver,
    fixture::{self, Scratch},
};
use crate::qdrant_projection;
use maestro_kernel::{gateway::ModelCard, store::Database};
use maestro_knowledge::{index::Qdrant, suite::Suite};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// The principal the synthetic gate grants the collection to.
pub(in crate::synthetic_gate) const PRINCIPAL: &str = "synthetic-gate";

/// The published synthetic collection, and what searching it needs.
pub(in crate::synthetic_gate) struct Published {
    /// The collection's ID.
    pub(in crate::synthetic_gate) collection: &'static str,
    /// The scratch kernel holding it.
    pub(in crate::synthetic_gate) database: Arc<Database>,
    /// The fake Qdrant holding its generation.
    pub(in crate::synthetic_gate) qdrant: Qdrant,
    /// The embedder's card.
    pub(in crate::synthetic_gate) card: ModelCard,
    /// The fake inference.
    pub(in crate::synthetic_gate) models: SyntheticModels,
    /// The fixture's evaluation suite.
    pub(in crate::synthetic_gate) suite: Suite,
    /// The scratch directory, removed when this is dropped.
    _scratch: Scratch,
}

/// Imports, prepares and publishes the synthetic collection against a fake
/// Qdrant served on `runtime`.
pub(in crate::synthetic_gate) fn publish_fake(runtime: &Runtime) -> Result<Published, Failure> {
    let url = runtime.block_on(async { qdrant_projection::synthetic_fake_qdrant_url() });
    let qdrant = Qdrant::new(&url).map_err(|error| Failure::from_error("qdrant-client", error))?;
    let fixture = fixture::load()?;
    let built = driver::build_generation(runtime, &qdrant, &fixture, &mut Vec::new())?;
    Ok(Published {
        collection: COLLECTION,
        database: Arc::new(built.database),
        qdrant,
        card: built.card,
        models: built.models,
        suite: fixture.suite,
        _scratch: built.scratch,
    })
}
