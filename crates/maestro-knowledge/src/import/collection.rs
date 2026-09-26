//! Importing a collection: its declaration recorded, then the manifest of
//! each of its sources in the declared order, then its completion journaled
//! with its counts.

use super::{
    corpus::{Corpus, Files},
    entry::Target,
    error::Error,
    report::Report,
    source::import_source,
};
use crate::collection::{Declaration, Source, SourceKind, Visibility};
use maestro_kernel::{
    binding::Bindings,
    document, journal,
    journal::NewEvent,
    scope::{Scope, ScopeSet},
    store::Database,
};
use serde_json::json;
use std::collections::BTreeMap;

/// The type of the event an import journals when it completes.
const COMPLETED: &str = "maestro.knowledge.import.completed.v1";

/// Imports the corpus manifest of each source of `declaration`, found
/// through `bindings`, for a caller who reads `scopes`, and returns its
/// report. An entry is refused, with its line and reason, when its line is
/// no entry or its document is not what the line declares, and the import
/// goes on; the revisions of lines of one manifest that give their
/// `source_ref` different digests are all recorded and all held back. The
/// import holds one manifest line and one document in memory at a time,
/// beside one digest per `source_ref` of the manifest in progress. It is
/// idempotent: a second import finds every revision unchanged and writes
/// nothing but its completion, and one rerun after a failure continues where
/// the first stopped.
///
/// # Errors
///
/// Before any work, [`Error::Binding`] when a manifest's binding is not
/// bound, and [`Error::NotVisible`] when `scopes` does not cover the scope of
/// each source, whose records the import reads and writes. Part way,
/// [`Error::Manifest`] when a manifest cannot be read, and
/// [`Error::Records`], [`Error::Artifacts`] or [`Error::Journal`] when the
/// kernel fails: what was recorded before stays, and no completion is
/// journaled.
pub fn import(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
    bindings: &Bindings,
) -> Result<Report, Error> {
    let corpora: Vec<(&Source, Files)> = declaration
        .manifest_paths(bindings)
        .map_err(Error::Binding)?
        .into_iter()
        .map(|(source, manifest)| (source, Files::new(manifest)))
        .collect();
    import_corpora(database, scopes, declaration, &corpora)
}

/// [`import`], each source's corpus given with it.
pub(super) fn import_corpora<C: Corpus>(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
    corpora: &[(&Source, C)],
) -> Result<Report, Error> {
    let hidden = declaration
        .sources
        .iter()
        .map(|source| source_scope(&declaration.id, &source.id))
        .find(|scope| !visible(scopes, scope));
    if let Some(scope) = hidden {
        return Err(Error::NotVisible(scope));
    }
    record_declaration(database, scopes, declaration).map_err(Error::Records)?;
    let mut report = Report::new(&declaration.id);
    for (source, corpus) in corpora {
        let target = Target {
            database,
            scopes,
            collection: &declaration.id,
            source: &source.id,
        };
        import_source(&target, corpus, &mut report)?;
    }
    journal_completion(database, &report).map_err(Error::Journal)?;
    Ok(report)
}

/// The scope of the source `source` of the collection `collection`.
fn source_scope(collection: &str, source: &str) -> String {
    format!("workspace/default/collection/{collection}/source/{source}")
}

/// Whether `scopes` covers the scope `path`.
fn visible(scopes: &ScopeSet, path: &str) -> bool {
    path.parse::<Scope>()
        .is_ok_and(|scope| scopes.covers(&scope))
}

/// Records the collection `declaration` declares and each of its sources,
/// unless the kernel records them already just so: before any revision, and
/// without a write when nothing changed.
fn record_declaration(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
) -> Result<(), document::Error> {
    let collection = collection_record(declaration);
    if database.collection(scopes, &collection.id)?.as_ref() != Some(&collection) {
        database.record_collection(&collection)?;
    }
    for source in &declaration.sources {
        let source = source_record(&declaration.id, source);
        let recorded = database.source(scopes, &source.collection_id, &source.id)?;
        if recorded.as_ref() != Some(&source) {
            database.record_source(&source)?;
        }
    }
    Ok(())
}

/// The record of the collection `declaration` declares.
fn collection_record(declaration: &Declaration) -> document::Collection {
    let profiles = &declaration.profiles;
    let stages = [
        ("extraction", &profiles.extraction),
        ("chunking", &profiles.chunking),
        ("embedding", &profiles.embedding),
        ("sparse", &profiles.sparse),
    ];
    let visibility = match declaration.visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
    };
    document::Collection {
        id: declaration.id.clone(),
        title: declaration.title.clone(),
        visibility: visibility.to_owned(),
        profiles: stages
            .into_iter()
            .map(|(stage, profile)| (stage.to_owned(), profile.clone()))
            .collect(),
    }
}

/// The record of `source`, declared by the collection `collection`: an
/// import, with no transport, from the manifest its binding and path name.
fn source_record(collection: &str, source: &Source) -> document::Source {
    let kind = match source.kind {
        SourceKind::Import => "import",
    };
    document::Source {
        collection_id: collection.to_owned(),
        id: source.id.clone(),
        kind: kind.to_owned(),
        transport: None,
        reference: format!(
            "{}:{}",
            source.manifest.binding,
            source.manifest.path.as_str()
        ),
        profiles: BTreeMap::new(),
    }
}

/// Journals the completion of the import `report` tells of, with its
/// counts, on the stream of its collection, in the collection's scope.
fn journal_completion(database: &Database, report: &Report) -> Result<(), journal::Error> {
    let collection = format!("collection/{}", report.collection);
    let data = json!({
        "collection": report.collection,
        "imported": report.imported,
        "unchanged": report.unchanged,
        "held": report.held,
        "refused": report.refused,
    });
    database.record(&NewEvent {
        stream: &collection,
        r#type: COMPLETED,
        subject: &collection,
        scope: &format!("workspace/default/{collection}"),
        data: &data,
    })?;
    Ok(())
}
