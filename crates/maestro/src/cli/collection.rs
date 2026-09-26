//! `knowledge collection add`: a collection's declaration parsed strictly,
//! the collection and its sources recorded as an import records them, the
//! declaration's bytes kept as a pinned artifact, and its addition journaled
//! as `maestro.knowledge.collection.added.v1`; and the declaration a later
//! command finds for a collection, the one added last.

use super::{failure::Failure, kernel::Kernel, output::Output};
use maestro_kernel::{
    artifact::Digest,
    journal::{self, Filter, NewEvent},
    scope::Scope,
};
use maestro_knowledge::{
    collection::{self, Declaration},
    import,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{self, Path, PathBuf},
    process::ExitCode,
    str,
};

/// The type of the event that records a declaration added: internal to the
/// kernel's journal, never one of the public events, so the absolute path
/// it carries stays on this machine.
const ADDED: &str = "maestro.knowledge.collection.added.v1";
/// The media type of a declaration's artifact.
const MEDIA: &str = "application/json";
/// The schema of the document `collection add` prints under `--json`.
const SCHEMA: &str = "maestro-cli/collection-add/1";

/// A declaration added, as the event of its addition carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Added {
    /// The collection it declares.
    collection: String,
    /// The digest of its bytes, the artifact that keeps them.
    declaration: String,
    /// The file it was read from, absolute: its quality ledger and its
    /// evaluation suite are relative to its directory.
    path: String,
}

/// What `collection add` prints under `--json`.
#[derive(Debug, Serialize)]
struct AddedDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// The collection's ID.
    collection: &'a str,
    /// The digest of the declaration's bytes.
    declaration: &'a str,
    /// The file it was read from, absolute.
    path: &'a str,
    /// Its sources' IDs, in the declared order.
    sources: Vec<&'a str>,
    /// Whether this declaration, from this file, was not the one added last.
    changed: bool,
}

/// A collection's declaration, as it was added last.
#[derive(Debug)]
pub(super) struct Declared {
    /// The declaration.
    pub(super) declaration: Declaration,
    /// The digest of its bytes.
    pub(super) digest: Digest,
    /// The file it was added from, absolute: its quality ledger and its
    /// evaluation suite are relative to its directory.
    pub(super) path: PathBuf,
}

/// Adds the collection the declaration `file` declares, and prints what it
/// did; adding the same declaration from the same file again changes
/// nothing.
///
/// # Errors
///
/// [`Failure::Refused`] when the file cannot be read, is not a strict
/// declaration, or declares a collection the local principal cannot read,
/// and [`Failure::Failed`] when the kernel fails.
pub(super) fn add(kernel: &Kernel, output: Output, file: &Path) -> Result<ExitCode, Failure> {
    let bytes = fs::read(file)
        .map_err(|error| Failure::refused(format!("cannot read {}: {error}", file.display())))?;
    let declaration = parse(&bytes).map_err(Failure::refused)?;
    let scope = collection_scope(&declaration.id)?;
    if !kernel.scopes.covers(&scope) {
        return Err(Failure::refused(format!(
            "the local principal cannot read {scope}, where the collection's records go: grant \
             it in config.toml"
        )));
    }
    let absolute = path::absolute(file).map_err(|error| Failure::failed_by(&error))?;
    let path = absolute
        .to_str()
        .ok_or_else(|| Failure::refused(format!("the path {} is not UTF-8", absolute.display())))?;
    import::declare(&kernel.database, &kernel.scopes, &declaration)
        .map_err(|error| Failure::failed_by(&error))?;
    let digest = kernel
        .database
        .put(&bytes, MEDIA)
        .map_err(|error| Failure::failed_by(&error))?;
    let added = Added {
        collection: declaration.id.clone(),
        declaration: digest.as_str().to_owned(),
        path: path.to_owned(),
    };
    let changed = last_added(kernel, &declaration.id)?.as_ref() != Some(&added);
    if changed {
        record(kernel, &scope, &added)?;
    }
    let sources: Vec<&str> = declaration
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect();
    let text = format!(
        "collection {} {}: declaration {}, sources {}",
        declaration.id,
        if changed { "added" } else { "unchanged" },
        digest.as_str(),
        sources.join(", ")
    );
    let document = AddedDocument {
        schema: SCHEMA,
        collection: &declaration.id,
        declaration: digest.as_str(),
        path,
        sources,
        changed,
    };
    output.result(&document, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// The declaration of the collection `collection` that was added last, read
/// through the local principal's scopes.
///
/// # Errors
///
/// [`Failure::Refused`] when none was added that the principal reads, and
/// [`Failure::Failed`] when the kernel fails or no longer holds it as added.
pub(super) fn declared(kernel: &Kernel, collection: &str) -> Result<Declared, Failure> {
    let added = last_added(kernel, collection)?.ok_or_else(|| {
        Failure::refused(format!(
            "no collection {collection} was added that the local principal reads: add it with \
             `maestro knowledge collection add <collection.json>`"
        ))
    })?;
    let digest = Digest::parse(&added.declaration).map_err(|error| Failure::failed_by(&error))?;
    let bytes = kernel
        .database
        .get(&digest)
        .map_err(|error| Failure::failed_by(&error))?;
    let declaration = parse(&bytes).map_err(Failure::failed)?;
    Ok(Declared {
        declaration,
        digest,
        path: PathBuf::from(added.path),
    })
}

/// The declaration `bytes` hold, or why they hold none.
fn parse(bytes: &[u8]) -> Result<Declaration, String> {
    let text = str::from_utf8(bytes).map_err(|error| {
        format!("not a strict maestro-collection/1 declaration: not UTF-8: {error}")
    })?;
    text.parse()
        .map_err(|error: collection::Error| error.to_string())
}

/// The scope of the collection `id`, which its records have.
pub(super) fn collection_scope(id: &str) -> Result<Scope, Failure> {
    format!("workspace/default/collection/{id}")
        .parse()
        .map_err(|error| Failure::refused_by(&error))
}

/// The addition of the collection `collection` journaled last, if the local
/// principal reads one.
fn last_added(kernel: &Kernel, collection: &str) -> Result<Option<Added>, Failure> {
    let stream = journal::stream(collection);
    let filter = Filter {
        stream: &stream,
        after: 0,
        r#type: Some(ADDED),
    };
    let events = kernel
        .database
        .events(&kernel.scopes, &filter)
        .map_err(|error| Failure::failed_by(&error))?;
    events
        .last()
        .map(|event| serde_json::from_value(event.data.clone()))
        .transpose()
        .map_err(|error| Failure::failed_by(&error))
}

/// Pins the declaration `added` names and journals its addition in `scope`.
fn record(kernel: &Kernel, scope: &Scope, added: &Added) -> Result<(), Failure> {
    let digest = Digest::parse(&added.declaration).map_err(|error| Failure::failed_by(&error))?;
    kernel
        .database
        .pin(&digest)
        .map_err(|error| Failure::failed_by(&error))?;
    let stream = journal::stream(&added.collection);
    let data = serde_json::to_value(added).map_err(|error| Failure::failed_by(&error))?;
    kernel
        .database
        .record(&NewEvent {
            stream: &stream,
            r#type: ADDED,
            subject: &stream,
            scope: scope.as_str(),
            data: &data,
        })
        .map_err(|error| Failure::failed_by(&error))?;
    Ok(())
}
