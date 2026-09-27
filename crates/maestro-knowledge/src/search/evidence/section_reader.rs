//! Reads one authorized canonical section from a completed chunk set.

use super::sections::SectionIndex;
use maestro_canonicalization::{CanonicalDocument, Section, ValidationStatus};
use maestro_kernel::{
    artifact::{Digest, Error as ArtifactError},
    chunk_set::{self, ChunkSetState},
    document::{self, Document, Outcome, Revision, RevisionStatus},
    evidence::Span,
    scope::ScopeSet,
    store::{self, Database},
};
use serde_json::Value;
use std::{error, fmt};

/// A section excerpt read from its authoritative original Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionExcerpt {
    /// The canonical document containing the section.
    pub document_id: String,
    /// The admitted revision containing the section.
    pub revision_id: String,
    /// The canonical section identity.
    pub section_id: String,
    /// The source reference recorded for the document.
    pub source_ref: String,
    /// The canonical title, or an empty string when unknown.
    pub title: String,
    /// The section's canonical path of ancestor headings.
    pub section_path: Vec<String>,
    /// The literal version metadata, if it is text.
    pub version: Option<String>,
    /// Half-open UTF-8 byte offsets in the original Markdown.
    pub span: [usize; 2],
    /// SHA-256 digest of the exact excerpt bytes.
    pub digest: Digest,
    /// The exact original Markdown slice.
    pub text: String,
}

/// Why an authorized section could not be returned.
#[derive(Debug)]
pub enum SectionReadError {
    /// The section is absent or outside the caller's scopes.
    NotFound,
    /// This section ID names more than one document/revision pair in the set.
    Ambiguous,
    /// The exact source extent exceeds the caller's byte limit.
    TooLarge {
        /// The full extent's byte length.
        bytes: usize,
    },
    /// The canonical document or source content failed an integrity check.
    Integrity,
    /// The kernel store or record lookup failed; the kernel error is its source.
    Store(Box<dyn error::Error + Send + Sync>),
}

impl fmt::Display for SectionReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("section is unknown or inaccessible"),
            Self::Ambiguous => {
                formatter.write_str("section identity is ambiguous in this chunk set")
            }
            Self::TooLarge { bytes } => {
                write!(
                    formatter,
                    "section extent is {bytes} bytes, above the requested limit"
                )
            }
            Self::Integrity => formatter.write_str("section content failed an integrity check"),
            Self::Store(_) => formatter.write_str("kernel store could not read section data"),
        }
    }
}

impl error::Error for SectionReadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(source) => Some(source.as_ref()),
            Self::NotFound | Self::Ambiguous | Self::TooLarge { .. } | Self::Integrity => None,
        }
    }
}

/// Reads one visible section's exact source extent from a complete chunk set.
///
/// `max_bytes` caps only the returned excerpt. The canonical artifact is read
/// whole to map the section to source spans; the original is read whole only
/// after its extent passes the limit. No canonical replay is performed.
///
/// # Errors
///
/// Returns `NotFound` for an absent or ineligible section, `Ambiguous` when
/// its ID names multiple visible document/revision pairs, `TooLarge` when the
/// extent exceeds `max_bytes` (decided before the original is read),
/// `Integrity` for inconsistent canonical data, or `Store` when a kernel read
/// fails.
pub fn read_section(
    database: &Database,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    section_id: &str,
    max_bytes: usize,
) -> Result<SectionExcerpt, SectionReadError> {
    let (chunk_set, revision, document) =
        find_section_document(database, scopes, chunk_set_id, section_id)?;
    let (ChunkSetState::Complete, Some(_)) = (chunk_set.state, &chunk_set.manifest_digest) else {
        return Err(SectionReadError::NotFound);
    };
    let disposition = database
        .disposition(scopes, &revision.id)
        .map_err(map_document_error)?
        .ok_or(SectionReadError::NotFound)?;
    if !section_is_eligible(revision.status, disposition.outcome) {
        return Err(SectionReadError::NotFound);
    }
    let canonical = read_canonical_document(database, &revision, &document)?;
    let bounded = bounded_section_extent(database, &revision, &canonical, section_id, max_bytes)?;
    let markdown = read_verified_markdown(database, &revision, &canonical, &bounded)?;
    let text = markdown
        .get(bounded.extent.start..bounded.extent.end)
        .ok_or(SectionReadError::Integrity)?;

    Ok(SectionExcerpt {
        document_id: document.id,
        revision_id: revision.id,
        section_id: bounded.section.section_id.clone(),
        source_ref: document.source_ref,
        title: canonical.source_metadata.title.clone().unwrap_or_default(),
        section_path: bounded.section.heading_path.clone(),
        version: revision
            .metadata
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        span: [bounded.extent.start, bounded.extent.end],
        digest: Digest::of(text.as_bytes()),
        text: text.to_owned(),
    })
}

