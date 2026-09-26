//! `knowledge status`: a collection's documents, its revisions by
//! canonicalization's status and by quality disposition, and its
//! generations, as the local principal reads them.

use super::{failure::Failure, kernel::Kernel, output::Output};
use maestro_kernel::{document::Counts, generation::Generation};
use serde::Serialize;
use std::{collections::BTreeMap, fmt::Display, process::ExitCode};

/// The schema of the document `knowledge status` prints under `--json`.
const SCHEMA: &str = "maestro-cli/knowledge-status/1";

/// What `knowledge status` prints under `--json`.
#[derive(Debug, Serialize)]
struct StatusDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// The collection's ID.
    collection: &'a str,
    /// Its title.
    title: &'a str,
    /// How many documents it holds.
    documents: u64,
    /// How many of its revisions have each status.
    revisions: BTreeMap<String, u64>,
    /// How many of its revisions have each quality disposition, and how many
    /// none yet, as `undecided`.
    dispositions: BTreeMap<String, u64>,
    /// Its generations, in the order they were created.
    generations: Vec<GenerationDocument<'a>>,
}

/// A generation, as `knowledge status` prints it under `--json`.
#[derive(Debug, Serialize)]
struct GenerationDocument<'a> {
    /// Its ID.
    id: i64,
    /// Where it is in its lifecycle.
    state: String,
    /// The chunk set it is built from.
    chunk_set: &'a str,
    /// Its embedding profile.
    embedding_profile: &'a str,
    /// Its sparse profile.
    sparse_profile: &'a str,
    /// How many points its verification counted, once verified.
    point_count: Option<u64>,
    /// When it was published, if it was.
    published_at: Option<&'a str>,
}

/// Prints the status of the collection `collection`.
///
/// # Errors
///
/// [`Failure::Refused`] when the local principal reads no collection
/// `collection`, and [`Failure::Failed`] when the kernel fails.
pub(super) fn run(kernel: &Kernel, output: Output, collection: &str) -> Result<ExitCode, Failure> {
    let recorded = kernel
        .database
        .collection(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused(format!(
                "no collection {collection} is recorded that the local principal reads"
            ))
        })?;
    let counts = kernel
        .database
        .collection_counts(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?;
    let generations = kernel
        .database
        .generations(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?;
    let dispositions = dispositions(&counts);
    let document = StatusDocument {
        schema: SCHEMA,
        collection: &recorded.id,
        title: &recorded.title,
        documents: counts.documents,
        revisions: named(&counts.statuses).collect(),
        dispositions: dispositions.iter().cloned().collect(),
        generations: generations.iter().map(generation_document).collect(),
    };
    let mut text = vec![
        format!("collection {}: {}", recorded.id, recorded.title),
        format!("documents {}", counts.documents),
        format!("revisions {}", listed(named(&counts.statuses))),
        format!("dispositions {}", listed(dispositions.into_iter())),
    ];
    text.extend(generations.iter().map(generation_line));
    if generations.is_empty() {
        text.push("generations none".to_owned());
    }
    output.result(&document, &text.join("\n"))?;
    Ok(ExitCode::SUCCESS)
}

/// Each count of `counts` with its key's name, in their order.
fn named<K: Display>(counts: &[(K, u64)]) -> impl Iterator<Item = (String, u64)> + '_ {
    counts.iter().map(|(key, count)| (key.to_string(), *count))
}

/// The revisions of `counts` by quality disposition, in the order of the
/// outcomes, then those without one, `undecided`.
fn dispositions(counts: &Counts) -> Vec<(String, u64)> {
    let mut dispositions: Vec<_> = named(&counts.outcomes).collect();
    dispositions.push(("undecided".to_owned(), counts.undecided));
    dispositions
}

/// `counts` as people read them: `name count`, separated by commas.
fn listed(counts: impl Iterator<Item = (String, u64)>) -> String {
    counts
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `generation` as the document prints it.
fn generation_document(generation: &Generation) -> GenerationDocument<'_> {
    GenerationDocument {
        id: generation.id,
        state: generation.state.to_string(),
        chunk_set: &generation.chunk_set_id,
        embedding_profile: &generation.embedding_profile,
        sparse_profile: &generation.sparse_profile,
        point_count: generation.point_count,
        published_at: generation.published_at.as_deref(),
    }
}

/// `generation` as people read it: its ID, its state, its chunk set, and
/// its points once counted.
fn generation_line(generation: &Generation) -> String {
    let points = generation
        .point_count
        .map_or_else(String::new, |count| format!(", {count} points"));
    format!(
        "generation {} {}: chunk set {}{points}",
        generation.id, generation.state, generation.chunk_set_id
    )
}
