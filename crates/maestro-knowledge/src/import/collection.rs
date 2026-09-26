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
    document,
    journal::{self, ImportCompleted, NewEvent},
    scope::{Scope, ScopeSet, collection_path, source_path},
    store::Database,
};
use serde_json::json;
use std::{collections::BTreeMap, ops::ControlFlow};

/// Imports the corpus manifest of each source of `declaration`, found
/// through `bindings`, for a caller who reads `scopes`, and returns its
/// report. An entry is refused, with its line and reason, when its line is
/// no entry or its document is not what the line declares, and the import
/// goes on; the revisions of lines of one manifest that give their
/// `source_ref` different digests are all recorded and all held back, each
/// new one with its hold in one write. The
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
    let mut unobserved = |_: &Report| ControlFlow::Continue(());
    import_observed(database, scopes, declaration, bindings, &mut unobserved)
}

/// [`import`], showing `observer` the report as it stands after every
/// hundredth line of each source's manifest and after the last line of
/// each, which a job journals as its progress. When `observer` breaks, the
/// import stops there with [`Error::Stopped`]: what it recorded stays, no
/// completion is journaled, and a rerun continues where it stopped.
///
/// # Errors
///
/// As [`import`], and [`Error::Stopped`].
pub fn import_observed(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
    bindings: &Bindings,
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<Report, Error> {
    let corpora: Vec<(&Source, Files)> = declaration
        .manifest_paths(bindings)
        .map_err(Error::Binding)?
        .into_iter()
        .map(|(source, manifest)| (source, Files::new(manifest)))
        .collect();
    import_corpora(database, scopes, declaration, &corpora, observer)
}

/// [`import_observed`], each source's corpus given with it.
pub(super) fn import_corpora<C: Corpus>(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
    corpora: &[(&Source, C)],
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<Report, Error> {
    declare(database, scopes, declaration)?;
    let mut report = Report::new(&declaration.id);
    for (source, corpus) in corpora {
        let target = Target {
            database,
            scopes,
            collection: &declaration.id,
            source: &source.id,
        };
        import_source(&target, corpus, &mut report, observer)?;
    }
    journal_completion(database, &report).map_err(Error::Journal)?;
    Ok(report)
}

/// Records the collection `declaration` declares and each of its sources,
/// as an import does before its first revision, for a caller who reads
/// `scopes`: unless the kernel records them already just so, and without a
/// write when nothing changed. `maestro knowledge collection add` declares a
/// collection before its first import.
///
/// # Errors
///
/// [`Error::NotVisible`] when `scopes` does not cover the scope of each
/// source, whose records the import reads and writes: nothing is recorded
/// then. [`Error::Records`] when the kernel fails.
pub fn declare(
    database: &Database,
    scopes: &ScopeSet,
    declaration: &Declaration,
) -> Result<(), Error> {
    let hidden = declaration
        .sources
        .iter()
        .map(|source| source_path(&declaration.id, &source.id))
        .find(|scope| !visible(scopes, scope));
    if let Some(scope) = hidden {
        return Err(Error::NotVisible(scope));
    }
    record_declaration(database, scopes, declaration).map_err(Error::Records)
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
    let stream = journal::stream(&report.collection);
    let data = json!(ImportCompleted {
        collection: report.collection.clone(),
        imported: report.imported,
        unchanged: report.unchanged,
        held: report.held,
        refused: report.refused,
    });
    database.record(&NewEvent {
        stream: &stream,
        r#type: ImportCompleted::TYPE,
        // The collection it happened to, which its stream names.
        subject: &stream,
        scope: &collection_path(&report.collection),
        data: &data,
    })?;
    Ok(())
}
