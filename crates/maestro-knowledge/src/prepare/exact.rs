//! Exact duplicates (docs/architecture/01 §6): eligible revisions with the
//! same original bytes and the same canonical content, as
//! maestro-canonicalization's `group_exact` fingerprints them.
//!
//! Each revision is read and fingerprinted alone, which replays its
//! canonical document against its original first, so the memory held is one
//! document's. Revisions whose two SHA-256 fingerprints, of the original
//! bytes and of the canonical content, both agree form a group: the content
//! identity the artifact store relies on too. So a collision of both digests
//! is the one case a group does not check, since its members are never read
//! together and compared whole. A group is prepared once, as its member with
//! the smallest revision ID, so the choice never depends on the order of
//! import, and every member's place is an occurrence of that revision.

use super::{
    failure::Error,
    near::{Shingles, Vocabulary, words},
    report::Refusal,
};
use maestro_canonicalization::{
    CanonicalDocument, DedupInput, DedupScope, RevisionKey, WarningPolicy, group_exact,
};
use maestro_kernel::{
    document::{Document, Occurrence, Revision},
    scope::ScopeSet,
    store::Database,
};
use std::collections::BTreeMap;

/// A revision with what preparing it reads: its document's record, its
/// canonical document and its original Markdown.
#[derive(Debug)]
pub(super) struct Loaded {
    /// The revision.
    pub(super) revision: Revision,
    /// Its document.
    pub(super) document: Document,
    /// Its canonical document.
    pub(super) canonical: CanonicalDocument,
    /// Its original Markdown.
    pub(super) markdown: String,
}

impl Loaded {
    /// The refusal of this revision for `reason`.
    pub(super) fn refusal(&self, reason: String) -> Refusal {
        refusal(&self.revision, &self.document, reason)
    }
}

/// An eligible revision as the grouping sees it: where it occurs, its two
/// fingerprints, and its shingles.
#[derive(Debug)]
pub(super) struct Fingerprint {
    /// The revision.
    pub(super) revision: Revision,
    /// Its document.
    pub(super) document: Document,
    /// The SHA-256 of its original bytes and of its canonical content.
    hashes: (String, String),
    /// Its shingles.
    pub(super) shingles: Shingles,
}

/// The kernel a preparation reads and records in, through the caller's
/// scopes, for one collection.
#[derive(Debug, Clone, Copy)]
pub(super) struct Kernel<'a> {
    /// The kernel.
    pub(super) database: &'a Database,
    /// The caller's scopes.
    pub(super) scopes: &'a ScopeSet,
    /// The collection.
    pub(super) collection: &'a str,
}

