//! corpus/1 adapter: no native additions, no change to its metadata or identity bytes.
use super::{
    corpus::Corpus,
    ingest::{IngestInput, NotImported, Target, ingest, verified},
    report::{Imported, Reason},
};
use crate::corpus::Entry;
use maestro_canonicalization::{CanonicalizeInput, SourceMetadata};
use maestro_kernel::document::{Disposition, Outcome};
use serde_json::{Map, Value};

/// The unchanged corpus/1 shared-reference hold rule.
const SHARED_SOURCE_REF: &str = "import.shared-source-ref";

/// Reads a corpus entry and enters the shared integrity/revision path with empty additions.
pub(super) fn import(
    target: &Target<'_>,
    corpus: &impl Corpus,
    entry: &Entry,
    shared: bool,
) -> Result<Imported, NotImported> {
    let bytes = corpus
        .document(&entry.path)
        .map_err(|error| Reason::Unreadable {
            path: entry.path.as_str().to_owned(),
            message: error.to_string(),
        })?;
    let markdown = verified(&bytes, &entry.sha256, entry.bytes.get())?;
    let mut canonical = CanonicalizeInput::new(markdown, &entry.source_ref);
    canonical.metadata = source_metadata(entry);
    ingest(
        target,
        IngestInput {
            canonical,
            metadata: metadata(entry),
            captured_at: entry.captured_at.clone(),
            disposition: shared.then(|| quarantine(target.source, &entry.source_ref)),
            link: None,
        },
    )
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

/// The hold of the revision of the source `source`:
/// quarantined by the import, since lines of its manifest give `source_ref`
/// different digests, and neither may replace the other until someone
/// decides.
fn quarantine(source: &str, source_ref: &str) -> Disposition {
    Disposition {
        revision_id: String::new(),
        outcome: Outcome::Quarantined,
        reasons: vec![format!(
            "lines of the manifest of the source {source} give the source_ref {source_ref} \
             different digests: neither replaces the other until someone decides"
        )],
        rule_ids: vec![SHARED_SOURCE_REF.to_owned()],
        decided_by: "import".to_owned(),
    }
}
