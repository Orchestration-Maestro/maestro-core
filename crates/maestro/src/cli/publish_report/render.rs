//! Reconciled output for the historical publication and current projection.

use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    gateway::ModelCard,
    generation::Generation,
    job::{Job, JobState},
    retrieval::IDENTIFIER_PROFILE,
};
use maestro_knowledge::{index::Qdrant, lexical};
use serde::Serialize;
use serde_json::Value;
use std::process::ExitCode;
use tokio::runtime::Builder;

use crate::cli::output::Output;

/// The schema of `knowledge publish`'s JSON document.
pub(in crate::cli) const SCHEMA: &str = "maestro-cli/knowledge-publish/1";

/// The inputs needed to report a completed or resumed publication.
#[derive(Clone, Copy)]
pub(in crate::cli) struct ReportRequest<'a> {
    /// The historical publication job.
    pub(in crate::cli) job: &'a Job,
    /// The tuple frozen in that job.
    pub(in crate::cli) inputs: &'a Value,
    /// The restored embedder card.
    pub(in crate::cli) card: &'a ModelCard,
    /// The Qdrant endpoint holding the current projection.
    pub(in crate::cli) qdrant_url: &'a str,
    /// Whether the unfinished job was resumed.
    pub(in crate::cli) resuming: bool,
}

/// The JSON fields reconciling historical job state with current health.
#[derive(Serialize)]
pub(in crate::cli) struct PublishDocument<'a> {
    /// The document schema.
    pub(in crate::cli) schema: &'static str,
    /// The historical job ID.
    pub(in crate::cli) job: String,
    /// The kind of job.
    pub(in crate::cli) kind: &'a str,
    /// The attempt number.
    pub(in crate::cli) attempt: u64,
    /// The historical job state.
    pub(in crate::cli) state: String,
    /// The immutable historical outcome.
    pub(in crate::cli) outcome: &'a Value,
    /// The generation recorded by the historical outcome.
    pub(in crate::cli) historical_generation: Option<i64>,
    /// The currently published generation.
    pub(in crate::cli) current_generation: Option<i64>,
    /// Whether the current projection and alias are ready.
    pub(in crate::cli) projection_ready: bool,
    /// Whether this request resumed unfinished work.
    pub(in crate::cli) resuming: bool,
}

/// Reconciles a historical job with the currently published projection.
pub(in crate::cli) fn render(
    kernel: &Kernel,
    output: Output,
    request: ReportRequest<'_>,
) -> Result<ExitCode, Failure> {
    let ReportRequest {
        job,
        inputs,
        card,
        qdrant_url,
        resuming,
    } = request;
    let current = kernel
        .database
        .published_generation(
            &kernel.scopes,
            inputs["collection"].as_str().unwrap_or_default(),
        )
        .map_err(|error| Failure::failed_by(&error))?;
    let current_generation = current.as_ref().map(|generation| generation.id);
    let tuple_matches = current.as_ref().is_some_and(|generation| {
        generation.chunk_set_id == inputs["chunk_set"].as_str().unwrap_or_default()
            && generation.embedding_profile == format!("dense/1:sha256:{}", card.digest().as_str())
            && generation.sparse_profile == lexical::PROFILE
    });
    let search_ready = if let Some(generation) = current.as_ref().filter(|_| tuple_matches) {
        kernel
            .database
            .generation_search(&kernel.scopes, generation.id)
            .map_err(|error| Failure::failed_by(&error))?
            .is_some_and(|search| search.ready && search.identifier_profile == IDENTIFIER_PROFILE)
    } else {
        false
    };
    let projection_ready = if let Some(generation) = current.as_ref().filter(|_| search_ready) {
        let collection = generation_collection(generation);
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Failure::failed_by(&error))?;
        // The publication runtime is gone, so do not reuse its tonic channel.
        let qdrant = Qdrant::new(qdrant_url).map_err(|error| Failure::failed_by(&error))?;
        runtime
            .block_on(qdrant.exists(&collection))
            .map_err(|error| Failure::failed_by(&error))?
            && runtime
                .block_on(qdrant.alias_collection(&generation_alias(generation)))
                .map_err(|error| Failure::failed_by(&error))?
                .as_deref()
                == Some(collection.as_str())
    } else {
        false
    };
    let outcome = job.outcome.as_ref().unwrap_or(&Value::Null);
    let historical_generation = outcome.get("generation").and_then(Value::as_i64);
    let document = PublishDocument {
        schema: SCHEMA,
        job: job.id.to_string(),
        kind: &job.kind,
        attempt: job.attempt,
        state: job.state.to_string(),
        outcome,
        historical_generation,
        current_generation,
        projection_ready,
        resuming,
    };
    let text = publish_line(job, &document, inputs, card);
    output.result(&document, &text)?;
    Ok(if job.state == JobState::Succeeded && projection_ready {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// The text equivalent of the JSON reconciliation fields.
pub(in crate::cli) fn publish_line(
    job: &Job,
    document: &PublishDocument<'_>,
    inputs: &Value,
    card: &ModelCard,
) -> String {
    let mut line = format!(
        "{}: historical generation {}, current generation {}, projection {}",
        job.state,
        generation_label(document.historical_generation),
        generation_label(document.current_generation),
        if document.projection_ready {
            "ready"
        } else {
            "not ready"
        }
    );
    if document.resuming {
        line.push_str("; resumed unfinished publication");
    }
    if job.state == JobState::Succeeded && !document.projection_ready {
        let collection = inputs["collection"].as_str().unwrap_or_default();
        let chunk_set = inputs["chunk_set"].as_str().unwrap_or_default();
        line.push_str("; next: run `");
        line.push_str(&recovery_command(collection, chunk_set, card));
        line.push('`');
    } else if job.state != JobState::Succeeded {
        line.push_str("; outcome ");
        line.push_str(&job.outcome.as_ref().unwrap_or(&Value::Null).to_string());
    }
    line
}

/// Names the explicit recovery action without guessing a newer tuple.
pub(in crate::cli) fn recovery_command(
    collection: &str,
    chunk_set: &str,
    card: &ModelCard,
) -> String {
    format!(
        "maestro knowledge publish --collection {collection} --chunk-set {chunk_set} \
         --card {} --again",
        card.digest().as_str()
    )
}

/// Formats the generation ID for the human-readable reconciliation line.
pub(in crate::cli) fn generation_label(generation: Option<i64>) -> String {
    generation.map_or_else(|| "none".to_owned(), |id| id.to_string())
}

/// Names the Qdrant collection for a generation.
pub(in crate::cli) fn generation_collection(generation: &Generation) -> String {
    format!("maestro-{}-g{}", generation.collection_id, generation.id)
}

/// Names the collection alias for a generation.
pub(in crate::cli) fn generation_alias(generation: &Generation) -> String {
    format!("maestro-{}", generation.collection_id)
}
