//! `knowledge publish`: a verified Qdrant projection as a leased job.

use super::recovery::{KIND, last_progress, selected_inputs};
use crate::cli::{
    args::PublishArguments,
    collection, foreground, health,
    lease::{self, Holder},
    output::Output,
    prepare::chunk_profile,
    publish_report,
};
use crate::{
    failure::{Failure, chain},
    kernel::Kernel,
};
use maestro_kernel::{
    chunk_set::ChunkSetState,
    gateway::{ModelCard, RouterClient, Url},
    job::{JobState, NewJob},
};
use maestro_knowledge::index::{Error as IndexError, Progress, Projection, Qdrant, RebuildGuard};
use serde_json::{Value, json};
use std::{env, ops::ControlFlow, process::ExitCode};
use tokio::runtime::Builder;

/// Publishes the requested complete set, or rebuilds it as a new generation.
pub(in crate::cli) fn run(
    kernel: &Kernel,
    output: Output,
    arguments: &PublishArguments,
) -> Result<ExitCode, Failure> {
    let collection_id = arguments.collection.as_str();
    if arguments.again && arguments.chunk_set.is_none() {
        return Err(Failure::refused(
            "knowledge publish --again requires --chunk-set",
        ));
    }
    collection::declared(kernel, collection_id)?;
    let card = kernel.embedder_card(&arguments.card)?;
    let set = if let Some(id) = arguments.chunk_set.as_deref() {
        kernel
            .database
            .chunk_set(&kernel.scopes, id)
            .map_err(|error| Failure::failed_by(&error))?
            .filter(|set| set.collection_id == collection_id)
            .ok_or_else(|| {
                Failure::refused(format!("no chunk set {id} belongs to {collection_id}"))
            })?
    } else {
        let profile = chunk_profile(arguments.chunk_profile.as_deref())?.chunker_version();
        kernel
            .database
            .latest_complete_chunk_set(&kernel.scopes, collection_id, profile)
            .map_err(|error| Failure::failed_by(&error))?
            .ok_or_else(|| {
                Failure::refused(format!(
                    "collection {collection_id} has no complete chunk set of {profile}"
                ))
            })?
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
    let resource = format!("collection/{collection_id}/publish");
    let (inputs, recovery, resuming) =
        selected_inputs(kernel, &resource, arguments, &card, &chunk_set)?;
    let scope = collection::collection_scope(collection_id)?;
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let publication = Publication {
        kernel,
        output,
        collection: collection_id,
        chunk_set: &chunk_set,
        card: &card,
        qdrant: &qdrant,
        router_url,
        recovery,
    };
    let job = foreground::run_to_end(kernel, output, &new, lease::TIMING, |holder| {
        publication.run(holder)
    })?;
    publish_report::render(
        kernel,
        output,
        publish_report::ReportRequest {
            job: &job,
            inputs: &inputs,
            card: &card,
            qdrant_url: &qdrant_url,
            resuming,
        },
    )
}

/// Frozen publication and service inputs used under the job lease.
struct Publication<'a> {
    /// The kernel read and written by the publication.
    kernel: &'a Kernel,
    /// Output mode selected by the caller.
    output: Output,
    /// Collection whose projection is being published.
    collection: &'a str,
    /// Complete chunk set being projected.
    chunk_set: &'a str,
    /// Embedder card for the dense vectors.
    card: &'a ModelCard,
    /// Qdrant endpoint client.
    qdrant: &'a Qdrant,
    /// Model-router endpoint.
    router_url: Url,
    /// Frozen recovery guard, if this is an explicit rebuild.
    recovery: Option<RebuildGuard>,
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
            projection: self.qdrant,
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
        let mut observer = |progress: &Progress| {
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
            drop(self.output.step(&data));
            ControlFlow::Continue(())
        };
        let published = match self.recovery {
            Some(recovery) => runtime.block_on(projection.republish_observed(
                self.chunk_set,
                recovery,
                resume.as_ref(),
                &mut observer,
            )),
            None => runtime.block_on(projection.publish_observed(
                self.chunk_set,
                resume.as_ref(),
                &mut observer,
            )),
        };
        match (published, stopped) {
            (_, Some(error)) => (JobState::Failed, json!({"error": error})),
            (Ok(report), None) => (JobState::Succeeded, json!(report)),
            (Err(error), None) => {
                let command =
                    publish_report::recovery_command(self.collection, self.chunk_set, self.card);
                let message = failure_message(&error, self.recovery.is_some(), &command);
                (JobState::Failed, json!({"error": message}))
            }
        }
    }
}

/// What a failed publication reports for `error`: a lost collection names
/// `recovery_command`, which replaces it, unless `recovering` already does.
pub(in crate::cli) fn failure_message(
    error: &IndexError,
    recovering: bool,
    recovery_command: &str,
) -> String {
    if !recovering && matches!(error, IndexError::MissingCollection(_)) {
        format!("{error}; recover explicitly with `{recovery_command}`")
    } else {
        chain(error)
    }
}
