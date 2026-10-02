//! One integrity, canonicalization and immutable revision path for both ingress routes.
use super::{
    error::Error,
    report::{Imported, Reason},
};
use maestro_canonicalization::{
    AssetStatus, CanonicalDocument, CanonicalizeInput, ExtractorBlock, SourceMetadata,
    ValidationStatus, canonicalize,
};
use maestro_kernel::{
    acquisition::{Handle, RevisionLink},
    artifact::Digest,
    document::{self, Disposition, Document, Outcome, Recorded, Revision, RevisionStatus},
    scope::{ScopeSet, source_path},
    store::{self, Database},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, str};

/// Core-reserved native semantic inventory domain and metadata key.
const ASSETS: &str = "maestro.native_assets/1";

/// Declared source and current read grants for one ingestion.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    /// Existing canonical artifact and document store.
    pub database: &'a Database,
    /// Current caller grants.
    pub scopes: &'a ScopeSet,
    /// Declared collection ID.
    pub collection: &'a str,
    /// Declared source ID.
    pub source: &'a str,
}
/// Entry refusal or a failure stopping its containing import.
#[derive(Debug)]
pub enum NotImported {
    /// Refused input, without recording a revision.
    Refused(Reason),
    /// Kernel or artifact failure.
    Failed(Error),
}
impl From<Reason> for NotImported {
    fn from(reason: Reason) -> Self {
        Self::Refused(reason)
    }
}
impl From<document::Error> for NotImported {
    fn from(error: document::Error) -> Self {
        match error {
            document::Error::DocumentConflict(_)
            | document::Error::SourceRefConflict { .. }
            | document::Error::RevisionConflict(_) => Self::Refused(Reason::Conflict {
                message: error.to_string(),
            }),
            other => Self::Failed(Error::Records(other)),
        }
    }
}
impl From<store::Error> for NotImported {
    fn from(error: store::Error) -> Self {
        match error {
            store::Error::MediaConflict { .. } => Self::Refused(Reason::Conflict {
                message: error.to_string(),
            }),
            other => Self::Failed(Error::Artifacts(other)),
        }
    }
}
/// Relevant semantic asset record. Transfer URLs, paths and attempt times do not belong here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    /// Stable destination referenced by Markdown/extractor structures or required by the profile.
    pub destination: String,
    /// Explicit availability, never inferred from nonempty text.
    pub status: AssetStatus,
    /// Verified shared-store content digest; null when unknown.
    pub digest: Option<Digest>,
    /// Verified byte length; null when unknown.
    pub length: Option<u64>,
}
/// Evidence outside canonical identity and the original Markdown reference.
#[derive(Debug, Clone)]
pub struct MappedEvidence {
    /// Scoped immutable capture envelope.
    pub capture: Handle,
    /// Scoped fidelity receipt retaining the full extraction records.
    pub fidelity: Handle,
}
/// Native mapped input, without a second document or asset store.
#[derive(Debug, Clone)]
pub struct MappedInput<'a> {
    /// Immutable extracted Markdown bytes, never raw PDF/HTML bytes.
    pub markdown: &'a [u8],
    /// Declared Markdown digest, independently verified here.
    pub digest: Digest,
    /// Declared Markdown length, independently verified here.
    pub length: u64,
    /// Exact legacy spelling; no URL normalization or alias migration.
    pub source_ref: &'a str,
    /// Source metadata, permissions and profile identities for canonicalization.
    pub metadata: SourceMetadata,
    /// Existing S1 revision metadata fields.
    pub revision_metadata: Map<String, Value>,
    /// Source-supplied capture time, when known.
    pub captured_at: Option<String>,
    /// Observations excluded from semantic identity.
    pub operational_metadata: BTreeMap<String, Value>,
    /// Existing typed extractor mappings with supplied or unknown original coordinates.
    pub extractor_blocks: Vec<ExtractorBlock>,
    /// Complete relevant semantic asset inventory.
    pub assets: Vec<AssetRecord>,
    /// Approved quality decision; its revision ID is bound by core, not the caller.
    pub disposition: Option<Disposition>,
    /// Separate resolvable raw-capture and fidelity references.
    pub evidence: MappedEvidence,
}
/// Common inputs after integrity checks; corpus/1 supplies no native link or additions.
pub(super) struct IngestInput<'a> {
    /// Single canonicalizer input.
    pub(super) canonical: CanonicalizeInput<'a>,
    /// Existing revision metadata.
    pub(super) metadata: Map<String, Value>,
    /// Existing source capture time.
    pub(super) captured_at: Option<String>,
    /// Quality hold or approved outcome, revision ID rebound below.
    pub(super) disposition: Option<Disposition>,
    /// Native evidence and semantic inventory, absent for corpus/1.
    pub(super) link: Option<RevisionLink>,
}
/// Verifies native additions and enters the same S1 revision/disposition transaction.
/// Equal semantic inventory is unchanged even across operationally different attempts.
///
/// # Errors
/// Refuses corrupt bytes, forged reserved metadata, conflicting assets or unsafe mappings;
/// returns kernel failures without recording a partial revision/link/disposition.
pub fn ingest_mapped(target: &Target<'_>, input: MappedInput<'_>) -> Result<Imported, NotImported> {
    let markdown = verified(input.markdown, &input.digest, input.length)?;
    if target
        .database
        .source(target.scopes, target.collection, target.source)?
        .is_none()
    {
        return Err(invalid("mapped source is undeclared or not visible").into());
    }
    if input.metadata.extra.contains_key(ASSETS) {
        return Err(invalid("source supplied the core-reserved native asset key").into());
    }
    if input.metadata.source_reference.as_deref() != Some(input.source_ref) {
        return Err(invalid("mapped source reference differs from its metadata").into());
    }
    validate_locations(&input.extractor_blocks, input.source_ref)?;
    let (inventory, assets) = inventory(target.database, &input.assets)?;
    let digest = target.database.put(&inventory, "application/json")?;
    let mut canonical = CanonicalizeInput::new(markdown, input.source_ref);
    canonical.metadata = input.metadata;
    canonical
        .metadata
        .extra
        .insert(ASSETS.into(), Value::from(digest.as_str()));
    canonical.extractor_blocks = input.extractor_blocks;
    canonical.assets = assets;
    canonical.operational_metadata = input.operational_metadata;
    ingest(
        target,
        IngestInput {
            canonical,
            metadata: input.revision_metadata,
            captured_at: input.captured_at,
            disposition: input.disposition,
            link: Some(RevisionLink {
                revision: String::new(),
                capture: input.evidence.capture,
                fidelity: input.evidence.fidelity,
                inventory: digest,
            }),
        },
    )
}
/// Independent declared digest/length and UTF-8 checks shared with corpus/1.
pub(super) fn verified<'a>(
    bytes: &'a [u8],
    digest: &Digest,
    length: u64,
) -> Result<&'a str, Reason> {
    let found = Digest::of(bytes);
    if found != *digest {
        return Err(Reason::DigestMismatch {
            declared: digest.clone(),
            found,
        });
    }
    let found = bytes.len() as u64;
    if found != length {
        return Err(Reason::SizeMismatch {
            declared: length,
            found,
        });
    }
    str::from_utf8(bytes).map_err(|_| Reason::NotUtf8)
}
/// Sorted unique compact tuple preimage; operational observations never enter it.
fn inventory(
    db: &Database,
    records: &[AssetRecord],
) -> Result<(Vec<u8>, BTreeMap<String, AssetStatus>), NotImported> {
    let mut unique = BTreeMap::new();
    for record in records {
        verify_asset(db, record)?;
        if let Some(previous) = unique.insert(record.destination.as_str(), record)
            && previous != record
        {
            return Err(invalid("conflicting duplicate asset destination").into());
        }
    }
    let tuples: Vec<_> = unique
        .values()
        .map(|record| {
            (
                &record.destination,
                &record.status,
                &record.digest,
                record.length,
            )
        })
        .collect();
    let bytes =
        serde_json::to_vec(&(ASSETS, tuples)).map_err(|error| invalid(&error.to_string()))?;
    let assets = unique
        .values()
        .map(|record| (record.destination.clone(), record.status.clone()))
        .collect();
    Ok((bytes, assets))
}
/// Available assets must resolve to independently verified bytes before even an unchanged replay.
fn verify_asset(db: &Database, record: &AssetRecord) -> Result<(), NotImported> {
    if record.destination.is_empty() {
        return Err(invalid("empty semantic asset destination").into());
    }
    if record.status == AssetStatus::Available {
        let (Some(digest), Some(length)) = (&record.digest, record.length) else {
            return Err(invalid("available asset lacks digest or length").into());
        };
        let bytes = db.get(digest)?;
        if bytes.len() as u64 != length {
            return Err(invalid("asset byte length mismatch").into());
        }
    }
    Ok(())
}
/// Supplied original-media references cannot silently point at a different source.
fn validate_locations(blocks: &[ExtractorBlock], source_ref: &str) -> Result<(), Reason> {
    for location in blocks.iter().flat_map(|block| &block.original_locations) {
        if location
            .source_reference
            .as_deref()
            .is_some_and(|reference| reference != source_ref)
        {
            return Err(invalid("extractor location names another source"));
        }
    }
    Ok(())
}
/// Shared single canonicalization and immutable revision/disposition path.
pub(super) fn ingest(target: &Target<'_>, input: IngestInput<'_>) -> Result<Imported, NotImported> {
    let source_ref = input.canonical.identity_key;
    let markdown = input.canonical.markdown;
    let named = format!("collection\0{}\0{source_ref}", target.collection);
    let document_id = format!("doc-{}", Digest::of(named.as_bytes()).as_str());
    let canonical_input = CanonicalizeInput {
        document_id: Some(&document_id),
        ..input.canonical
    };
    let canonical = canonicalize(canonical_input).map_err(|error| invalid(&error.0))?;
    if input.link.is_some() {
        if canonical.validation_status == ValidationStatus::Failed {
            return Err(invalid("mapped canonical validation failed").into());
        }
        validate_references(&canonical)?;
    }
    let db = target.database;
    let disposition = input.disposition.map(|hold| Disposition {
        revision_id: canonical.revision_id.clone(),
        ..hold
    });
    let existing = db.revision(target.scopes, &canonical.revision_id)?;
    let recorded = if let Some(revision) = existing {
        if db
            .document(target.scopes, &document_id)?
            .is_some_and(|document| document.source_id != target.source)
        {
            return Err(document::Error::DocumentConflict(document_id).into());
        }
        record(target, &revision, disposition.as_ref(), input.link)?
    } else {
        let original_digest = db.put(markdown.as_bytes(), "text/markdown")?;
        let canonical_digest = db.put(&encoded(&canonical)?, "application/json")?;
        db.record_document(&Document {
            id: document_id.clone(),
            collection_id: target.collection.into(),
            source_id: target.source.into(),
            source_ref: source_ref.into(),
        })?;
        let revision = Revision {
            id: canonical.revision_id,
            document_id,
            original_digest,
            canonical_digest,
            status: status(&canonical.validation_status),
            captured_at: input.captured_at,
            metadata: input.metadata,
        };
        record(target, &revision, disposition.as_ref(), input.link)?
    };
    Ok(match (recorded, disposition) {
        (Recorded::Unchanged, _) => Imported::Unchanged,
        (
            Recorded::New,
            None
            | Some(Disposition {
                outcome: Outcome::Accepted | Outcome::AcceptedWithWarnings,
                ..
            }),
        ) => Imported::New,
        (Recorded::New, Some(_)) => Imported::Held,
    })
}
/// Reuse canonical classification: native local destinations cannot disappear from the inventory.
fn validate_references(document: &CanonicalDocument) -> Result<(), Reason> {
    for asset in document
        .blocks
        .iter()
        .flat_map(|block| &block.asset_references)
    {
        if !matches!(asset.status, AssetStatus::Remote | AssetStatus::Fragment)
            && !document.asset_inventory.contains_key(&asset.destination)
        {
            return Err(invalid(
                "local asset reference lacks a semantic inventory record",
            ));
        }
    }
    Ok(())
}