impl Kernel<'_> {
    /// The explicit scope canonicalization groups and chunks in: the
    /// kernel's workspace and the collection, which authorize the revisions
    /// of `loaded`, and no other.
    pub(super) fn dedup_scope(&self, loaded: &[&Loaded]) -> DedupScope {
        DedupScope {
            tenant_id: "workspace/default".to_owned(),
            workspace_id: format!("collection/{}", self.collection),
            authorized_revisions: loaded
                .iter()
                .map(|loaded| RevisionKey {
                    document_id: loaded.canonical.document_id.clone(),
                    revision_id: loaded.canonical.revision_id.clone(),
                })
                .collect(),
        }
    }

    /// The document of `revision`, which a caller who reads the collection
    /// reads.
    ///
    /// # Errors
    ///
    /// [`Error::Records`] when the kernel cannot be read, and
    /// [`Error::NotVisible`] when the caller's scopes do not read it.
    pub(super) fn document_of(&self, revision: &Revision) -> Result<Document, Error> {
        self.database
            .document(self.scopes, &revision.document_id)
            .map_err(Error::Records)?
            .ok_or_else(|| {
                Error::NotVisible(format!("workspace/default/collection/{}", self.collection))
            })
    }

    /// `revision` with its document and its two documents, or why it cannot
    /// be prepared: a canonical artifact that is no canonical document, or
    /// another revision's, or an original that is not UTF-8.
    ///
    /// # Errors
    ///
    /// [`Error::Records`] and [`Error::Artifacts`] when the kernel cannot be
    /// read.
    pub(super) fn load(&self, revision: &Revision) -> Result<Result<Loaded, Refusal>, Error> {
        let document = self.document_of(revision)?;
        let canonical = self
            .database
            .get(&revision.canonical_digest)
            .map_err(Error::Artifacts)?;
        let original = self
            .database
            .get(&revision.original_digest)
            .map_err(Error::Artifacts)?;
        let parsed = serde_json::from_slice::<CanonicalDocument>(&canonical)
            .map_err(|error| format!("its canonical document cannot be read: {error}"))
            .and_then(|canonical| {
                if (&canonical.revision_id, &canonical.document_id)
                    != (&revision.id, &revision.document_id)
                {
                    return Err(format!(
                        "its canonical document is the revision {} of the document {}",
                        canonical.revision_id, canonical.document_id
                    ));
                }
                let markdown = String::from_utf8(original)
                    .map_err(|error| format!("its original is not UTF-8: {error}"))?;
                Ok((canonical, markdown))
            });
        Ok(match parsed {
            Ok((canonical, markdown)) => Ok(Loaded {
                revision: revision.clone(),
                document,
                canonical,
                markdown,
            }),
            Err(reason) => Err(refusal(revision, &document, reason)),
        })
    }

    /// The fingerprint of `revision`, read and replayed alone, or why it
    /// cannot be prepared.
    ///
    /// # Errors
    ///
    /// As [`Kernel::load`].
    pub(super) fn fingerprint(
        &self,
        revision: &Revision,
        vocabulary: &mut Vocabulary,
    ) -> Result<Result<Fingerprint, Refusal>, Error> {
        let loaded = match self.load(revision)? {
            Ok(loaded) => loaded,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let scope = self.dedup_scope(&[&loaded]);
        let input = DedupInput {
            document: &loaded.canonical,
            markdown: &loaded.markdown,
        };
        let grouped = group_exact(&scope, &[input], WarningPolicy::Preserve);
        let hashes = match grouped.map(|grouped| grouped.occurrences) {
            Ok(occurrences) => occurrences.first().map(|occurrence| {
                (
                    occurrence.original_hash.clone(),
                    occurrence.canonical_hash.clone(),
                )
            }),
            Err(error) => return Ok(Err(loaded.refusal(error.0))),
        };
        let Some(hashes) = hashes else {
            let reason = "canonicalization gave it no fingerprint".to_owned();
            return Ok(Err(loaded.refusal(reason)));
        };
        let shingles = Shingles::of(&words(&loaded.canonical), vocabulary);
        Ok(Ok(Fingerprint {
            hashes,
            revision: loaded.revision,
            document: loaded.document,
            shingles,
        }))
    }
}

/// The groups of `fingerprints` whose two fingerprints both agree, each in
/// revision order, in the order of their first member.
pub(super) fn exact_groups(fingerprints: Vec<Fingerprint>) -> Vec<Vec<Fingerprint>> {
    let mut by_hashes: BTreeMap<(String, String), Vec<Fingerprint>> = BTreeMap::new();
    for fingerprint in fingerprints {
        by_hashes
            .entry(fingerprint.hashes.clone())
            .or_default()
            .push(fingerprint);
    }
    let mut groups: Vec<Vec<Fingerprint>> = by_hashes.into_values().collect();
    for members in &mut groups {
        members.sort_by(|first, second| first.revision.id.cmp(&second.revision.id));
    }
    groups.sort_by(|first, second| {
        let first = first.first().map(|member| &member.revision.id);
        first.cmp(&second.first().map(|member| &member.revision.id))
    });
    groups
}

/// Every place the content of the group of `members` occurs, as occurrences
/// of the member prepared for it, the first.
pub(super) fn occurrences(collection: &str, members: &[Fingerprint]) -> Vec<Occurrence> {
    let Some(chunked) = members.first() else {
        return Vec::new();
    };
    members
        .iter()
        .map(|member| Occurrence {
            revision_id: chunked.revision.id.clone(),
            collection_id: collection.to_owned(),
            source_id: member.document.source_id.clone(),
            source_ref: member.document.source_ref.clone(),
        })
        .collect()
}

/// The refusal of `revision` of `document` for `reason`.
fn refusal(revision: &Revision, document: &Document, reason: String) -> Refusal {
    Refusal {
        revision: revision.id.clone(),
        document: document.id.clone(),
        source_ref: document.source_ref.clone(),
        reason,
    }
}
