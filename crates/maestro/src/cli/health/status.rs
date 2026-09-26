//! `maestro status`: which services are ready, the kernel, Qdrant and the
//! model router, and the collections the local principal reads, with their
//! documents and published generation. It neither creates nor migrates the
//! kernel, exits 0 whatever is down, and leaves why and what to do to
//! `maestro doctor`.

use super::{
    check::Check,
    kernel::{Opened, config_check, database_check},
    services::{ROUTER_VARIABLE, qdrant_address, qdrant_check, router_check, router_url},
};
use crate::cli::{failure::Failure, output::Output, setup};
use maestro_kernel::{
    generation::GenerationState,
    paths::{self, Environment},
};
use serde::Serialize;
use std::{env, process::ExitCode};

/// The schema of the document `status` prints under `--json`.
const SCHEMA: &str = "maestro-cli/status/1";

/// What `status` prints under `--json`.
#[derive(Debug, Serialize)]
struct StatusDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// The kernel, Qdrant and the model router, in that order.
    services: Vec<ServiceDocument<'a>>,
    /// The collections the local principal reads, in id order.
    collections: Vec<CollectionDocument>,
}

/// A service, as `status` prints it under `--json`.
#[derive(Debug, Serialize)]
struct ServiceDocument<'a> {
    /// Its name.
    name: &'a str,
    /// Where it was looked for.
    target: &'a str,
    /// Whether it is ready.
    ready: bool,
    /// What its check saw, or what is wrong.
    detail: &'a str,
}

/// A collection, as `status` prints it under `--json`.
#[derive(Debug, Serialize)]
struct CollectionDocument {
    /// Its ID.
    collection: String,
    /// Its title.
    title: String,
    /// How many of its documents the local principal reads.
    documents: u64,
    /// Its published generation, if it has one.
    published: Option<PublishedDocument>,
}

/// A published generation, as `status` prints it under `--json`.
#[derive(Debug, Serialize)]
struct PublishedDocument {
    /// Its ID.
    generation: i64,
    /// How many points its verification counted.
    points: Option<u64>,
}

/// Prints which services are ready and the collections the local principal
/// reads.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel's directories cannot be resolved, or
/// its database, once it opened, cannot be read.
pub(in crate::cli) fn run(output: Output) -> Result<ExitCode, Failure> {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let config_dir = paths::config_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let (config, read) = config_check(&config_dir);
    let (database, opened) = database_check(&data, read.as_ref());
    let kernel = Check {
        name: "kernel",
        ..if config.next().is_some() {
            config
        } else {
            database
        }
    };
    let services = [
        kernel,
        qdrant_check(&qdrant_address(), || setup::readiness(&environment)),
        router_check(router_url(env::var_os(ROUTER_VARIABLE).as_deref())),
    ];
    let collections = match &opened {
        Some(opened) => collections(opened)?,
        None => Vec::new(),
    };
    let mut text: Vec<String> = services
        .iter()
        .map(|service| {
            let verdict = if service.next().is_some() {
                "down"
            } else {
                "ready"
            };
            format!(
                "{verdict:<6} {:<7} {}: {}",
                service.name,
                service.target,
                service.detail()
            )
        })
        .collect();
    text.extend(collections.iter().map(collection_line));
    if services.iter().any(|service| service.next().is_some()) {
        text.push("`maestro doctor` names the next action for what is down.".to_owned());
    }
    let document = StatusDocument {
        schema: SCHEMA,
        services: services
            .iter()
            .map(|service| ServiceDocument {
                name: service.name,
                target: &service.target,
                ready: service.next().is_none(),
                detail: service.detail(),
            })
            .collect(),
        collections,
    };
    output.result(&document, &text.join("\n"))?;
    Ok(ExitCode::SUCCESS)
}

/// The collections the local principal reads in `opened`, with their
/// documents and published generation; none without its grants.
fn collections(opened: &Opened) -> Result<Vec<CollectionDocument>, Failure> {
    let Some(scopes) = &opened.scopes else {
        return Ok(Vec::new());
    };
    let database = &opened.database;
    let mut documents = Vec::new();
    for collection in database
        .collections(scopes)
        .map_err(|error| Failure::failed_by(&error))?
    {
        let counts = database
            .collection_counts(scopes, &collection.id)
            .map_err(|error| Failure::failed_by(&error))?;
        let published = database
            .generations(scopes, &collection.id)
            .map_err(|error| Failure::failed_by(&error))?
            .into_iter()
            .find(|generation| generation.state == GenerationState::Published)
            .map(|generation| PublishedDocument {
                generation: generation.id,
                points: generation.point_count,
            });
        documents.push(CollectionDocument {
            collection: collection.id,
            title: collection.title,
            documents: counts.documents,
            published,
        });
    }
    Ok(documents)
}

/// `collection` as people read it.
fn collection_line(collection: &CollectionDocument) -> String {
    let published = collection.published.as_ref().map_or_else(
        || "no generation published".to_owned(),
        |published| format!("generation {} published", published.generation),
    );
    format!(
        "collection {}: {} documents, {published}",
        collection.collection, collection.documents
    )
}
