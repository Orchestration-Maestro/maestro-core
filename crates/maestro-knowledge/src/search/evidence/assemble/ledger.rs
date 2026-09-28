//! The duplicate facts a complete chunk set's manifest permits T032 to use.

use super::super::types::EvidenceError;
use crate::prepare::{
    Error as PrepareError,
    manifest::{Manifest, SCHEMA},
};
use maestro_kernel::{
    chunk_set::{self, ChunkSetState},
    retrieval::ReadControl,
    scope::ScopeSet,
    store::{self, Database},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error, fmt,
    sync::atomic::Ordering,
    time::Instant,
};

/// Manifest-backed duplicate facts for one pinned chunk set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::search::evidence) struct DuplicateLedger {
    /// Revisions eligible when the set was prepared.
    pub(in crate::search::evidence) revisions: BTreeSet<String>,
    /// Exact duplicate revision to prepared representative.
    pub(in crate::search::evidence) duplicates: BTreeMap<String, String>,
    /// Near-duplicate groups allowed by the set manifest.
    pub(in crate::search::evidence) near_duplicate_groups: BTreeSet<String>,
}

/// Why the authoritative duplicate ledger could not be read.
#[derive(Debug)]
pub(in crate::search::evidence) enum DuplicateLedgerError {
    /// The set does not exist in the caller's scope.
    NotVisible,
    /// The scoped chunk-set record could not be read.
    ChunkSet(chunk_set::Error),
    /// The manifest artifact could not be read or verified.
    Artifacts(store::Error),
    /// The manifest was not strict JSON of its declared schema.
    Json(serde_json::Error),
    /// The set record and manifest are invalid or inconsistent.
    Invalid(String),
    /// The caller cancelled the blocking operation.
    Cancelled,
    /// The operation reached its absolute deadline.
    TimedOut,
}

impl fmt::Display for DuplicateLedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotVisible => formatter.write_str("chunk set is not visible"),
            Self::ChunkSet(error) => fmt::Display::fmt(error, formatter),
            Self::Artifacts(error) => fmt::Display::fmt(error, formatter),
            Self::Json(error) => fmt::Display::fmt(error, formatter),
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::Cancelled => formatter.write_str("duplicate ledger read was cancelled"),
            Self::TimedOut => formatter.write_str("duplicate ledger read reached its deadline"),
        }
    }
}

impl error::Error for DuplicateLedgerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::ChunkSet(error) => Some(error),
            Self::Artifacts(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::NotVisible | Self::Invalid(_) | Self::Cancelled | Self::TimedOut => None,
        }
    }
}

impl From<DuplicateLedgerError> for EvidenceError {
    fn from(error: DuplicateLedgerError) -> Self {
        match error {
            DuplicateLedgerError::NotVisible => Self::NotVisible,
            DuplicateLedgerError::ChunkSet(error) => Self::ChunkSet(error),
            DuplicateLedgerError::Artifacts(error) => Self::Store(error),
            DuplicateLedgerError::Json(error) => Self::Json(error),
            DuplicateLedgerError::Invalid(reason) => Self::Integrity(reason),
            DuplicateLedgerError::Cancelled | DuplicateLedgerError::TimedOut => Self::TimedOut,
        }
    }
}

/// Reads and validates the immutable manifest of the complete, visible chunk set.
pub(in crate::search::evidence) fn duplicate_ledger(
    database: &Database,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    control: &ReadControl,
) -> Result<DuplicateLedger, DuplicateLedgerError> {
    check(control)?;
    let set = database
        .chunk_set(scopes, chunk_set_id)
        .map_err(DuplicateLedgerError::ChunkSet)?;
    check(control)?;
    let set = set.ok_or(DuplicateLedgerError::NotVisible)?;
    if set.state != ChunkSetState::Complete {
        return Err(DuplicateLedgerError::Invalid(
            "duplicate ledger requires a complete chunk set".to_owned(),
        ));
    }
    let manifest_digest = set.manifest_digest.as_ref().ok_or_else(|| {
        DuplicateLedgerError::Invalid("complete chunk set has no manifest digest".to_owned())
    })?;

    check(control)?;
    let manifest = Manifest::read(database, manifest_digest).map_err(|error| match error {
        PrepareError::Artifacts(error) => DuplicateLedgerError::Artifacts(error),
        PrepareError::Manifest(error) => DuplicateLedgerError::Json(error),
        _ => DuplicateLedgerError::Invalid("chunk-set manifest could not be read".to_owned()),
    })?;
    check(control)?;
    validate_manifest(
        &manifest,
        &set.collection_id,
        &set.id,
        &set.chunk_profile,
        &set.counter_contract_id,
    )
}

