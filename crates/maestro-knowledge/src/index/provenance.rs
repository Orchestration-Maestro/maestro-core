//! What the kernel says of a chunk's revision that the chunk's point
//! carries: its version and source kind, the scopes of every place its
//! content occurs, and the heading path of each of its sections.

use super::error::Error;
use maestro_canonicalization::Section;
use maestro_kernel::{chunk_set::Chunk, scope::ScopeSet, store::Database};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};

/// The workspace the kernel keeps its records in.
const WORKSPACE: &str = "workspace/default";

/// What a chunk's point carries of its revision.
#[derive(Debug)]
pub(super) struct Provenance {
    /// The revision.
    revision: String,
    /// Its `version` metadata, when it is text.
    pub(super) version: Option<String>,
    /// Its `source_kind` metadata, when it is text.
    pub(super) source_kind: Option<String>,
    /// The scope of each source its content occurs in, its own document's
    /// and every occurrence's, with every scope above it, in order: a caller
    /// may read the point when a scope granted to it is among them.
    pub(super) scope_tags: Vec<String>,
    /// The heading path of each section of its canonical document, by
    /// section id.
    sections: HashMap<String, Vec<String>>,
}

impl Provenance {
    /// What the kernel says of the revision of `chunk`, read through
    /// `scopes`.
    ///
    /// # Errors
    ///
    /// [`Error::Records`] and [`Error::Artifacts`] when the kernel fails, and
    /// [`Error::Unreadable`] when the revision or its document is not
    /// recorded, or its canonical document holds no sections to read.
    pub(super) fn read(
        database: &Database,
        scopes: &ScopeSet,
        chunk: &Chunk,
    ) -> Result<Self, Error> {
        let unreadable = |reason: String| Error::Unreadable {
            chunk: chunk.id.clone(),
            reason,
        };
        let revision = database
            .revision(scopes, &chunk.revision_id)
            .map_err(Error::Records)?
            .ok_or_else(|| {
                unreadable(format!(
                    "its revision {} is not recorded",
                    chunk.revision_id
                ))
            })?;
        let document = database
            .document(scopes, &revision.document_id)
            .map_err(Error::Records)?
            .ok_or_else(|| {
                unreadable(format!(
                    "its document {} is not recorded",
                    revision.document_id
                ))
            })?;
        let occurrences = database
            .occurrences(scopes, &revision.id)
            .map_err(Error::Records)?;
        let canonical = database
            .get(&revision.canonical_digest)
            .map_err(Error::Artifacts)?;
        let sectioned: Sectioned = serde_json::from_slice(&canonical).map_err(|error| {
            unreadable(format!(
                "its revision's canonical document has no sections to read: {error}"
            ))
        })?;
        let places = occurrences
            .iter()
            .map(|place| (place.collection_id.as_str(), place.source_id.as_str()))
            .chain([(document.collection_id.as_str(), document.source_id.as_str())]);
        Ok(Self {
            version: text(&revision.metadata, "version"),
            source_kind: text(&revision.metadata, "source_kind"),
            scope_tags: scope_tags(places),
            sections: sectioned
                .sections
                .into_iter()
                .map(|section| (section.section_id, section.heading_path))
                .collect(),
            revision: revision.id,
        })
    }

    /// The heading path of the section of `chunk`, a chunk of this revision;
    /// empty for a chunk without a section.
    ///
    /// # Errors
    ///
    /// [`Error::Unreadable`] when its section is not one of the revision's.
    pub(super) fn section_path(&self, chunk: &Chunk) -> Result<&[String], Error> {
        let Some(section) = &chunk.section_id else {
            return Ok(&[]);
        };
        self.sections
            .get(section)
            .map(Vec::as_slice)
            .ok_or_else(|| Error::Unreadable {
                chunk: chunk.id.clone(),
                reason: format!(
                    "its section {section} is not one of its revision {}'s",
                    self.revision
                ),
            })
    }
}

/// The scope of each of `places`, a collection and a source, and every scope
/// above it: the workspace's, the collection's, then the source's, all in
/// order and each once.
fn scope_tags<'a>(places: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for (collection, source) in places {
        let collection = format!("{WORKSPACE}/collection/{collection}");
        tags.insert(format!("{collection}/source/{source}"));
        tags.insert(collection);
        tags.insert(WORKSPACE.to_owned());
    }
    tags.into_iter().collect()
}

/// The value of `metadata` under `key`, when it is text.
fn text(metadata: &Map<String, Value>, key: &str) -> Option<String> {
    metadata.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A canonical document, as far as its sections: the rest is not read.
#[derive(Deserialize)]
struct Sectioned {
    /// Its heading sections.
    sections: Vec<Section>,
}
