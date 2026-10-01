//! What the kernel says of a chunk's revision that the chunk's point
//! carries: its version and source kind, the scope of its owning document,
//! and the heading path of each of its sections.

use super::error::Error;
use maestro_canonicalization::Section;
use maestro_kernel::{
    chunk_set::Chunk,
    scope::{ScopeSet, WORKSPACE, collection_path, source_path},
    store::Database,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// What a chunk's point carries of its revision.
#[derive(Debug)]
pub(super) struct Provenance {
    /// The revision.
    revision: String,
    /// Its `version` metadata, when it is text.
    pub(super) version: Option<String>,
    /// Its `source_kind` metadata, when it is text.
    pub(super) source_kind: Option<String>,
    /// The scope of its owning document's source and every scope above it:
    /// a caller may read the point when a scope granted to it is among them.
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
        let canonical = database
            .get(&revision.canonical_digest)
            .map_err(Error::Artifacts)?;
        let sectioned: Sectioned = serde_json::from_slice(&canonical).map_err(|error| {
            unreadable(format!(
                "its revision's canonical document has no sections to read: {error}"
            ))
        })?;
        Ok(Self {
            version: text(&revision.metadata, "version"),
            source_kind: text(&revision.metadata, "source_kind"),
            scope_tags: scope_tags(&document.collection_id, &document.source_id),
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

/// The scope of the owning source and every scope above it, in order.
fn scope_tags(collection: &str, source: &str) -> Vec<String> {
    vec![
        WORKSPACE.to_owned(),
        collection_path(collection),
        source_path(collection, source),
    ]
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
