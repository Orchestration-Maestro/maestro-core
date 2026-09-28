//! Whether a search ran every stage its rung enables. Search falls back when
//! a stage cannot run, and records why in its routes; the ladder counts such
//! a search, and the `ask` answered from it, as failed, since the rung's
//! configuration did not run. Identifier and structured statuses are not
//! checked: they skip some questions by design.

use maestro_kernel::evidence::RouteStatus;
use maestro_knowledge::search::{
    DEADLINE_EXCEEDED, NO_FUSED_CANDIDATES, Route, SearchConfiguration,
};
use std::collections::BTreeMap;

/// How a search, or an `ask`, that did not run its rung ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StageFailure {
    /// It ran out of time.
    TimedOut,
    /// It failed.
    Failed,
}

/// The name search gives its rerank stage in its routes.
const RERANK: &str = "rerank";

/// How the first stage `configuration` enables that did not run, as
/// `routes` records it, ended: out of time for `deadline_exceeded`, failed
/// otherwise, and failed when `routes` does not name it. None when every
/// enabled stage ran. A rerank with no fused candidate to rerank ran.
pub(super) fn stage_failure(
    configuration: &SearchConfiguration,
    routes: &BTreeMap<String, RouteStatus>,
) -> Option<StageFailure> {
    let stages = [
        (configuration.dense_enabled, Route::Dense.name()),
        (configuration.lexical_enabled, Route::Lexical.name()),
        (configuration.rerank_enabled, RERANK),
    ];
    stages
        .into_iter()
        .filter(|(enabled, _)| *enabled)
        .find_map(|(_, stage)| match routes.get(stage) {
            Some(RouteStatus::Ok) => None,
            Some(RouteStatus::Unavailable(reason))
                if stage == RERANK && reason == NO_FUSED_CANDIDATES =>
            {
                None
            }
            Some(RouteStatus::Unavailable(reason)) if reason == DEADLINE_EXCEEDED => {
                Some(StageFailure::TimedOut)
            }
            Some(RouteStatus::Unavailable(_)) | None => Some(StageFailure::Failed),
        })
}
