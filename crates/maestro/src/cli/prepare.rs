//! `knowledge prepare`: T023's chunking as a leased job.

use super::{
    collection, foreground, health,
    output::Output,
    wait::{self, Printing},
};
use crate::{
    failure::{Failure, chain},
    kernel::Kernel,
};
use maestro_kernel::{
    gateway::{ModelCard, RouterClient},
    job::{JobState, NewJob},
};
use maestro_knowledge::prepare::{
    self, ChunkProfile, Preparation, RouterTokenizer, TokenizerQualification,
};
use serde_json::json;
use std::{env, ops::ControlFlow, process::ExitCode};

/// The kind of the job preparation runs.
const KIND: &str = "knowledge.prepare";
/// The schema of `knowledge prepare`'s JSON document.
const SCHEMA: &str = "maestro-cli/knowledge-prepare/1";
/// How `knowledge prepare` prints its ended job.
const PRINTING: Printing = Printing {
    schema: SCHEMA,
    text: wait::line,
};

/// Prepares `collection` with the recorded embedder card `card_digest` under
/// the chunking profile `profile_name`, the default one when none is named,
/// as a job, or finds the job with the same frozen inputs.
pub(super) fn run(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    card_digest: &str,
    profile_name: Option<&str>,
) -> Result<ExitCode, Failure> {
    let preparation = Preparation {
        collection: collection_id,
        profile: chunk_profile(profile_name)?,
    };
    collection::declared(kernel, collection_id)?;
    let card = kernel.embedder_card(card_digest)?;
    let chunk_set =
        prepare::chunk_set_id_for_card(&kernel.database, &kernel.scopes, preparation, &card)
            .map_err(|error| Failure::failed_by(&error))?;
    let router_url =
        health::router_url(env::var_os(health::ROUTER_VARIABLE).as_deref()).map_err(|text| {
            Failure::refused(format!("{} is not a URL: {text}", health::ROUTER_VARIABLE))
        })?;
    let inputs = json!({
        "card": card.digest().as_str(),
        "chunk_set": chunk_set,
        "collection": collection_id,
    });
    let scope = collection::collection_scope(collection_id)?;
    let resource = format!("collection/{collection_id}/prepare");
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    foreground::run(
        kernel,
        output,
        &new,
        |holder| {
            let client = match RouterClient::new(router_url) {
                Ok(client) => client,
                Err(error) => return (JobState::Failed, json!({"error": chain(&error)})),
            };
            let tokenizer = match qualify_tokenizer(kernel, client, card) {
                Ok(tokenizer) => tokenizer,
                Err(error) => return (JobState::Failed, json!({"error": error})),
            };
            let mut stopped = None;
            let prepared = prepare::prepare_observed(
                &kernel.database,
                &kernel.scopes,
                preparation,
                &tokenizer,
                &mut |report| {
                    let data = match serde_json::to_value(report) {
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
                    drop(output.text(&format!("step {data}")));
                    ControlFlow::Continue(())
                },
            );
            match (prepared, stopped) {
                (_, Some(error)) => (JobState::Failed, json!({"error": error})),
                (Ok(report), None) => (JobState::Succeeded, json!(report)),
                (Err(error), None) => (JobState::Failed, json!({"error": chain(&error)})),
            }
        },
        PRINTING,
    )
}

/// The chunking profile whose chunker version is `name`, the default one when
/// none is named; refused, naming the known profiles, when it is unknown.
pub(super) fn chunk_profile(name: Option<&str>) -> Result<ChunkProfile, Failure> {
    let Some(name) = name else {
        return Ok(ChunkProfile::default());
    };
    ChunkProfile::named(name).ok_or_else(|| {
        let known: Vec<_> = ChunkProfile::ALL
            .iter()
            .map(|profile| profile.chunker_version())
            .collect();
        Failure::refused(format!(
            "no chunking profile is named {name}; the profiles are {}",
            known.join(", ")
        ))
    })
}

/// Qualifies a legacy card through its original path or loads and checks v2 evidence.
fn qualify_tokenizer(
    kernel: &Kernel,
    client: RouterClient,
    card: ModelCard,
) -> Result<RouterTokenizer, String> {
    let Some(identity) = card.identity() else {
        return RouterTokenizer::qualify(client, card).map_err(|error| chain(&error));
    };
    let bytes = kernel
        .artifacts
        .get(&identity.formats.qualification_digest)
        .map_err(|error| chain(&error))?;
    let profile = TokenizerQualification::parse(&bytes).map_err(|error| chain(&error))?;
    RouterTokenizer::qualify_with_profile(client, card, &profile).map_err(|error| chain(&error))
}
