//! Importing one entry of a manifest: its document read and checked against
//! its line, canonicalized under the ID of its document in its collection,
//! stored as two artifacts and recorded as a revision, held in the same write
//! when its manifest gives its `source_ref` other bytes too.

use super::{
    corpus::Corpus,
    error::Error,
    report::{Imported, Reason},
};
use crate::corpus::Entry;
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, SourceMetadata, ValidationStatus, canonicalize,
};
use maestro_kernel::{
    artifact::Digest,
    document::{self, Disposition, Document, Outcome, Recorded, Revision, RevisionStatus},
    scope::ScopeSet,
    store::{self, Database},
};
use serde_json::{Map, Value};

/// The namespace of the document IDs an import derives, apart from
/// canonicalization's own, `source` and `local`.
const NAMESPACE: &str = "collection";

/// The rule that holds the revisions of lines of one manifest that give
/// their `source_ref` different digests.
const SHARED_SOURCE_REF: &str = "import.shared-source-ref";

/// Where an import records the entries of one source, and the scopes it
/// reads them with.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target<'a> {
    /// The kernel's database.
    pub(super) database: &'a Database,
    /// What the caller may read.
    pub(super) scopes: &'a ScopeSet,
    /// The collection's ID.
    pub(super) collection: &'a str,
    /// The source's ID.
    pub(super) source: &'a str,
}

/// Why an entry was not imported: a refusal of that entry alone, after which
/// the import goes on, or a failure that stops the import.
#[derive(Debug)]
pub(super) enum NotImported {
    /// The entry is refused, for this reason.
    Refused(Reason),
    /// The import stops.
    Failed(Error),
}

impl From<Reason> for NotImported {
    fn from(reason: Reason) -> Self {
        Self::Refused(reason)
    }
}

impl From<document::Error> for NotImported {
    /// A conflict with what the kernel records refuses the entry; any other
    /// failure of the kernel stops the import.
    fn from(error: document::Error) -> Self {
        match error {
            document::Error::DocumentConflict(_)
            | document::Error::SourceRefConflict { .. }
            | document::Error::RevisionConflict(_) => Self::Refused(Reason::Conflict {
                message: error.to_string(),
            }),
            document::Error::InvalidId(_) | document::Error::Store(_) => {
                Self::Failed(Error::Records(error))
            }
        }
    }
}

impl From<store::Error> for NotImported {
    /// Bytes the kernel records under another media type refuse the entry;
    /// any other failure of the store stops the import.
    fn from(error: store::Error) -> Self {
        match error {
            store::Error::MediaConflict { .. } => Self::Refused(Reason::Conflict {
                message: error.to_string(),
            }),
            other => Self::Failed(Error::Artifacts(other)),
        }
    }
}

/// Imports `entry`, a line of the manifest of `target`'s source, from
/// `corpus`: `shared` when another line of that manifest gives its
/// `source_ref` other bytes, which holds its revision back. A new revision
/// that is held is recorded with its hold in one write, so none is ever
/// recorded undecided. A revision the kernel records already is left as it
/// is: nothing is written for it but the hold it lacks, when its
/// `source_ref` became shared after an earlier import recorded it. One whose
/// document another source of the collection holds is refused, as a line
/// that differs by one byte is: a revision's ID names its document, not
/// its source.
pub(super) fn import(
    target: &Target<'_>,
    corpus: &impl Corpus,
    entry: &Entry,
    shared: bool,
) -> Result<Imported, NotImported> {
    let markdown = verified(corpus, entry)?;
    let document_id = document_id(target.collection, &entry.source_ref);
    let canonical = canonical(&markdown, entry, &document_id)?;
    let hold = shared.then(|| quarantine(target.source, &canonical.revision_id, &entry.source_ref));
    let database = target.database;
    let recorded = if database
        .revision(target.scopes, &canonical.revision_id)?
        .is_some()
    {
        let held_elsewhere = database
            .document(target.scopes, &document_id)?
            .is_some_and(|document| document.source_id != target.source);
        if held_elsewhere {
            return Err(document::Error::DocumentConflict(document_id).into());
        }
        match &hold {
            Some(hold) => database.record_disposition(hold)?,
            None => Recorded::Unchanged,
        }
    } else {
        let revision = store(target, entry, &document_id, &markdown, &canonical)?;
        match &hold {
            Some(hold) => database.record_revision_with_disposition(&revision, hold)?,
            None => database.record_revision(&revision)?,
        }
    };
    Ok(match (recorded, hold) {
        (Recorded::Unchanged, _) => Imported::Unchanged,
        (Recorded::New, None) => Imported::New,
        (Recorded::New, Some(_)) => Imported::Held,
    })
}

/// The document of `entry`, read from `corpus` and checked against its
/// line: its digest, then its size, then its encoding.
fn verified(corpus: &impl Corpus, entry: &Entry) -> Result<String, Reason> {
    let bytes = corpus
        .document(&entry.path)
        .map_err(|error| Reason::Unreadable {
            path: entry.path.as_str().to_owned(),
            message: error.to_string(),
        })?;
    let found = Digest::of(&bytes);
    if found != entry.sha256 {
        let declared = entry.sha256.clone();
        return Err(Reason::DigestMismatch { declared, found });
    }
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size != entry.bytes.get() {
        let declared = entry.bytes.get();
        return Err(Reason::SizeMismatch {
            declared,
            found: size,
        });
    }
    String::from_utf8(bytes).map_err(|_| Reason::NotUtf8)
}

