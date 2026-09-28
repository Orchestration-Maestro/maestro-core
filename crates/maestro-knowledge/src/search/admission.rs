//! Request bounds, scope snapshots and generation admission.

use super::request::{
    CandidateContext, SearchConfiguration, SearchContext, SearchError, SearchRequest,
};
use super::{deadline, inventory_query::inventory_request, pin};
use crate::query::{Understood, understand};
use maestro_kernel::{
    evidence::RequestBudget,
    gateway::ModelPort,
    generation::Generation,
    retrieval::{self, InventoryRequest},
    scope::ScopeSet,
    store::Database,
};
use std::{collections::HashSet, sync::Arc};
use tokio::time::Instant;

/// One admitted request pinned to a generation, scope snapshot and cutoff set.
pub(super) struct AdmittedSearch {
    /// The deterministic interpretation of the original question.
    pub(super) understood: Understood,
    /// A recognized inventory request, absent when parsing failed or is inapplicable.
    pub(super) structured_request: Option<InventoryRequest>,
    /// The precise malformed-inventory reason, retained for the structured route.
    pub(super) structured_error: Option<String>,
    /// The exact explicit version filter selected before any route starts.
    pub(super) version: Option<String>,
    /// Whether an explicit version occurs in the pinned, scoped generation.
    pub(super) version_documented: bool,
    /// The one generation pinned before backend access.
    pub(super) generation: Generation,
    /// The trusted caller whose permissions admitted this search.
    pub(super) principal: String,
    /// The immutable permission snapshot used by all routes.
    pub(super) scopes: Arc<ScopeSet>,
    /// Absolute route, work and T032 deadlines derived at request entry.
    pub(super) cutoffs: deadline::Deadlines,
    /// Validated execution settings frozen before route access.
    pub(super) configuration: SearchConfiguration,
}

/// Validates request bounds and pins its generation before route access.
pub(super) async fn admit_request<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
) -> Result<AdmittedSearch, SearchError> {
    let started = Instant::now();
    let understood = validate(request)?;
    let (structured_request, structured_error) = match inventory_request(&understood) {
        Ok(request) => (request, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let version = request.version.map(str::to_owned);
    let cutoffs =
        deadline::from_budget(started, request.budget, request.configuration.stage_window);
    let (admission_scopes, generation, version_documented) = admit(
        context.database.clone(),
        context.principal,
        request.collection,
        version.clone(),
        cutoffs.expires,
    )
    .await?;
    Ok(AdmittedSearch {
        understood,
        structured_request,
        structured_error,
        version,
        version_documented,
        generation,
        principal: context.principal.to_owned(),
        scopes: Arc::new(admission_scopes),
        cutoffs,
        configuration: request.configuration,
    })
}

/// Checks all request bounds and performs deterministic local understanding.
pub(super) fn validate(request: &SearchRequest<'_>) -> Result<Understood, SearchError> {
    let invalid = |reason: &str| SearchError::InvalidRequest {
        reason: reason.to_owned(),
    };
    if request.text.len() > 8192 {
        return Err(invalid("query exceeds 8192 UTF-8 bytes"));
    }
    let understood = understand(request.text);
    if understood.normalized.is_empty() {
        return Err(invalid("query must not be blank"));
    }
    if HashSet::<&str>::from_iter(
        understood
            .identifiers
            .iter()
            .map(|identifier| identifier.text.as_str()),
    )
    .len()
        > 64
    {
        return Err(invalid("query has more than 64 distinct identifiers"));
    }
    if !(1..=50).contains(&request.budget.k) {
        return Err(invalid("k must be between 1 and 50"));
    }
    if !(1..=12_000).contains(&request.budget.max_tokens) {
        return Err(invalid("max_tokens must be between 1 and 12000"));
    }
    if !(1..=RequestBudget::MAX_DEADLINE_MS).contains(&request.budget.deadline_ms) {
        return Err(invalid("deadline_ms must be between 1 and 30000"));
    }
    if request.configuration.rerank_depth.get() > 120 {
        return Err(invalid("rerank depth must be between 1 and 120"));
    }
    if !request.configuration.weights_are_valid() {
        return Err(invalid("route weights must be finite and nonnegative"));
    }
    if request
        .configuration
        .rerank_blend
        .is_some_and(|blend| !blend.is_finite() || !(0.0..=1.0).contains(&blend))
    {
        return Err(invalid("rerank blend must be between 0 and 1"));
    }
    if !request.configuration.section_prior.is_valid() {
        return Err(invalid("section prior weight must be between 0 and 1"));
    }
    if let CandidateContext::BoundedSection { max_bytes } = request.configuration.candidate_context
        && !(1..=1500).contains(&max_bytes)
    {
        return Err(invalid(
            "candidate context max_bytes must be between 1 and 1500",
        ));
    }
    if request
        .version
        .is_some_and(|version| version.is_empty() || version.len() > 256)
    {
        return Err(invalid("version must contain 1 to 256 UTF-8 bytes"));
    }
    Ok(understood)
}

/// Resolves current scopes and pins a published generation off the async executor.
async fn admit(
    database: Arc<Database>,
    principal: &str,
    collection: &str,
    version: Option<String>,
    deadline: Instant,
) -> Result<(ScopeSet, Generation, bool), SearchError> {
    let principal = principal.to_owned();
    let collection = collection.to_owned();
    match deadline::run_blocking(deadline, move |cancelled| {
        let scopes = database
            .visible(&principal)
            .map_err(|error| SearchError::Kernel(retrieval::Error::Store(error)))?;
        let generation = pin(&database, &scopes, &collection).map_err(SearchError::Admission)?;
        let version_documented = if let Some(version) = version.as_deref() {
            let control = retrieval::ReadControl {
                deadline: deadline.into_std(),
                cancelled,
            };
            let read = retrieval::SearchRead {
                generation: &generation,
                scopes: &scopes,
                version: Some(version),
                control: &control,
            };
            database
                .version_exists(&read)
                .map_err(SearchError::Kernel)?
        } else {
            true
        };
        Ok((scopes, generation, version_documented))
    })
    .await
    {
        Ok(result) => result,
        Err(deadline::BlockingFailure::TimedOut) => Err(SearchError::AdmissionTimedOut),
        Err(deadline::BlockingFailure::WorkerFailed) => Err(SearchError::WorkerFailed),
    }
}

/// Re-reads permissions on a bounded worker and requires the admitted snapshot.
pub(super) async fn ensure_permissions(
    database: Arc<Database>,
    principal: &str,
    admitted: &ScopeSet,
    deadline: Instant,
) -> Result<(), SearchError> {
    let principal = principal.to_owned();
    let scopes = match deadline::run_blocking(deadline, move |_| {
        database
            .visible(&principal)
            .map_err(|error| SearchError::Kernel(retrieval::Error::Store(error)))
    })
    .await
    {
        Ok(result) => result?,
        Err(deadline::BlockingFailure::TimedOut) => {
            return Err(SearchError::PermissionCheckTimedOut);
        }
        Err(deadline::BlockingFailure::WorkerFailed) => return Err(SearchError::WorkerFailed),
    };
    if scopes != *admitted {
        return Err(SearchError::PermissionsChanged);
    }
    Ok(())
}