/// Records revision, hold journal and optional native relation in their single write.
fn record(
    target: &Target<'_>,
    revision: &Revision,
    disposition: Option<&Disposition>,
    link: Option<RevisionLink>,
) -> Result<Recorded, NotImported> {
    let db = target.database;
    Ok(match link {
        Some(link) => {
            let scope = source_path(target.collection, target.source)
                .parse()
                .map_err(|_| invalid("invalid mapped source scope"))?;
            db.record_mapped_revision(
                revision,
                disposition,
                &scope,
                &RevisionLink {
                    revision: revision.id.clone(),
                    ..link
                },
            )?
        }
        None => match disposition {
            Some(hold) => db.record_revision_with_disposition(revision, hold)?,
            None => db.record_revision(revision)?,
        },
    })
}
/// Canonical pretty JSON plus final newline, identical to the corpus/1 artifact encoding.
fn encoded(document: &CanonicalDocument) -> Result<Vec<u8>, Reason> {
    let mut json =
        serde_json::to_vec_pretty(document).map_err(|error| invalid(&error.to_string()))?;
    json.push(b'\n');
    Ok(json)
}
/// Existing canonical verdict to kernel status mapping.
fn status(verdict: &ValidationStatus) -> RevisionStatus {
    match verdict {
        ValidationStatus::Valid => RevisionStatus::Valid,
        ValidationStatus::ValidWithWarnings => RevisionStatus::ValidWithWarnings,
        ValidationStatus::Failed => RevisionStatus::Failed,
    }
}
/// Typed refusal for invalid mapped content.
fn invalid(message: &str) -> Reason {
    Reason::Canonicalization {
        message: message.into(),
    }
}
