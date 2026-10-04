//! One bounded capture-link query, with the same immutable artifact verification.
use super::{
    capture_integrity::validate_parent,
    envelope::CaptureEnvelope,
    privacy::{Handle, ReceiptError},
    record::Item,
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use rusqlite::params;
use std::collections::BTreeMap;
use ulid::Ulid;

/// Verified current-generation linkage; acknowledged status is tied to the input row.
#[derive(Debug, Clone)]
pub struct CaptureLookup {
    /// Scoped immutable capture handle.
    pub handle: Handle,
    /// Rehashed immutable envelope and body provenance.
    pub envelope: CaptureEnvelope,
    /// The input frontier capture digest matches this envelope artifact.
    pub acknowledged: bool,
}
/// Bounded verified capture links keyed by existing frontier item identity.
pub type CapturePage = BTreeMap<Ulid, CaptureLookup>;

/// Batch lookup shared by acquisition traversal and partition coverage.
pub(super) fn lookup(
    db: &Database,
    scope: &Scope,
    items: &[Item],
) -> Result<BTreeMap<Ulid, CaptureLookup>, ReceiptError> {
    if items.len() > 1000 {
        return Err(ReceiptError::Invalid);
    }
    let ids: Vec<_> = items.iter().map(|item| item.id.to_string()).collect();
    let reader = db.reader()?;
    let mut query = reader.prepare(
        "SELECT l.item, l.envelope, e.artifact, b.artifact, f.capture
         FROM acquisition_capture_links l
         JOIN acquisition_frontier f ON f.id = l.item
         AND l.generation = f.capture_generation
         JOIN acquisition_evidence e ON e.id = l.envelope
         JOIN acquisition_evidence b ON b.id = l.body
         WHERE l.item IN (SELECT value FROM json_each(?1)) AND e.scope = ?2 AND b.scope = ?2",
    )?;
    let rows = query
        .query_map(
            params![serde_json::to_string(&ids)?, scope.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let indexed: BTreeMap<_, _> = items
        .iter()
        .map(|item| (item.id.to_string(), item))
        .collect();
    let mut found = BTreeMap::new();
    for (id, handle, artifact, body, acknowledged) in rows {
        let item = indexed.get(&id).ok_or(ReceiptError::Invalid)?;
        let verified = verify(
            db,
            scope,
            item,
            (&handle, &artifact, &body, acknowledged.as_deref()),
        );
        match verified {
            Ok(capture) => {
                found.insert(item.id, capture);
            }
            // Single-item capture_for also treats damaged acknowledged evidence as absent.
            Err(_) if item.capture.is_some() => {}
            Err(error) => return Err(error),
        }
    }
    Ok(found)
}

/// Rehash both immutable artifacts; no nested SQL lookup for each page row.
fn verify(
    db: &Database,
    scope: &Scope,
    item: &Item,
    row: (&str, &str, &str, Option<&str>),
) -> Result<CaptureLookup, ReceiptError> {
    let (handle, artifact, body, acknowledged) = row;
    let artifact = Digest::parse(artifact).map_err(|_| ReceiptError::Storage)?;
    let envelope: CaptureEnvelope = serde_json::from_slice(&db.get(&artifact)?)?;
    let body = Digest::parse(body).map_err(|_| ReceiptError::Storage)?;
    db.get_bounded(&body, envelope.length)?;
    validate_parent(db, &envelope, scope)?;
    Ok(CaptureLookup {
        handle: handle.parse()?,
        acknowledged: acknowledged == Some(artifact.as_str())
            && item.capture.as_ref() == Some(&artifact)
            && envelope.item.to_string() == item.id.to_string(),
        envelope,
    })
}
