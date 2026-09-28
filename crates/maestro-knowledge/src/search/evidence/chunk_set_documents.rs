//! The documents of a chunk set by `source_ref` and by chunk: the ladder
//! resolves its suite's expected sections in their canonical documents, names
//! the document of each citation by its `source_ref`, and the document of each
//! ranked chunk by the chunk's ID.

use super::section_reader::{
    SectionReadError, map_chunk_set_error, map_document_error, read_canonical_document,
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    document::{Document, Revision},
    scope::ScopeSet,
    store::Database,
};
use std::collections::{BTreeMap, BTreeSet};

/// The documents with a chunk in one chunk set, and their revisions there.
#[derive(Debug, Clone)]
pub struct ChunkSetDocuments {
    /// Each document and its revision in the set, by `source_ref`.
    by_source_ref: BTreeMap<String, (Document, Revision)>,
    /// The ID of each chunk's document, by the chunk's ID.
    by_chunk: BTreeMap<String, String>,
}

impl ChunkSetDocuments {
    /// Every document with a chunk in `chunk_set_id` whose source `scopes`
    /// covers.
    ///
    /// # Errors
    ///
    /// [`SectionReadError::Store`] when the kernel cannot be read, and
    /// [`SectionReadError::Integrity`] for a chunk whose revision or document
    /// is not recorded.
    pub fn read(
        database: &Database,
        scopes: &ScopeSet,
        chunk_set_id: &str,
    ) -> Result<Self, SectionReadError> {
        let chunks = database
            .chunks(scopes, chunk_set_id)
            .map_err(map_chunk_set_error)?;
        let revisions: BTreeSet<&str> = chunks
            .iter()
            .map(|chunk| chunk.revision_id.as_str())
            .collect();
        let mut by_source_ref = BTreeMap::new();
        let mut documents = BTreeMap::new();
        for revision_id in revisions {
            let revision = database
                .revision(scopes, revision_id)
                .map_err(map_document_error)?
                .ok_or(SectionReadError::Integrity)?;
            let document = database
                .document(scopes, &revision.document_id)
                .map_err(map_document_error)?
                .ok_or(SectionReadError::Integrity)?;
            documents.insert(revision_id, document.id.clone());
            by_source_ref.insert(document.source_ref.clone(), (document, revision));
        }
        let by_chunk = chunks
            .iter()
            .filter_map(|chunk| {
                let document = documents.get(chunk.revision_id.as_str())?;
                Some((chunk.id.clone(), document.clone()))
            })
            .collect();
        Ok(Self {
            by_source_ref,
            by_chunk,
        })
    }

    /// The ID of the document of the chunk `chunk_id`, if the set holds it.
    #[must_use]
    pub fn document_of_chunk(&self, chunk_id: &str) -> Option<&str> {
        self.by_chunk.get(chunk_id).map(String::as_str)
    }

    /// The ID of the document of `source_ref`, if the set holds one.
    #[must_use]
    pub fn document_id(&self, source_ref: &str) -> Option<&str> {
        self.by_source_ref
            .get(source_ref)
            .map(|(document, _)| document.id.as_str())
    }

    /// The canonical document of `source_ref` in the set, if it holds one,
    /// read from `database` and checked against its revision.
    ///
    /// # Errors
    ///
    /// [`SectionReadError::Integrity`] for a canonical document that is
    /// missing or does not match its revision, and
    /// [`SectionReadError::Store`] when the kernel cannot be read.
    pub fn canonical(
        &self,
        database: &Database,
        source_ref: &str,
    ) -> Result<Option<CanonicalDocument>, SectionReadError> {
        self.by_source_ref
            .get(source_ref)
            .map(|(document, revision)| read_canonical_document(database, revision, document))
            .transpose()
    }
}