/// Whether both the revision and its current disposition allow source reading.
fn section_is_eligible(status: RevisionStatus, outcome: Outcome) -> bool {
    status != RevisionStatus::Failed
        && matches!(outcome, Outcome::Accepted | Outcome::AcceptedWithWarnings)
}

/// Finds the sole currently eligible revision and document naming this section.
fn find_section_document(
    database: &Database,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    section_id: &str,
) -> Result<(chunk_set::ChunkSet, Revision, Document), SectionReadError> {
    let pairs = database
        .section_revisions(scopes, chunk_set_id, section_id)
        .map_err(map_chunk_set_error)?;
    let [(document_id, revision_id)] = pairs.as_slice() else {
        return Err(if pairs.is_empty() {
            SectionReadError::NotFound
        } else {
            SectionReadError::Ambiguous
        });
    };
    let chunk_set = database
        .chunk_set_for_revision(scopes, chunk_set_id, revision_id)
        .map_err(map_chunk_set_error)?
        .ok_or(SectionReadError::NotFound)?;
    let revision = database
        .revision(scopes, revision_id)
        .map_err(map_document_error)?
        .ok_or(SectionReadError::NotFound)?;
    let document = database
        .document(scopes, document_id)
        .map_err(map_document_error)?
        .ok_or(SectionReadError::NotFound)?;
    if revision.document_id != document.id {
        return Err(SectionReadError::Integrity);
    }
    if document.collection_id != chunk_set.collection_id {
        return Err(SectionReadError::Integrity);
    }
    Ok((chunk_set, revision, document))
}

/// Loads the canonical record only when its identity and content hash agree.
fn read_canonical_document(
    database: &Database,
    revision: &Revision,
    document: &Document,
) -> Result<CanonicalDocument, SectionReadError> {
    let bytes = database
        .get(&revision.canonical_digest)
        .map_err(map_artifact_error)?;
    let canonical: CanonicalDocument =
        serde_json::from_slice(&bytes).map_err(|_| SectionReadError::Integrity)?;
    let expected_hash = format!("sha256:{}", revision.original_digest.as_str());
    if canonical.document_id != document.id {
        return Err(SectionReadError::Integrity);
    }
    if canonical.revision_id != revision.id {
        return Err(SectionReadError::Integrity);
    }
    if canonical.content_hash != expected_hash {
        return Err(SectionReadError::Integrity);
    }
    if canonical.original_markdown_reference.content_hash != expected_hash {
        return Err(SectionReadError::Integrity);
    }
    if canonical.validation_status == ValidationStatus::Failed {
        return Err(SectionReadError::Integrity);
    }
    Ok(canonical)
}

/// The selected section and its extent computed without original text.
struct BoundedSection<'a> {
    /// Canonical identity and presentation metadata for the section.
    section: &'a Section,
    /// Exact half-open byte extent in the original Markdown.
    extent: Span,
    /// Recorded byte length of the original artifact.
    source_bytes: u64,
}

