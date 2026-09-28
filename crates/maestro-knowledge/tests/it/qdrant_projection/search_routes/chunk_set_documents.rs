//! The ladder reads the documents of a chunk set by `source_ref`.

use super::fused_search::searchable_scheduler_kernel;
use maestro_knowledge::search::evidence::ChunkSetDocuments;

#[test]
fn each_document_of_the_set_is_found_by_its_source_ref() {
    let kernel = searchable_scheduler_kernel();
    let documents =
        ChunkSetDocuments::read(&kernel.database, &kernel.scopes, &kernel.chunk_set).unwrap();
    let source_ref = "https://example.org/guides/0";
    let document_id = format!("doc-{}-0", kernel.collection);

    assert_eq!(
        documents.document_id(source_ref),
        Some(document_id.as_str())
    );
    assert_eq!(
        documents.document_of_chunk("chunk-0-0"),
        Some(document_id.as_str())
    );
    assert_eq!(documents.document_of_chunk("chunk-9-0"), None);
    let canonical = documents
        .canonical(&kernel.database, source_ref)
        .unwrap()
        .unwrap();
    assert_eq!(canonical.document_id, document_id);
    assert_eq!(
        documents.revision_id(source_ref),
        Some(canonical.revision_id.as_str())
    );
    assert_eq!(documents.revision_id("https://example.org/guides/9"), None);
    assert!(!canonical.sections.is_empty());
    assert_eq!(documents.document_id("https://example.org/guides/9"), None);
    assert!(
        documents
            .canonical(&kernel.database, "https://example.org/guides/9")
            .unwrap()
            .is_none()
    );
}
