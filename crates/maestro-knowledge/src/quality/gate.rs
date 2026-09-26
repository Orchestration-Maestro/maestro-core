//! The gate over a collection, and the revisions it lets through.

use super::{
    decide::decide,
    error::Error,
    ledger::{Candidate, Ledger},
    outcome,
    report::Report,
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    document::{self, Disposition, Recorded, Revision, RevisionStatus},
    scope::ScopeSet,
    store::Database,
};
use std::collections::HashMap;

/// Gives every revision of the collection `collection` that `scopes` reads,
/// failed ones included and in record order, a disposition, unless it has
/// one, which it keeps: the first rule of `ledger` that matches it, else the
/// automatic checks of its canonical document and its original Markdown.
/// Each disposition that holds a revision back is journaled as
/// `maestro.knowledge.revision.held.v1` in the write that records it.
/// Returns the report of every revision, decided now or before, which counts
/// each first matching rule of `ledger` that a kept disposition outranks.
///
/// # Errors
///
/// Before any work, [`Error::UnknownCollection`] when `scopes` reads no
/// collection `collection`. Part way, [`Error::Records`] or
/// [`Error::Artifacts`] when the kernel fails, and [`Error::Canonical`] when a
/// canonical artifact is not a canonical document: what was decided before
/// stays decided.
pub fn gate(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    ledger: &Ledger,
) -> Result<Report, Error> {
    if database
        .collection(scopes, collection)
        .map_err(Error::Records)?
        .is_none()
    {
        return Err(Error::UnknownCollection(collection.to_owned()));
    }
    let mut report = Report::new(collection);
    for revision in database
        .revisions(scopes, collection)
        .map_err(Error::Records)?
    {
        // A revision has its document's scope, so its document is visible.
        let source_ref = database
            .document(scopes, &revision.document_id)
            .map_err(Error::Records)?
            .map(|document| document.source_ref)
            .unwrap_or_default();
        let kept = database
            .disposition(scopes, &revision.id)
            .map_err(Error::Records)?;
        let candidate = Candidate::new(&revision, &source_ref);
        let (disposition, decided) = match kept {
            Some(kept) => (kept, false),
            None => record(database, scopes, ledger, &candidate)?,
        };
        if !decided {
            let outranked = ledger
                .first_match(&candidate)
                .filter(|rule| rule.disposition != disposition.outcome);
            if let Some(rule) = outranked {
                report.ignore(rule.rule_id());
            }
        }
        report.count(&revision, &source_ref, disposition, decided);
    }
    Ok(report)
}

/// Decides `candidate` with `ledger`, its canonical document and its original
/// Markdown, then records the disposition, and returns it, decided by this
/// run; or, when another writer recorded one in between, that one, kept.
fn record(
    database: &Database,
    scopes: &ScopeSet,
    ledger: &Ledger,
    candidate: &Candidate<'_>,
) -> Result<(Disposition, bool), Error> {
    let revision = candidate.revision();
    let bytes = database
        .get(&revision.canonical_digest)
        .map_err(Error::Artifacts)?;
    let document: CanonicalDocument =
        serde_json::from_slice(&bytes).map_err(|error| Error::Canonical {
            revision_id: revision.id.clone(),
            error,
        })?;
    let original = database
        .get(&revision.original_digest)
        .map_err(Error::Artifacts)?;
    let disposition = decide(
        ledger,
        candidate,
        &document,
        &String::from_utf8_lossy(&original),
    );
    match database
        .record_disposition(&disposition)
        .map_err(Error::Records)?
    {
        Recorded::New => Ok((disposition, true)),
        Recorded::Unchanged => {
            let kept = database
                .disposition(scopes, &revision.id)
                .map_err(Error::Records)?;
            Ok((kept.unwrap_or(disposition), false))
        }
    }
}

/// The revisions of the collection `collection` that `scopes` reads and the
/// gate lets through, in record order: of each document, its latest revision
/// in record order, and only when that revision is not failed and its
/// disposition is `accepted` or `accepted_with_warnings`. An older revision
/// never stands in for a latest one that is held, failed or not decided
/// yet: that document has none.
///
/// Record order is the kernel's: an import that finds a revision recorded
/// already records nothing, so a document rewritten, then given its first
/// bytes and metadata back, keeps the revision in between as its latest.
///
/// # Errors
///
/// [`document::Error::Store`] when the database cannot be read.
pub fn eligible(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<Vec<Revision>, document::Error> {
    let revisions = database.revisions(scopes, collection)?;
    // Collected in record order, so each document keeps its last revision.
    let latest: HashMap<String, String> = revisions
        .iter()
        .map(|revision| (revision.document_id.clone(), revision.id.clone()))
        .collect();
    let mut eligible = Vec::new();
    for revision in revisions {
        if latest.get(&revision.document_id) != Some(&revision.id)
            || revision.status == RevisionStatus::Failed
        {
            continue;
        }
        let disposition = database.disposition(scopes, &revision.id)?;
        if disposition.is_some_and(|disposition| !outcome::holds(disposition.outcome)) {
            eligible.push(revision);
        }
    }
    Ok(eligible)
}