/// Checks the full source extent against the limit without loading its bytes.
fn bounded_section_extent<'a>(
    database: &Database,
    revision: &Revision,
    canonical: &'a CanonicalDocument,
    section_id: &str,
    max_bytes: usize,
) -> Result<BoundedSection<'a>, SectionReadError> {
    let section = canonical
        .sections
        .iter()
        .find(|section| section.section_id == section_id)
        .ok_or(SectionReadError::Integrity)?;
    let artifact = database
        .artifact(&revision.original_digest)
        .map_err(map_store_error)?
        .ok_or(SectionReadError::Integrity)?;
    if u64::try_from(canonical.original_markdown_reference.byte_length).ok() != Some(artifact.bytes)
    {
        return Err(SectionReadError::Integrity);
    }
    let source_length = usize::try_from(artifact.bytes).map_err(|_| SectionReadError::Integrity)?;
    let index = SectionIndex::new_from_length(canonical, source_length)
        .map_err(|_| SectionReadError::Integrity)?;
    let extent = index
        .section_extent(section_id)
        .ok_or(SectionReadError::Integrity)?;
    if extent.start >= extent.end {
        return Err(SectionReadError::Integrity);
    }
    let bytes = extent.end - extent.start;
    if bytes > max_bytes {
        return Err(SectionReadError::TooLarge { bytes });
    }
    Ok(BoundedSection {
        section,
        extent,
        source_bytes: artifact.bytes,
    })
}

/// Reads and digest-checks the original, then confirms its canonical extent.
fn read_verified_markdown(
    database: &Database,
    revision: &Revision,
    canonical: &CanonicalDocument,
    bounded: &BoundedSection<'_>,
) -> Result<String, SectionReadError> {
    let original = database
        .get(&revision.original_digest)
        .map_err(map_artifact_error)?;
    if u64::try_from(original.len()).ok() != Some(bounded.source_bytes) {
        return Err(SectionReadError::Integrity);
    }
    let markdown = String::from_utf8(original).map_err(|_| SectionReadError::Integrity)?;
    let index = SectionIndex::new(canonical, &markdown).map_err(|_| SectionReadError::Integrity)?;
    if index.section_extent(&bounded.section.section_id) != Some(bounded.extent) {
        return Err(SectionReadError::Integrity);
    }
    Ok(markdown)
}

/// Keeps a chunk-set query error attached to the public Store variant.
fn map_chunk_set_error(error: chunk_set::Error) -> SectionReadError {
    SectionReadError::Store(Box::new(error))
}

/// Keeps a scoped record-read error attached to the public Store variant.
fn map_document_error(error: document::Error) -> SectionReadError {
    SectionReadError::Store(Box::new(error))
}

/// Treats absent or corrupt content-addressed artifacts as integrity failures.
fn map_artifact_error(error: store::Error) -> SectionReadError {
    match error {
        store::Error::Artifact(ArtifactError::Missing(_) | ArtifactError::Corrupt { .. }) => {
            SectionReadError::Integrity
        }
        error => SectionReadError::Store(Box::new(error)),
    }
}

/// Keeps artifact-record query errors attached to the public Store variant.
fn map_store_error(error: store::Error) -> SectionReadError {
    SectionReadError::Store(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::section_is_eligible;
    use maestro_kernel::document::{Outcome, RevisionStatus};

    #[test]
    fn requires_an_eligible_revision_and_an_accepted_disposition() {
        assert!(section_is_eligible(
            RevisionStatus::Valid,
            Outcome::Accepted
        ));
        assert!(section_is_eligible(
            RevisionStatus::ValidWithWarnings,
            Outcome::AcceptedWithWarnings,
        ));
        assert!(!section_is_eligible(
            RevisionStatus::Failed,
            Outcome::Accepted,
        ));
        assert!(!section_is_eligible(
            RevisionStatus::Valid,
            Outcome::Quarantined,
        ));
    }
}
