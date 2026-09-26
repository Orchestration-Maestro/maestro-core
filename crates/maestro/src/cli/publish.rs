//! `knowledge publish`: T026's verified Qdrant projection as a leased job.

use super::{
    collection,
    failure::{Failure, chain},
    foreground, health,
    kernel::Kernel,
    lease::Holder,
    output::Output,
    wait::{self, Printing},
};
use maestro_kernel::{
    chunk_set::ChunkSetState,
    gateway::{ModelCard, RouterClient, Url},
    job::{self, JobState, NewJob},
    journal::Filter,
};
use maestro_knowledge::{
    index::{Progress, Projection, Qdrant},
    lexical,
};
use serde_json::{Value, json};
use std::{env, ops::ControlFlow, process::ExitCode};
use tokio::runtime::Builder;
use ulid::Ulid;

/// The kind of the job a publication runs.
const KIND: &str = "knowledge.publish";
/// The schema of `knowledge publish`'s JSON document.
const SCHEMA: &str = "maestro-cli/knowledge-publish/1";
/// How `knowledge publish` prints its ended job.
const PRINTING: Printing = Printing {
    schema: SCHEMA,
    text: wait::line,
};

/// Publishes `chunk_set` of `collection` with the recorded embedder card
/// `card_digest`, or finds the job with the same frozen inputs.
pub(super) fn run(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    card_digest: &str,
    chunk_set: Option<&str>,
) -> Result<ExitCode, Failure> {
    collection::declared(kernel, collection_id)?;
    let card = kernel.embedder_card(card_digest)?;
    let set = match chunk_set {
        Some(id) => kernel
            .database
            .chunk_set(&kernel.scopes, id)
            .map_err(|error| Failure::failed_by(&error))?
            .filter(|set| set.collection_id == collection_id)
            .ok_or_else(|| {
                Failure::refused(format!("no chunk set {id} belongs to {collection_id}"))
            })?,
        None => kernel
            .database
            .latest_complete_chunk_set(&kernel.scopes, collection_id)
            .map_err(|error| Failure::failed_by(&error))?
            .ok_or_else(|| {
                Failure::refused(format!(
                    "collection {collection_id} has no complete chunk set"
                ))
            })?,
    };
    if set.state != ChunkSetState::Complete {
        return Err(Failure::refused(format!(
            "chunk set {} is {}, not complete",
            set.id, set.state
        )));
    }
    let router_url =
        health::router_url(env::var_os(health::ROUTER_VARIABLE).as_deref()).map_err(|text| {
            Failure::refused(format!("{} is not a URL: {text}", health::ROUTER_VARIABLE))
        })?;
    let qdrant_url =
        env::var(health::QDRANT_VARIABLE).unwrap_or_else(|_| health::DEFAULT_QDRANT.to_owned());
    let qdrant = Qdrant::new(&qdrant_url).map_err(|error| Failure::refused_by(&error))?;
    let chunk_set = set.id;
    let inputs = json!({
        "card": card.digest().as_str(),
        "chunk_set": chunk_set,
        "collection": collection_id,
        "sparse_profile": lexical::PROFILE,
    });
    let scope = collection::collection_scope(collection_id)?;
    let resource = format!("collection/{collection_id}/publish");
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let publication = Publication {
        kernel,
        output,
        chunk_set: &chunk_set,
        card: &card,
        qdrant: &qdrant,
        router_url,
    };
    foreground::run(
        kernel,
        output,
        &new,
        |holder| publication.run(holder),
        PRINTING,
    )
}

/// The publication's frozen and service inputs, used under a job lease.
struct Publication<'a> {
    /// The kernel this job records and reads.
    kernel: &'a Kernel,
    /// The command's output for publication steps.
    output: Output,
    /// The complete chunk set being published.
    chunk_set: &'a str,
    /// The recorded embedder card used for vectors.
    card: &'a ModelCard,
    /// The Qdrant service that receives the projection.
    qdrant: &'a Qdrant,
    /// The validated model-router address.
    router_url: Url,
}

impl Publication<'_> {
    /// Runs the publication, journaling each observer step before printing it.
    fn run(&self, holder: &Holder<'_>) -> (JobState, Value) {
        let resume = match last_progress(self.kernel, holder.job_id()) {
            Ok(resume) => resume,
            Err(error) => return (JobState::Failed, json!({"error": error.to_string()})),
        };
        let client = match RouterClient::new(self.router_url.clone()) {
            Ok(client) => client,
            Err(error) => return (JobState::Failed, json!({"error": chain(&error)})),
        };
        let projection = Projection {
            database: &self.kernel.database,
            scopes: &self.kernel.scopes,
            qdrant: self.qdrant,
            port: &client,
            card: self.card,
        };
        let runtime = match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                return (
                    JobState::Failed,
                    json!({"error": format!("cannot start publication runtime: {error}")}),
                );
            }
        };
        let mut stopped = None;
        let published = runtime.block_on(projection.publish_observed(
            self.chunk_set,
            resume.as_ref(),
            &mut |progress| {
                let data = match serde_json::to_value(progress) {
                    Ok(data) => data,
                    Err(error) => {
                        stopped = Some(chain(&error));
                        return ControlFlow::Break(());
                    }
                };
                if let Err(error) = holder.step(&data) {
                    stopped = Some(chain(&error));
                    return ControlFlow::Break(());
                }
                drop(self.output.text(&format!("step {data}")));
                ControlFlow::Continue(())
            },
        ));
        match (published, stopped) {
            (_, Some(error)) => (JobState::Failed, json!({"error": error})),
            (Ok(report), None) => (JobState::Succeeded, json!(report)),
            (Err(error), None) => (JobState::Failed, json!({"error": chain(&error)})),
        }
    }
}

/// The last publication step, including the previous attempt's step when a
/// retry has not yet journaled one of its own.
pub(super) fn last_progress(kernel: &Kernel, job_id: Ulid) -> Result<Option<Progress>, Failure> {
    let mut job_id = job_id;
    loop {
        if let Some(event) = kernel
            .database
            .last_progress(&kernel.scopes, job_id)
            .map_err(|error| Failure::failed_by(&error))?
        {
            return serde_json::from_value(event.data)
                .map(Some)
                .map_err(|error| Failure::failed_by(&error));
        }
        let stream = job::stream(job_id);
        let events = kernel
            .database
            .events(
                &kernel.scopes,
                &Filter {
                    stream: &stream,
                    after: 0,
                    r#type: Some(job::CREATED),
                },
            )
            .map_err(|error| Failure::failed_by(&error))?;
        let Some(previous) = events
            .last()
            .and_then(|event| event.data.get("previous_attempt"))
        else {
            return Ok(None);
        };
        let previous = previous.as_str().ok_or_else(|| {
            Failure::failed("a publication attempt has an invalid previous-attempt ID")
        })?;
        job_id = Ulid::from_string(previous).map_err(|error| Failure::failed_by(&error))?;
    }
}