/// The ID of the document `source_ref` names in the collection
/// `collection`: `doc-` followed by the SHA-256, in hexadecimal, of
/// `collection`, the collection's ID and `source_ref`, separated by NUL
/// bytes. A collection's ID is a scope name, which holds no NUL, so no two
/// pairs give one text; and a `source_ref` names no release, so a document
/// keeps its ID from one release to the next.
fn document_id(collection: &str, source_ref: &str) -> String {
    let named = format!("{NAMESPACE}\0{collection}\0{source_ref}");
    format!("doc-{}", Digest::of(named.as_bytes()).as_str())
}

/// `markdown`, canonicalized as the document `document_id`, with what
/// `entry` says of it.
fn canonical(
    markdown: &str,
    entry: &Entry,
    document_id: &str,
) -> Result<CanonicalDocument, Reason> {
    let mut input = CanonicalizeInput::new(markdown, &entry.source_ref);
    input.document_id = Some(document_id);
    input.metadata = source_metadata(entry);
    canonicalize(input).map_err(|error| Reason::Canonicalization { message: error.0 })
}

/// What `entry` says of its document, as canonicalization reads it into the
/// revision's identity: its source reference, title, language, extractor
/// and access in their own fields, and its other fields under the names the
/// manifest gives them. A change to any of them gives a new revision.
fn source_metadata(entry: &Entry) -> SourceMetadata {
    let mut extra = metadata(entry);
    for own_field in ["title", "lang", "extractor", "access"] {
        extra.remove(own_field);
    }
    SourceMetadata {
        source_reference: Some(entry.source_ref.clone()),
        title: Some(entry.title.clone()),
        language: entry.lang.clone(),
        extraction: entry.extractor.clone().map(Value::Object),
        access_policy: entry.access.clone().map(Value::Object),
        extra: extra.into_iter().collect(),
    }
}

/// Every field of `entry` that describes its document, under the name the
/// manifest gives it: the revision's metadata.
fn metadata(entry: &Entry) -> Map<String, Value> {
    let text = |value: Option<&String>| value.map(|text| Value::from(text.as_str()));
    [
        ("title", text(Some(&entry.title))),
        ("source_kind", text(Some(&entry.source_kind))),
        ("set", text(entry.set.as_ref())),
        ("version", text(entry.version.as_ref())),
        ("lang", text(entry.lang.as_ref())),
        ("captured_at", text(entry.captured_at.as_ref())),
        ("product", text(entry.product.as_ref())),
        ("component", text(entry.component.as_ref())),
        ("platform", text(entry.platform.as_ref())),
        ("extractor", entry.extractor.clone().map(Value::Object)),
        ("access", entry.access.clone().map(Value::Object)),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name.to_owned(), value?)))
    .collect()
}

/// Stores the original of `entry`, `markdown`, and its canonical document
/// as artifacts, records its document, and returns its revision, for the
/// caller to record: that record pins both artifacts.
fn store(
    target: &Target<'_>,
    entry: &Entry,
    document_id: &str,
    markdown: &str,
    canonical: &CanonicalDocument,
) -> Result<Revision, NotImported> {
    let database = target.database;
    let original_digest = database.put(markdown.as_bytes(), "text/markdown")?;
    let canonical_digest = database.put(&encoded(canonical)?, "application/json")?;
    database.record_document(&Document {
        id: document_id.to_owned(),
        collection_id: target.collection.to_owned(),
        source_id: target.source.to_owned(),
        source_ref: entry.source_ref.clone(),
    })?;
    Ok(Revision {
        id: canonical.revision_id.clone(),
        document_id: document_id.to_owned(),
        original_digest,
        canonical_digest,
        status: status(&canonical.validation_status),
        captured_at: entry.captured_at.clone(),
        metadata: metadata(entry),
    })
}

/// `document` encoded as `save_document` encodes it: pretty JSON and a final
/// newline.
fn encoded(document: &CanonicalDocument) -> Result<Vec<u8>, Reason> {
    let mut json =
        serde_json::to_vec_pretty(document).map_err(|error| Reason::Canonicalization {
            message: error.to_string(),
        })?;
    json.push(b'\n');
    Ok(json)
}

/// The status of a revision to which canonicalization gave `verdict`.
fn status(verdict: &ValidationStatus) -> RevisionStatus {
    match verdict {
        ValidationStatus::Valid => RevisionStatus::Valid,
        ValidationStatus::ValidWithWarnings => RevisionStatus::ValidWithWarnings,
        ValidationStatus::Failed => RevisionStatus::Failed,
    }
}

/// The hold of the revision `revision_id` of the source `source`:
/// quarantined by the import, since lines of its manifest give `source_ref`
/// different digests, and neither may replace the other until someone
/// decides.
fn quarantine(source: &str, revision_id: &str, source_ref: &str) -> Disposition {
    Disposition {
        revision_id: revision_id.to_owned(),
        outcome: Outcome::Quarantined,
        reasons: vec![format!(
            "lines of the manifest of the source {source} give the source_ref {source_ref} \
             different digests: neither replaces the other until someone decides"
        )],
        rule_ids: vec![SHARED_SOURCE_REF.to_owned()],
        decided_by: "import".to_owned(),
    }
}
