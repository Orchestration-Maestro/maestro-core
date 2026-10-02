//! Verified immutable capture preparation before fenced stage acknowledgment.
use super::{
    capture_integrity::{validate_parent, verify},
    capture_page::{self, CaptureLookup},
    envelope::{CaptureEnvelope, Representation, SafeIdentity, Transport},
    frontier::Frontier,
    headers,
    lease::{self, ItemLease, SourceLease},
    privacy::{self, Handle, ReceiptError},
    record::Item,
};
use crate::{
    artifact::{self, Digest},
    scope::Scope,
    store::{self, Database},
};
use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use std::{collections::BTreeMap, time::SystemTime};
use ulid::Ulid;

/// Current trusted source/dispatch ownership, never a staging path from a source.
#[derive(Debug, Clone)]
pub struct CaptureContext {
    /// Fenced source writer.
    pub writer: SourceLease,
    /// Fenced frontier staging identity.
    pub item: ItemLease,
    /// Current trusted authority time.
    pub now: SystemTime,
}
/// A durable handle and actual storage newly retained by this preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedCapture {
    /// Scoped immutable envelope linkage.
    pub handle: Handle,
    /// New body and envelope bytes; zero for digest-identical reuse.
    pub retained_bytes: u64,
}
/// Replaceable immutable capture boundary; preparation is a durable checkpoint.
pub trait Captures: Send + Sync {
    /// Resolve at most 1,000 current-generation capture links in one read page.
    /// # Errors
    /// Invalid bounds or corrupt immutable artifacts refuse.
    fn capture_page(
        &self,
        scope: &Scope,
        items: &[Item],
    ) -> Result<BTreeMap<Ulid, CaptureLookup>, ReceiptError>;
    /// Locate immutable prepared evidence for the item's current refresh generation.
    /// # Errors
    /// Invalid scoped evidence or unreadable storage refuses.
    fn prepared_for(&self, scope: &Scope, item: &Item) -> Result<Option<Handle>, ReceiptError>;
    /// Locate a scoped acknowledged envelope, verifying its immutable linkage.
    /// Unknown, differently scoped or corrupt capture evidence returns none.
    /// # Errors
    /// A content-free storage failure prevents inspection.
    fn capture_for(&self, scope: &Scope, item: &Item) -> Result<Option<Handle>, ReceiptError>;
    /// Read a prepared capture through its existing verified linkage under the current lease.
    /// A finite byte ceiling is checked before body allocation; admitted artifacts are rehashed.
    /// Over-budget content returns the verified envelope with absent bytes.
    /// # Errors
    /// Substituted handles, lost ownership, corrupt content or exceeded bounds refuse.
    fn read_capture(
        &self,
        context: &CaptureContext,
        capture: Handle,
        max_bytes: u64,
    ) -> Result<(CaptureEnvelope, Option<Vec<u8>>), ReceiptError>;
    /// Verify bytes, lease and provenance, and retain one scoped immutable link.
    /// # Errors
    /// Invalid envelope, stale ownership, conflicting replay or storage failure.
    fn prepare_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
        max_new_bytes: u64,
    ) -> Result<PreparedCapture, ReceiptError>;
    /// Check the same ownership/reuse rules as preparation without writing.
    /// Returns verified new body/envelope bytes needing admission.
    /// # Errors
    /// Invalid content, fenced ownership, conflicting reuse or unreadable storage.
    fn check_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
    ) -> Result<u64, ReceiptError>;
    /// Read back the unique body/envelope bytes actually retained, including unlinked writes.
    /// # Errors
    /// Corrupt or unreadable artifacts refuse; missing bytes count as zero.
    fn capture_bytes(&self, envelope: &CaptureEnvelope) -> Result<u64, ReceiptError>;
    /// Bind intact scoped capture evidence to this item's acknowledged envelope digest.
    /// # Errors
    /// Unknown, substituted, differently scoped or corrupt evidence refuses.
    fn verify_capture(
        &self,
        scope: &Scope,
        item: &Item,
        evidence: Handle,
    ) -> Result<(), ReceiptError>;
    /// Verify persisted artifacts before acknowledging this exact item once.
    /// # Errors
    /// Unknown/substituted handle, corrupt artifacts or fenced ownership refuses.
    fn acknowledge_capture(
        &self,
        context: &CaptureContext,
        capture: Handle,
    ) -> Result<(), ReceiptError>;
}
impl Captures for Database {
    fn capture_page(
        &self,
        scope: &Scope,
        items: &[Item],
    ) -> Result<BTreeMap<Ulid, CaptureLookup>, ReceiptError> {
        capture_page::lookup(self, scope, items)
    }
    fn prepared_for(&self, scope: &Scope, item: &Item) -> Result<Option<Handle>, ReceiptError> {
        let reader = self.reader()?;
        let handle: Option<String> = reader
            .query_row(
                "SELECT l.envelope FROM acquisition_capture_links l
             JOIN acquisition_frontier f ON f.id = l.item
             JOIN acquisition_evidence e ON e.id = l.envelope
             WHERE l.item = ?1 AND l.generation = f.capture_generation AND e.scope = ?2",
                params![item.id.to_string(), scope.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let handle = handle.map(|text| text.parse()).transpose()?;
        if let Some(handle) = handle {
            verify(self, &reader, handle)?;
        }
        Ok(handle)
    }
    fn capture_for(&self, scope: &Scope, item: &Item) -> Result<Option<Handle>, ReceiptError> {
        capture_for_on(self, &*self.reader()?, scope, item)
    }
    fn read_capture(
        &self,
        context: &CaptureContext,
        capture: Handle,
        max_bytes: u64,
    ) -> Result<(CaptureEnvelope, Option<Vec<u8>>), ReceiptError> {
        let envelope: CaptureEnvelope =
            serde_json::from_slice(&privacy::snapshot(self, &capture.to_string())?)?;
        if max_bytes == 0
            || envelope.item.to_string() != context.item.item.to_string()
            || envelope.source != context.writer.source
        {
            return Err(ReceiptError::Invalid);
        }
        let (scope, stored) =
            self.write(|tx| existing(tx, context, &envelope, &envelope.identity()?))?;
        if stored != Some(capture) {
            return Err(ReceiptError::Invalid);
        }
        validate_parent(self, &envelope, &scope)?;
        if envelope.length > max_bytes {
            return Ok((envelope, None));
        }
        // Preparation bound this immutable envelope to verified length/digest;
        // the artifact store rehashes the returned body on every read.
        let bytes = self
            .get_bounded(&envelope.artifact, max_bytes)
            .map_err(|error| {
                if let store::Error::Artifact(artifact::Error::TooLarge) = error {
                    ReceiptError::Invalid
                } else {
                    ReceiptError::Storage
                }
            })?;
        Ok((envelope, Some(bytes)))
    }
    fn prepare_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
        max_new_bytes: u64,
    ) -> Result<PreparedCapture, ReceiptError> {
        let new_bytes = self.check_capture(context, envelope, bytes)?;
        if new_bytes > max_new_bytes {
            return Err(ReceiptError::Conflict);
        }
        let identity = envelope.identity()?;
        let encoded = serde_json::to_vec(envelope)?;
        let (_, previous) = self.write(|tx| existing(tx, context, envelope, &identity))?;
        if let Some(handle) = previous {
            verify(self, &*self.reader()?, handle)?;
            return Ok(PreparedCapture {
                handle,
                retained_bytes: 0,
            });
        }
        let body = persist(self, bytes)?;
        let artifact = persist(self, &encoded)?;
        self.write(|tx| {
            let (scope, previous) = existing(tx, context, envelope, &identity)?;
            if let Some(handle) = previous {
                return Ok(PreparedCapture {
                    handle,
                    retained_bytes: 0,
                });
            }
            let body_handle = privacy::retain_on(tx, &scope, &body, &[])?;
            let mut references = vec![
                body_handle,
                envelope.inputs,
                envelope.access,
                envelope.decision,
            ];
            references.extend(envelope.parent);
            let handle = privacy::retain_on(tx, &scope, &artifact, &references)?;
            tx.execute(
                "INSERT INTO acquisition_capture_links (item, generation, identity, envelope, body)
                 SELECT ?1, capture_generation, ?2, ?3, ?4 FROM acquisition_frontier
                 WHERE id = ?1",
                params![
                    envelope.item.to_string(),
                    identity.as_str(),
                    handle.to_string(),
                    body_handle.to_string()
                ],
            )?;
            Ok(PreparedCapture {
                handle,
                retained_bytes: new_bytes,
            })
        })
    }
    fn check_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
    ) -> Result<u64, ReceiptError> {
        validate(context, envelope, bytes)?;
        let encoded = serde_json::to_vec(envelope)?;
        let (scope, previous) =
            self.write(|tx| existing(tx, context, envelope, &envelope.identity()?))?;
        privacy::validate(&scope, &encoded, &[])?;
        validate_parent(self, envelope, &scope)?;
        if let Some(handle) = previous {
            verify(self, &*self.reader()?, handle)?;
            return Ok(0);
        }
        let total = payloads(envelope)?.values().sum::<u64>();
        Ok(total.saturating_sub(self.capture_bytes(envelope)?))
    }
    fn capture_bytes(&self, envelope: &CaptureEnvelope) -> Result<u64, ReceiptError> {
        let mut retained = 0;
        for (digest, length) in payloads(envelope)? {
            match self.get_bounded(&digest, length) {
                Ok(bytes) => retained += bytes.len() as u64,
                Err(store::Error::Artifact(artifact::Error::Missing(_))) => {}
                Err(store::Error::Artifact(artifact::Error::TooLarge)) => {
                    return Err(ReceiptError::Invalid);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(retained)
    }
    fn verify_capture(
        &self,
        scope: &Scope,
        item: &Item,
        evidence: Handle,
    ) -> Result<(), ReceiptError> {
        verify_capture_on(self, &*self.reader()?, scope, item, evidence)
    }
    fn acknowledge_capture(
        &self,
        context: &CaptureContext,
        capture: Handle,
    ) -> Result<(), ReceiptError> {
        let envelope = verify(self, &*self.reader()?, capture)?;
        let (_, prepared) =
            self.write(|tx| existing(tx, context, &envelope, &envelope.identity()?))?;
        if prepared != Some(capture) {
            return Err(ReceiptError::Invalid);
        }
        if envelope.item.to_string() != context.item.item.to_string() {
            return Err(ReceiptError::Invalid);
        }
        let scope: String = self.reader()?.query_row(
            "SELECT scope FROM acquisition_evidence WHERE id = ?1",
            [capture.to_string()],
            |row| row.get(0),
        )?;
        validate_parent(
            self,
            &envelope,
            &scope.parse().map_err(|_| ReceiptError::Storage)?,
        )?;
        let artifact = Digest::of(&privacy::snapshot(self, &capture.to_string())?);
        self.acknowledge(&context.writer, &context.item, &artifact, context.now)
            .map_err(|_| ReceiptError::Conflict)
    }
}
/// Scoped lookup and every artifact/link check share this read unit.
fn capture_for_on(
    db: &Database,
    reader: &Connection,
    scope: &Scope,
    item: &Item,
) -> Result<Option<Handle>, ReceiptError> {
    let capture: Option<String> = reader
        .query_row(
            "SELECT l.envelope FROM acquisition_capture_links l
             JOIN acquisition_frontier f ON f.id = l.item
             WHERE l.item = ?1 AND l.generation = f.capture_generation",
            [item.id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(handle) = capture.and_then(|text| text.parse().ok()) else {
        return Ok(None);
    };
    if verify_capture_on(db, reader, scope, item, handle).is_err() {
        return Ok(None);
    }
    Ok(Some(handle))
}
/// Verify immutable evidence and its exact acknowledged frontier linkage together.
fn verify_capture_on(
    db: &Database,
    reader: &Connection,
    scope: &Scope,
    item: &Item,
    evidence: Handle,
) -> Result<(), ReceiptError> {
    let envelope = verify(db, reader, evidence)?;
    let row: Option<(String, String, String, Option<String>)> = reader
        .query_row(
            "SELECT e.scope, e.artifact, l.item, f.capture FROM acquisition_capture_links l
             JOIN acquisition_evidence e ON e.id = l.envelope
             JOIN acquisition_frontier f ON f.id = l.item WHERE l.envelope = ?1
             AND l.generation = f.capture_generation",
            [evidence.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let (stored_scope, artifact, linked_item, acknowledged) = row.ok_or(ReceiptError::Invalid)?;
    if stored_scope != scope.as_str()
        || linked_item != item.id.to_string()
        || envelope.item.to_string() != linked_item
        || acknowledged.as_deref() != Some(artifact.as_str())
        || item.capture.as_ref().map(Digest::as_str) != Some(artifact.as_str())
    {
        return Err(ReceiptError::Invalid);
    }
    validate_parent(db, &envelope, scope)
}
/// Validate content and exact representation before admitting any durable bytes.
fn validate(
    context: &CaptureContext,
    envelope: &CaptureEnvelope,
    bytes: &[u8],
) -> Result<(), ReceiptError> {
    if envelope.artifact != Digest::of(bytes) {
        return Err(ReceiptError::Invalid);
    }
    if envelope.length != bytes.len() as u64 {
        return Err(ReceiptError::Invalid);
    }
    if envelope.schema != "maestro-capture/1"
        || envelope.source != context.writer.source
        || envelope.item.to_string() != context.item.item.to_string()
        || !(100..=599).contains(&envelope.status)
        || envelope.redirects.len() > 1000
    {
        return Err(ReceiptError::Invalid);
    }
    let valid_label = match envelope.representation {
        Representation::WireBody => {
            envelope.transport != Transport::BrowserRender && envelope.parent.is_none()
        }
        Representation::RenderedDom => {
            envelope.transport == Transport::BrowserRender && envelope.parent.is_none()
        }
        Representation::SelectedHtml | Representation::ApiRecord => envelope.parent.is_some(),
    };
    if !valid_label {
        return Err(ReceiptError::Invalid);
    }
    for media in [&envelope.declared_media, &envelope.detected_media]
        .into_iter()
        .flatten()
    {
        if headers::safe_media(media).as_ref() != Some(media) {
            return Err(ReceiptError::Invalid);
        }
    }
    for hop in &envelope.redirects {
        if ![301, 302, 303, 307, 308].contains(&hop.status) {
            return Err(ReceiptError::Invalid);
        }
    }
    headers::validate(&envelope.headers)
}
/// Validate the durable staging handle and original request contexts, then reuse.
fn existing(
    tx: &Transaction<'_>,
    context: &CaptureContext,
    envelope: &CaptureEnvelope,
    identity: &Digest,
) -> Result<(Scope, Option<Handle>), ReceiptError> {
    let (scope, _) = lease::dispatched(tx, &context.writer, &context.item, context.now)
        .map_err(|_| ReceiptError::Conflict)?;
    let scope: Scope = scope.parse().map_err(|_| ReceiptError::Invalid)?;
    let (request, authorization, profile): (String, String, String) = tx.query_row(
        "SELECT fetch_identity, authorization_context, representation_profile
         FROM acquisition_frontier WHERE id = ?1",
        [envelope.item.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if SafeIdentity::new(&request)? != envelope.requested
        || authorization != envelope.authorization_context.as_str()
        || profile != envelope.profile.as_str()
    {
        return Err(ReceiptError::Invalid);
    }
    let found: Option<(String, String)> = tx
        .query_row(
            "SELECT l.identity, l.envelope FROM acquisition_capture_links l
             JOIN acquisition_frontier f ON f.id = l.item
             WHERE l.item = ?1 AND l.generation = f.capture_generation",
            [envelope.item.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match found {
        Some((previous, handle)) if previous == identity.as_str() => {
            Ok((scope, Some(handle.parse()?)))
        }
        Some(_) => Err(ReceiptError::Conflict),
        None => Ok((scope, None)),
    }
}
/// Unique immutable payloads, so identical body/envelope digests are charged once.
fn payloads(envelope: &CaptureEnvelope) -> Result<BTreeMap<Digest, u64>, ReceiptError> {
    let encoded = serde_json::to_vec(envelope)?;
    Ok(BTreeMap::from([
        (envelope.artifact.clone(), envelope.length),
        (Digest::of(&encoded), encoded.len() as u64),
    ]))
}
/// Existing verified bytes keep their first recorded media; defaults apply only to new bytes.
fn persist(db: &Database, bytes: &[u8]) -> Result<Digest, ReceiptError> {
    let digest = Digest::of(bytes);
    if db.artifact(&digest)?.is_some() {
        db.get(&digest)?;
        return Ok(digest);
    }
    Ok(db.put(bytes, "application/octet-stream")?)
}
