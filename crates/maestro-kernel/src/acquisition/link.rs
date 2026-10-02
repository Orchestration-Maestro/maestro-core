//! Typed immutable capture/fidelity links, composed with S1's revision transaction.
use super::{
    capture::Captures,
    envelope::CaptureEnvelope,
    privacy::{self, Handle},
    record::{COLUMNS, item_row},
};
use crate::{
    artifact::Digest,
    document::{
        self, Disposition, Document, Recorded, Revision, record_document_on,
        record_with_disposition,
    },
    scope::{Scope, ScopeSet, source_path},
    store::{Database, artifacts::pin},
};
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params};
use serde::Deserialize;
use std::collections::BTreeSet;

/// One exact capture's evidence for a canonical S1 revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionLink {
    /// Existing immutable S1 revision ID.
    pub revision: String,
    /// Verified acknowledged raw-capture envelope evidence.
    pub capture: Handle,
    /// Retained extraction/fidelity evidence.
    pub fidelity: Handle,
    /// Exact compact semantic inventory bytes whose digest enters revision identity.
    pub inventory: Digest,
}
/// Borrowed records committed together by the native mapped write.
#[derive(Debug, Clone, Copy)]
pub struct MappedRevision<'a> {
    /// Immutable canonical revision.
    pub revision: &'a Revision,
    /// Optional first quality disposition and hold event.
    pub disposition: Option<&'a Disposition>,
    /// Document identity to register atomically for a new revision.
    pub document: Option<&'a Document>,
    /// Exact capture, fidelity and inventory relation.
    pub link: &'a RevisionLink,
}
/// Strict versioned inventory preimage: ordered destination/status/digest/length tuples.
#[derive(Deserialize)]
struct Inventory(
    InventoryVersion,
    Vec<(String, AssetState, Option<Digest>, Option<u64>)>,
);
/// Only the admitted inventory domain can add asset pins.
#[derive(Deserialize)]
enum InventoryVersion {
    /// Version one, identical to the metadata key.
    #[serde(rename = "maestro.native_assets/1")]
    V1,
}
/// Existing canonical asset status spelling, without a kernel→canonicalization dependency.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AssetState {
    /// Independently verified bytes.
    Available,
    /// Absent bytes.
    Missing,
    /// Unknown resolution.
    Unchecked,
    /// Unfetched remote destination.
    Remote,
    /// Document fragment.
    Fragment,
    /// Destination outside the admitted root.
    OutsideRoot,
}
impl Database {
    /// Records the optional document, S1 revision, disposition/hold journal
    /// and native evidence in one write.
    /// A replay may add another capture but cannot replace old evidence or add pins twice.
    ///
    /// # Errors
    /// Missing/corrupt raw evidence, scope mismatch, conflicting replay or a store failure;
    /// a failed insert leaves no new document, revision, disposition, journal event or pins.
    pub fn record_mapped_revision(
        &self,
        request: MappedRevision<'_>,
        scope: &Scope,
    ) -> Result<Recorded, document::Error> {
        let MappedRevision {
            revision,
            disposition,
            document,
            link,
        } = request;
        verify_capture(self, scope, link.capture)?;
        self.get(&evidence_digest(&*self.reader()?, link.fidelity)?)?;
        let inventory: Inventory =
            serde_json::from_slice(&self.get(&link.inventory)?).map_err(|_| conflict(link))?;
        let Inventory(_version, records) = inventory;
        let digests: BTreeSet<_> = records
            .into_iter()
            .filter_map(|(_, _, digest, _)| digest)
            .collect();
        self.write(|tx| {
            if let Some(document) = document {
                record_document_on(tx, document)?;
            }
            let recorded = record_with_disposition(tx, revision, disposition)?;
            insert(tx, revision, scope, link, &digests)?;
            Ok(recorded)
        })
    }
    /// Native evidence of a revision, visible only under current grants covering both closures.
    ///
    /// # Errors
    /// Store failures or corrupt recorded identities.
    pub fn revision_links(
        &self,
        scopes: &ScopeSet,
        revision: &str,
    ) -> Result<Vec<RevisionLink>, document::Error> {
        links(&*self.reader()?, scopes, "revision", revision)
    }
    /// Reverse lookup for withdrawal/repair; denied captures return no revision identities.
    ///
    /// # Errors
    /// Store failures or corrupt recorded identities.
    pub fn capture_revisions(
        &self,
        scopes: &ScopeSet,
        capture: Handle,
    ) -> Result<Vec<RevisionLink>, document::Error> {
        links(&*self.reader()?, scopes, "capture", &capture.to_string())
    }
}
/// Reuse N12's acknowledged-item verifier and capture artifact readback, never a parallel reader.
fn verify_capture(db: &Database, scope: &Scope, capture: Handle) -> Result<(), document::Error> {
    let invalid = || document::Error::RevisionConflict("invalid raw capture".into());
    let envelope: CaptureEnvelope = serde_json::from_slice(
        &privacy::snapshot(db, &capture.to_string()).map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let item = db.reader()?.query_row(
        &format!("SELECT {COLUMNS} FROM acquisition_frontier WHERE id = ?1"),
        [envelope.item.to_string()],
        item_row,
    )?;
    db.verify_capture(scope, &item, capture)
        .map_err(|_| invalid())?;
    db.capture_bytes(&envelope).map_err(|_| invalid())?;
    Ok(())
}
/// Revision conflict refuses invalid or substituted evidence without exposing private payloads.
fn conflict(link: &RevisionLink) -> document::Error {
    document::Error::RevisionConflict(link.revision.clone())
}
/// Checks the exact source scope and inserts once, pinning each distinct asset only for a new link.
fn insert(
    tx: &Transaction<'_>,
    revision: &Revision,
    scope: &Scope,
    link: &RevisionLink,
    digests: &BTreeSet<Digest>,
) -> Result<(), document::Error> {
    if link.revision != revision.id {
        return Err(conflict(link));
    }
    let (collection, source): (String, String) = tx.query_row(
        "SELECT collection_id, source_id FROM documents WHERE id = ?1",
        [&revision.document_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if source_path(&collection, &source) != scope.as_str() {
        return Err(conflict(link));
    }
    for evidence in [link.capture, link.fidelity] {
        let found: String = tx.query_row(
            "SELECT scope FROM acquisition_evidence WHERE id = ?1",
            [evidence.to_string()],
            |row| row.get(0),
        )?;
        if found != scope.as_str() {
            return Err(conflict(link));
        }
    }
    let previous = tx
        .query_row(
            "SELECT revision, capture, fidelity, inventory FROM acquisition_revision_links
         WHERE revision = ?1 AND capture = ?2",
            params![link.revision, link.capture.to_string()],
            link_row,
        )
        .optional()?;
    if let Some(previous) = previous {
        return if previous == *link {
            Ok(())
        } else {
            Err(conflict(link))
        };
    }
    tx.execute(
        "INSERT INTO acquisition_revision_links (revision, scope, capture, fidelity, inventory)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            link.revision,
            scope.as_str(),
            link.capture.to_string(),
            link.fidelity.to_string(),
            link.inventory.as_str()
        ],
    )?;
    pin(tx, &link.inventory)?;
    for digest in digests {
        pin(tx, digest)?;
    }
    Ok(())
}
/// Current root scope and transitive capture/fidelity grants govern both lookup directions.
fn links(
    reader: &Connection,
    scopes: &ScopeSet,
    column: &str,
    id: &str,
) -> Result<Vec<RevisionLink>, document::Error> {
    let mut statement = reader.prepare(&format!(
        "SELECT revision, capture, fidelity, inventory FROM acquisition_revision_links
         WHERE {column} = ?1 ORDER BY revision, capture",
    ))?;
    // Insert enforces root scope equality; handle authorization checks both closures.
    let mut visible = Vec::new();
    for row in statement.query_map([id], link_row)? {
        let link = row?;
        let mut covered = true;
        for handle in [link.capture, link.fidelity] {
            if privacy::authorized(reader, scopes, handle)
                .map_err(|_| conflict(&link))?
                .is_none()
            {
                covered = false;
            }
        }
        if covered {
            visible.push(link);
        }
    }
    Ok(visible)
}
/// Immutable typed row decoder, shared by replay checks and both scoped lookups.
fn link_row(row: &Row<'_>) -> rusqlite::Result<RevisionLink> {
    /// Storage identity corruption is an invalid query, never a silently repaired handle.
    fn handle(row: &Row<'_>, index: usize) -> rusqlite::Result<Handle> {
        row.get::<_, String>(index)?
            .parse()
            .map_err(|_| rusqlite::Error::InvalidQuery)
    }
    Ok(RevisionLink {
        revision: row.get(0)?,
        capture: handle(row, 1)?,
        fidelity: handle(row, 2)?,
        inventory: Digest::parse(&row.get::<_, String>(3)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
    })
}
/// Artifact digest of an existing protected handle, rehashed by the existing store read.
fn evidence_digest(reader: &Connection, handle: Handle) -> Result<Digest, document::Error> {
    let text: String = reader.query_row(
        "SELECT artifact FROM acquisition_evidence WHERE id = ?1",
        [handle.to_string()],
        |row| row.get(0),
    )?;
    Digest::parse(&text)
        .map_err(|_| document::Error::RevisionConflict("invalid evidence digest".into()))
}
