//! What the gate's unit tests share: canonical documents made from Markdown,
//! the revisions a decision reads, and ledgers written a rule a line.

use super::super::{
    checks::{self, Flag},
    ledger::{Candidate, Ledger},
};
use maestro_canonicalization::{
    AssetStatus, CanonicalDocument, CanonicalizeInput, SourceMetadata, ValidationStatus,
    canonicalize,
};
use maestro_kernel::{
    artifact::Digest,
    document::{Outcome, Revision, RevisionStatus},
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The source reference of the documents the tests make.
pub(super) const PAGE: &str = "https://example.org/page";

/// `markdown` canonicalized from [`PAGE`], titled `Page`: its language,
/// extraction and access policy unknown, as an import's usually are.
pub(super) fn document(markdown: &str) -> CanonicalDocument {
    canonical(markdown, Some(PAGE), Some("Page"), BTreeMap::new())
}

/// `markdown` canonicalized from `source`, titled `title`, with the asset
/// observations `assets`.
pub(super) fn canonical(
    markdown: &str,
    source: Option<&str>,
    title: Option<&str>,
    assets: BTreeMap<String, AssetStatus>,
) -> CanonicalDocument {
    let mut input = CanonicalizeInput::new(markdown, "notes/page.md");
    input.metadata = SourceMetadata {
        source_reference: source.map(str::to_owned),
        title: title.map(str::to_owned),
        ..SourceMetadata::default()
    };
    input.assets = assets;
    canonicalize(input).unwrap()
}

/// What the automatic checks flag in `document`, canonicalized from
/// `markdown`.
pub(super) fn flags_of(document: &CanonicalDocument, markdown: &str) -> Vec<Flag> {
    checks::run(document, markdown)
}

/// What the automatic checks flag in the document of `markdown`.
pub(super) fn flags(markdown: &str) -> Vec<Flag> {
    flags_of(&document(markdown), markdown)
}

/// The rule IDs the automatic checks flag in the document of `markdown`.
pub(super) fn rules(markdown: &str) -> Vec<&'static str> {
    flags(markdown).iter().map(|flag| flag.rule).collect()
}

/// The reason of the one flag of `rule` in the document of `markdown`.
pub(super) fn reason(markdown: &str, rule: &str) -> String {
    flag(&flags(markdown), rule).reason.clone()
}

/// The one flag of `rule` among `flags`.
pub(super) fn flag<'a>(flags: &'a [Flag], rule: &str) -> &'a Flag {
    let mut found = flags.iter().filter(|flag| flag.rule == rule);
    let (Some(flag), None) = (found.next(), found.next()) else {
        panic!("{rule} is not flagged once in {flags:#?}");
    };
    flag
}

/// The revision of `document`, whose manifest line gave it `metadata`.
pub(super) fn revision(document: &CanonicalDocument, metadata: &Value) -> Revision {
    let Value::Object(metadata) = metadata.clone() else {
        panic!("{metadata} is not an object");
    };
    Revision {
        id: document.revision_id.clone(),
        document_id: document.document_id.clone(),
        original_digest: Digest::of(document.revision_id.as_bytes()),
        canonical_digest: Digest::of(document.document_id.as_bytes()),
        status: match document.validation_status {
            ValidationStatus::Failed => RevisionStatus::Failed,
            ValidationStatus::Valid => RevisionStatus::Valid,
            ValidationStatus::ValidWithWarnings => RevisionStatus::ValidWithWarnings,
        },
        captured_at: None,
        metadata: Map::from_iter(metadata),
    }
}

/// The metadata of a guide of the set `notes`, release 1.0.
pub(super) fn guide() -> Value {
    json!({ "title": "Page", "source_kind": "guide", "set": "notes", "version": "1.0" })
}

/// The candidate `revision`, from [`PAGE`].
pub(super) fn candidate(revision: &Revision) -> Candidate<'_> {
    Candidate::new(revision, PAGE)
}

/// The ledger of `rules`, each a JSON rule of which `schema`, `reason`,
/// `decided_by` and `reversal` are filled in when absent.
pub(super) fn ledger(rules: &[Value]) -> Ledger {
    rules
        .iter()
        .map(|rule| {
            let mut rule = rule.clone();
            let fields = rule.as_object_mut().unwrap();
            for (key, value) in [
                ("schema", "maestro-quality-ledger/1"),
                ("reason", "the owner decided so"),
                ("decided_by", "owner"),
                ("reversal", "remove this rule"),
            ] {
                fields
                    .entry(key)
                    .or_insert_with(|| Value::from(value.to_owned()));
            }
            rule.to_string() + "\n"
        })
        .collect::<String>()
        .parse()
        .unwrap()
}

/// The outcomes, from the one that lets a revision be indexed most freely.
pub(super) const OUTCOMES: [Outcome; 5] = [
    Outcome::Accepted,
    Outcome::AcceptedWithWarnings,
    Outcome::NeedsReextraction,
    Outcome::Quarantined,
    Outcome::Excluded,
];