/// Turns one parsed manifest into the only duplicate ledger for its set.
fn validate_manifest(
    manifest: &Manifest,
    collection_id: &str,
    chunk_set_id: &str,
    chunk_profile: &str,
    counter_contract_id: &str,
) -> Result<DuplicateLedger, DuplicateLedgerError> {
    if manifest.schema != SCHEMA
        || manifest.collection != collection_id
        || manifest.chunk_set != chunk_set_id
        || manifest.chunk_profile != chunk_profile
        || manifest.counter != counter_contract_id
    {
        return Err(DuplicateLedgerError::Invalid(
            "chunk-set manifest identity does not match its record".to_owned(),
        ));
    }

    let revisions: BTreeSet<_> = manifest.revisions.iter().cloned().collect();
    if revisions.len() != manifest.revisions.len()
        || revisions.iter().any(|revision| revision.trim().is_empty())
    {
        return Err(DuplicateLedgerError::Invalid(
            "chunk-set manifest revisions are blank or repeated".to_owned(),
        ));
    }
    let near_duplicate_groups: BTreeSet<_> =
        manifest.near_duplicate_groups.iter().cloned().collect();
    if near_duplicate_groups.len() != manifest.near_duplicate_groups.len()
        || near_duplicate_groups
            .iter()
            .any(|group| group.trim().is_empty())
    {
        return Err(DuplicateLedgerError::Invalid(
            "chunk-set manifest near-duplicate groups are blank or repeated".to_owned(),
        ));
    }
    for (duplicate, representative) in &manifest.duplicates {
        if !revisions.contains(duplicate) || !revisions.contains(representative) {
            return Err(DuplicateLedgerError::Invalid(
                "chunk-set manifest duplicate mapping names an unknown revision".to_owned(),
            ));
        }
        if duplicate == representative {
            return Err(DuplicateLedgerError::Invalid(
                "chunk-set manifest duplicate mapping is self-referential".to_owned(),
            ));
        }
        if manifest.duplicates.contains_key(representative) {
            return Err(DuplicateLedgerError::Invalid(
                "chunk-set manifest duplicate mapping contains a chain".to_owned(),
            ));
        }
    }

    Ok(DuplicateLedger {
        revisions,
        duplicates: manifest.duplicates.clone(),
        near_duplicate_groups,
    })
}

/// Stops before an operation when its shared cancellation or deadline has fired.
fn check(control: &ReadControl) -> Result<(), DuplicateLedgerError> {
    if control.cancelled.load(Ordering::Relaxed) {
        Err(DuplicateLedgerError::Cancelled)
    } else if Instant::now() >= control.deadline {
        Err(DuplicateLedgerError::TimedOut)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_canonicalization::ChunkProfile;
    use std::error::Error as _;

    fn manifest() -> Manifest {
        Manifest::new(
            "notes",
            "set",
            ChunkProfile::Structural,
            "counter",
            vec!["rev-a".to_owned(), "rev-b".to_owned(), "rev-c".to_owned()],
        )
    }

    fn assert_invalid(manifest: &Manifest) {
        assert!(matches!(
            validate_manifest(manifest, "notes", "set", &manifest.chunk_profile, "counter",),
            Err(DuplicateLedgerError::Invalid(_))
        ));
    }

    #[test]
    fn formats_errors_and_preserves_their_sources() {
        let invalid_json = serde_json::from_str::<Manifest>("{").unwrap_err();
        let error = DuplicateLedgerError::Json(invalid_json);

        assert!(error.to_string().contains("EOF while parsing an object"));
        assert!(error.source().is_some());
        assert_eq!(
            DuplicateLedgerError::Invalid("bad manifest".to_owned()).to_string(),
            "bad manifest"
        );
        assert!(
            DuplicateLedgerError::Invalid("bad manifest".to_owned())
                .source()
                .is_none()
        );
    }

    #[test]
    fn rejects_a_manifest_with_another_identity() {
        let mut other_set = manifest();
        other_set.chunk_set = "other-set".to_owned();
        assert_invalid(&other_set);

        let mut other_collection = manifest();
        other_collection.collection = "other-collection".to_owned();
        assert_invalid(&other_collection);

        let mut other_counter = manifest();
        other_counter.counter = "other-counter".to_owned();
        assert_invalid(&other_counter);
    }

    #[test]
    fn rejects_blank_or_repeated_revision_and_group_ids() {
        let mut revisions = manifest();
        revisions.revisions.push("rev-a".to_owned());
        assert_invalid(&revisions);

        let mut blank_revision = manifest();
        blank_revision.revisions.push(" ".to_owned());
        assert_invalid(&blank_revision);

        let mut groups = manifest();
        groups.near_duplicate_groups = vec!["near-a".to_owned(), "near-a".to_owned()];
        assert_invalid(&groups);

        let mut blank_group = manifest();
        blank_group.near_duplicate_groups.push(String::new());
        assert_invalid(&blank_group);
    }

    #[test]
    fn rejects_duplicate_mappings_outside_the_set_self_mappings_and_chains() {
        let mut outside = manifest();
        outside
            .duplicates
            .insert("unknown".to_owned(), "rev-a".to_owned());
        assert_invalid(&outside);

        let mut blank_duplicate = manifest();
        blank_duplicate
            .duplicates
            .insert(" ".to_owned(), "rev-a".to_owned());
        assert_invalid(&blank_duplicate);

        let mut self_mapping = manifest();
        self_mapping
            .duplicates
            .insert("rev-a".to_owned(), "rev-a".to_owned());
        assert_invalid(&self_mapping);

        let mut chain = manifest();
        chain
            .duplicates
            .insert("rev-a".to_owned(), "rev-b".to_owned());
        chain
            .duplicates
            .insert("rev-b".to_owned(), "rev-c".to_owned());
        assert_invalid(&chain);
    }
}
