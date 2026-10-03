//! Content-free output types and opaque, transitively scoped artifact handles.
use crate::{
    artifact::Digest,
    scope::{Scope, ScopeSet},
    store::{self, Database, artifacts::pin},
};
use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
#[cfg(test)]
use std::cell::Cell;
use std::{error, fmt, str::FromStr};
use ulid::Ulid;

/// An opaque identity, never an artifact digest or a content-derived URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle(Ulid);
impl Handle {
    /// Allocates a unique receipt, attempt, item or protected artifact identity.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::generate())
    }
}
impl From<Ulid> for Handle {
    fn from(id: Ulid) -> Self {
        Self(id)
    }
}
impl FromStr for Handle {
    type Err = ReceiptError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(text)
            .map(Self)
            .map_err(|_| ReceiptError::Invalid)
    }
}
impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
impl Serialize for Handle {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Handle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse()
            .map_err(|_| de::Error::custom("invalid receipt handle"))
    }
}
/// Status codes accepted at every acquisition progress sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Work has not reached a terminal outcome.
    Pending,
    /// All selected work has a completed disposition.
    Complete,
    /// Some selected work could not complete.
    Partial,
    /// Authority, readiness or resources prevented completion.
    Blocked,
    /// The attempted work failed.
    Failed,
    /// Owned work was cancelled.
    Cancelled,
}
/// Typed content-free dispatch reasons; raw errors belong in protected artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// No refusal or failure.
    None,
    /// Policy refused dispatch.
    Denied,
    /// The read-only authority decision found an expired grant.
    Expired,
    /// Current authority was revoked.
    Revoked,
    /// Human authentication is required.
    Authentication,
    /// A resource bound prevented dispatch.
    Budget,
    /// Transport failed without exposing its error text.
    Transport,
    /// A required capability is unavailable.
    Unsupported,
}
/// The only acquisition payload emitted to journal, log or notifier sinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    /// Access-checked attempt handle, not a digest capability.
    pub receipt: Handle,
    /// Fixed status, with pending and partial distinct from success.
    pub status: Status,
    /// Fixed reason, including refused dispatches.
    pub reason: Reason,
}
impl Progress {
    /// Refuses content-bearing attributes, arbitrary states and malformed handles.
    ///
    /// # Errors
    /// Returns a content-free invalid-input error; refused bytes are never echoed.
    pub fn parse(bytes: &[u8]) -> Result<Self, ReceiptError> {
        serde_json::from_slice(bytes).map_err(|_| ReceiptError::Invalid)
    }
}
/// Content-free errors at the receipt/privacy boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptError {
    /// A schema, scope, reference or bound is invalid.
    Invalid,
    /// An attempt is unknown, already terminal or conflicts with frozen inputs.
    Conflict,
    /// Storage failed; raw storage errors are not public event data.
    Storage,
}
impl fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid acquisition receipt",
            Self::Conflict => "acquisition receipt conflicts with its attempt",
            Self::Storage => "acquisition receipt storage failed",
        })
    }
}
impl error::Error for ReceiptError {}
impl From<store::Error> for ReceiptError {
    fn from(_: store::Error) -> Self {
        Self::Storage
    }
}
impl From<rusqlite::Error> for ReceiptError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl From<serde_json::Error> for ReceiptError {
    fn from(_: serde_json::Error) -> Self {
        Self::Invalid
    }
}
/// Bytes returned only after current grants cover the entire reference closure.
/// Its debug output deliberately excludes content and digest capabilities.
#[derive(Clone, PartialEq, Eq)]
pub struct ProtectedArtifact {
    /// Authorized bytes, private even when the artifact's scope is public.
    bytes: Vec<u8>,
}
impl ProtectedArtifact {
    /// The protected content for the authorized local view, never a notifier.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl fmt::Debug for ProtectedArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedArtifact")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
/// Maximum direct reference edges of a single protected snapshot.
pub(super) const MAX_REFERENCES: usize = 1000;

/// Checks the bounded protected payload and its collection-derived scope.
pub(super) fn validate(
    scope: &Scope,
    bytes: &[u8],
    references: &[Handle],
) -> Result<(), ReceiptError> {
    if scope.as_str().split('/').nth(2) != Some("collection")
        || bytes.len() > 4 * 1024 * 1024
        || references.len() > MAX_REFERENCES
    {
        return Err(ReceiptError::Invalid);
    }
    Ok(())
}
/// Pins an artifact and all opaque reference edges atomically with its owner row.
pub(super) fn retain_on(
    transaction: &Transaction<'_>,
    scope: &Scope,
    digest: &Digest,
    references: &[Handle],
) -> Result<Handle, ReceiptError> {
    let handle = Handle::new();
    pin(transaction, digest)?;
    transaction.execute(
        "INSERT INTO acquisition_evidence (id, scope, artifact) VALUES (?1, ?2, ?3)",
        params![handle.to_string(), scope.as_str(), digest.as_str()],
    )?;
    for child in references {
        transaction.execute(
            "INSERT OR IGNORE INTO acquisition_evidence_links (parent, child) VALUES (?1, ?2)",
            params![handle.to_string(), child.to_string()],
        )?;
    }
    Ok(handle)
}
/// Resolves a handle only when every transitive reference is currently visible.
pub(super) fn authorized(
    reader: &Connection,
    scopes: &ScopeSet,
    handle: Handle,
) -> Result<Option<Digest>, ReceiptError> {
    let query = format!(
        "WITH RECURSIVE closure(id) AS (
        SELECT id FROM acquisition_evidence WHERE id = ?1
        UNION SELECT child FROM acquisition_evidence_links JOIN closure ON parent = closure.id)
        SELECT artifact FROM acquisition_evidence AS root WHERE root.id = ?1
        AND NOT EXISTS (SELECT 1 FROM closure
            JOIN acquisition_evidence AS linked ON linked.id = closure.id
            WHERE NOT {})",
        ScopeSet::condition("linked.scope", 2)
    );
    let found: Option<String> = reader
        .query_row(
            &query,
            params![handle.to_string(), scopes.parameter()],
            |row| row.get(0),
        )
        .optional()?;
    found
        .map(|text| Digest::parse(&text).map_err(|_| ReceiptError::Storage))
        .transpose()
}
/// Verifies that typed inventory references have real stored scope edges.
pub(super) fn linked(
    reader: &Connection,
    parent: Handle,
    children: &[Handle],
) -> Result<bool, ReceiptError> {
    for child in children {
        let found: bool = reader.query_row(
            "SELECT EXISTS (SELECT 1 FROM acquisition_evidence_links
                WHERE parent = ?1 AND child = ?2)",
            params![parent.to_string(), child.to_string()],
            |row| row.get(0),
        )?;
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Loads authorized protected bytes without revealing digests on denial.
pub(super) fn read(
    db: &Database,
    principal: &str,
    handle: Handle,
) -> Result<Option<ProtectedArtifact>, ReceiptError> {
    let reader = db.reader()?;
    let scopes = Database::visible_on(&reader, principal)?;
    let Some(digest) = authorized(&reader, &scopes, handle)? else {
        return Ok(None);
    };
    Ok(Some(ProtectedArtifact {
        bytes: db.get(&digest)?,
    }))
}
/// Loads an internal snapshot. This is kernel bookkeeping, never an external view.
pub(super) fn snapshot(db: &Database, handle: &str) -> Result<Vec<u8>, ReceiptError> {
    snapshot_on(db, &*db.reader()?, handle)
}
#[cfg(test)]
thread_local! {
    /// Snapshot loads on this test thread, independent of concurrent tests.
    pub(super) static SNAPSHOT_READS: Cell<u32> = const { Cell::new(0) };
}

/// Read evidence through the caller's existing bookkeeping connection.
pub(super) fn snapshot_on(
    db: &Database,
    reader: &Connection,
    handle: &str,
) -> Result<Vec<u8>, ReceiptError> {
    #[cfg(test)]
    SNAPSHOT_READS.with(|count| count.set(count.get() + 1));
    let digest: String = reader.query_row(
        "SELECT artifact FROM acquisition_evidence WHERE id = ?1",
        [handle],
        |row| row.get(0),
    )?;
    Ok(db.get(&Digest::parse(&digest).map_err(|_| ReceiptError::Storage)?)?)
}

#[cfg(test)]
mod tests {
    use super::Handle;
    use ulid::Ulid;

    #[test]
    fn n42_handle_from_ulid_preserves_identity() {
        let id = Ulid::generate();
        let handle = Handle::from(id);
        assert_eq!(handle.to_string(), id.to_string());
        assert_eq!(handle.to_string().parse::<Ulid>().unwrap(), id);
        assert_eq!(handle.to_string().parse::<Handle>().unwrap(), handle);
    }
}
