//! The documents and the authorized scope each grouping test starts from.
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, DedupScope, RevisionKey, canonicalize,
};

/// The canonical document of `text` under the source identity `identity`.
pub(super) fn document(text: &str, identity: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(text, identity)).unwrap()
}

/// A scope of tenant `tenant-a` and workspace `workspace-a` that authorizes
/// exactly these documents' revisions.
pub(super) fn scope(documents: &[&CanonicalDocument]) -> DedupScope {
    DedupScope {
        tenant_id: "tenant-a".into(),
        workspace_id: "workspace-a".into(),
        authorized_revisions: documents
            .iter()
            .map(|doc| RevisionKey {
                document_id: doc.document_id.clone(),
                revision_id: doc.revision_id.clone(),
            })
            .collect(),
    }
}
