//! Shared immutable capture and prerequisite checks, including batched resume reads.
use super::{
    envelope::{CaptureEnvelope, Representation},
    privacy::{self, Handle, ReceiptError},
};
use crate::{
    artifact::{self, Digest},
    scope::Scope,
    store::{self, Database},
};
use rusqlite::{Connection, OptionalExtension as _};
use std::collections::BTreeSet;

/// Verify both artifact bodies and their immutable scoped linkage on every replay.
pub(super) fn verify(
    db: &Database,
    reader: &Connection,
    capture: Handle,
) -> Result<CaptureEnvelope, ReceiptError> {
    let body: String = reader.query_row(
        "SELECT e.artifact FROM acquisition_capture_links l
         JOIN acquisition_evidence e ON e.id = l.body WHERE l.envelope = ?1",
        [capture.to_string()],
        |row| row.get(0),
    )?;
    let envelope: CaptureEnvelope =
        serde_json::from_slice(&privacy::snapshot_on(db, reader, &capture.to_string())?)?;
    // Preparation checked digest and length before retaining these immutable
    // edges. The artifact store rehashes both payloads here; repeating those
    // field comparisons cannot detect any additional substitution.
    let digest = Digest::parse(&body).map_err(|_| ReceiptError::Storage)?;
    db.get_bounded(&digest, envelope.length)
        .map_err(|error| match error {
            store::Error::Artifact(artifact::Error::TooLarge) => ReceiptError::Invalid,
            _ => ReceiptError::Storage,
        })?;
    Ok(envelope)
}

/// Exact derivation edges are data; an unlisted pair is never inferred.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Derivation {
    /// Exact derived representation.
    child: Representation,
    /// Closed admitted parent kinds.
    parents: Vec<Representation>,
}
/// A derived payload cannot consume a pending, differently scoped or corrupt parent.
pub(super) fn validate_parent(
    db: &Database,
    child: &CaptureEnvelope,
    scope: &Scope,
) -> Result<(), ReceiptError> {
    let mut child = child.clone();
    let mut seen = BTreeSet::new();
    while let Some(parent) = child.parent {
        if !seen.insert(parent) {
            return Err(ReceiptError::Invalid);
        }
        child = verified_parent(db, &child, scope, parent)?;
    }
    Ok(())
}
/// Verify one immutable dependency edge without recursive stack growth.
fn verified_parent(
    db: &Database,
    child: &CaptureEnvelope,
    scope: &Scope,
    parent: Handle,
) -> Result<CaptureEnvelope, ReceiptError> {
    let row: Option<(String, String, Option<String>, String)> = db
        .reader()?
        .query_row(
            "SELECT e.scope, f.source, f.capture, e.artifact
         FROM acquisition_capture_links l
         JOIN acquisition_evidence e ON e.id = l.envelope
         JOIN acquisition_frontier f ON f.id = l.item
         WHERE l.envelope = ?1 AND l.generation = f.capture_generation",
            [parent.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let (parent_scope, source, accepted, artifact) = row.ok_or(ReceiptError::Invalid)?;
    if parent_scope != scope.as_str() {
        return Err(ReceiptError::Invalid);
    }
    if source != child.source {
        return Err(ReceiptError::Invalid);
    }
    if accepted.as_deref() != Some(artifact.as_str()) {
        return Err(ReceiptError::Invalid);
    }
    let envelope = verify(db, &*db.reader()?, parent)?;
    let rules: Vec<Derivation> = serde_json::from_str(include_str!("derivations.json"))?;
    if !rules.iter().any(|rule| {
        rule.child == child.representation && rule.parents.contains(&envelope.representation)
    }) {
        return Err(ReceiptError::Invalid);
    }
    Ok(envelope)
}
