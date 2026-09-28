//! Exact generation inventories and their separately bounded supports.

use super::outcome::{RouteOutcome, StructuredOutcome};
use crate::search::{
    deadline::{self, DEADLINE_EXCEEDED},
    query::Query,
};
use maestro_kernel::{
    evidence::RouteStatus,
    retrieval::{self, InventoryRequest, ReadControl, SearchRead},
    store::Database,
};
use std::sync::Arc;
use tokio::time::Instant;

/// Counts the pinned generation's manifest members for one exact inventory request.
pub async fn search_structured(
    query: &Query<'_>,
    database: Arc<Database>,
    request: &InventoryRequest,
    deadline: Instant,
) -> StructuredOutcome {
    let generation = query.generation.clone();
    let scopes = query.scopes.clone();
    let version = query.version.map(str::to_owned);
    let request = request.clone();
    match deadline::run_blocking(deadline, move |cancelled| {
        let control = ReadControl {
            deadline: deadline.into_std(),
            cancelled,
        };
        let read = SearchRead {
            generation: &generation,
            scopes: &scopes,
            version: version.as_deref(),
            control: &control,
        };
        database.inventory(&read, &request)
    })
    .await
    {
        Ok(Ok(selection)) => {
            let hits = selection
                .supports
                .into_iter()
                .map(|hit| super::results::ScoredChunk {
                    chunk_id: hit.chunk_id,
                    revision_id: hit.revision_id,
                    score: 1.0,
                })
                .collect::<Vec<_>>();
            let limit = hits.len();
            let hits = super::results::rank(hits, limit);
            StructuredOutcome {
                route: RouteOutcome {
                    hits,
                    status: RouteStatus::Ok,
                },
                inventory: Some(selection.inventory),
            }
        }
        Ok(Err(error)) => unavailable(&error_reason(&error)),
        Err(deadline::BlockingFailure::WorkerFailed) => {
            unavailable("inventory search worker failed")
        }
        Err(deadline::BlockingFailure::TimedOut) => unavailable(DEADLINE_EXCEEDED),
    }
}

/// Maps kernel errors to bounded public route reasons.
fn error_reason(error: &retrieval::Error) -> String {
    match error {
        retrieval::Error::ProjectionMissing => {
            "search projection missing; publish a new generation".to_owned()
        }
        retrieval::Error::ProfileMismatch { .. } => "search projection profile mismatch".to_owned(),
        retrieval::Error::TooLarge => "inventory too large; restrict the document set".to_owned(),
        retrieval::Error::TimedOut | retrieval::Error::Cancelled => DEADLINE_EXCEEDED.to_owned(),
        retrieval::Error::InvalidInput(_) => "inventory request is invalid".to_owned(),
        retrieval::Error::Store(_)
        | retrieval::Error::UnknownOrInaccessible
        | retrieval::Error::InputConflict
        | retrieval::Error::MembershipConflict => "inventory search failed".to_owned(),
    }
}

/// Builds a failed structured result without partial groups or supports.
fn unavailable(reason: &str) -> StructuredOutcome {
    StructuredOutcome {
        route: RouteOutcome {
            hits: Vec::new(),
            status: RouteStatus::Unavailable(reason.to_owned()),
        },
        inventory: None,
    }
}
