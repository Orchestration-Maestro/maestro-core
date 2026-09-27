//! Scoped application operations shared by CLI and MCP.

use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    evidence::{Error as EvidenceError, Excerpt},
    scope::ScopeSet,
};
use schemars::JsonSchema;
use serde::Serialize;

/// Schema identifier for collection listing results.
const COLLECTIONS_SCHEMA: &str = "maestro-knowledge-collections/1";
/// Schema identifier for exact retrieval results.
const GET_SCHEMA: &str = "maestro-knowledge-get/1";

/// A privacy-safe failure from a knowledge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnowledgeError {
    /// The request was refused without disclosing inaccessible records.
    Refused {
        /// A stable public error code.
        code: &'static str,
        /// A safe user-facing message.
        message: &'static str,
    },
    /// The local kernel or an integrity check failed.
    Failed {
        /// A stable public error code.
        code: &'static str,
        /// A safe user-facing message, never an error-chain body.
        message: &'static str,
    },
}

/// Visible collections and their pinned current publications.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, Serialize)]
pub(crate) struct CollectionsData {
    /// Result contract.
    pub(crate) schema: &'static str,
    /// Visible collection metadata in ID order.
    pub(crate) collections: Vec<CollectionItem>,
}

/// The deliberately limited collection metadata exposed by this operation.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, Serialize)]
pub(crate) struct CollectionItem {
    /// The collection ID.
    pub(crate) id: String,
    /// The visible collection title.
    pub(crate) title: String,
    /// Its current published generation, or `null` when none is published.
    pub(crate) published_generation: Option<i64>,
}

/// Exact retrieval from one source-scoped chunk membership.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, Serialize)]
pub(crate) struct GetData {
    /// Result contract.
    pub(crate) schema: &'static str,
    /// The selected collection.
    pub(crate) collection: String,
    /// The generation admitted before any alias can move.
    pub(crate) generation: i64,
    /// The exact excerpt returned by the kernel.
    pub(crate) excerpt: GetExcerpt,
}

/// Identity, provenance and exact original source bytes for a chunk.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, Serialize)]
pub(crate) struct GetExcerpt {
    /// The chunk selected.
    pub(crate) chunk_id: String,
    /// The stable source document identity.
    pub(crate) document_id: String,
    /// The exact revision identity.
    pub(crate) revision_id: String,
    /// The section recorded on the chunk, when present.
    pub(crate) section_id: Option<String>,
    /// The source reference recorded with the document.
    pub(crate) source_ref: String,
    /// The source title recorded with the revision, when present.
    pub(crate) title: Option<String>,
    /// The revision version when recorded as text.
    pub(crate) version: Option<String>,
    /// The half-open UTF-8 byte range in the original source.
    pub(crate) span: [usize; 2],
    /// SHA-256 of the exact excerpt bytes.
    pub(crate) digest: String,
    /// Verbatim original source text.
    pub(crate) text: String,
}

/// Result data paired with the effective permissions it used.
pub(crate) struct Scoped<T> {
    /// The operation result, never serialized with the permission snapshot.
    pub(crate) data: T,
    /// The open kernel used for every read and permission refresh.
    pub(crate) kernel: Kernel,
    /// The local scopes that admitted every read in the operation.
    pub(crate) scopes: ScopeSet,
}

/// Lists collections through an injected single-open kernel.
pub(crate) fn collections_with(
    open: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<Scoped<CollectionsData>, KnowledgeError> {
    let mut kernel = open().map_err(|failure| kernel_open_failure(&failure))?;
    let scopes = kernel.scopes.clone();
    let mut collections = Vec::new();
    for collection in kernel
        .database
        .collections(&scopes)
        .map_err(|_| kernel_failure())?
    {
        let generation = kernel
            .database
            .published_generation(&scopes, &collection.id)
            .map_err(|_| kernel_failure())?;
        collections.push(CollectionItem {
            id: collection.id,
            title: collection.title,
            published_generation: generation.map(|generation| generation.id),
        });
    }
    ensure_current_scopes(&mut kernel, &scopes)?;
    Ok(Scoped {
        data: CollectionsData {
            schema: COLLECTIONS_SCHEMA,
            collections,
        },
        kernel,
        scopes,
    })
}

/// Locates and resolves a chunk through one injected kernel, refreshing its scopes between phases.
pub(crate) fn get_with(
    open: impl FnOnce() -> Result<Kernel, Failure>,
    chunk_id: &str,
    collection_id: Option<&str>,
    generation_id: Option<i64>,
) -> Result<Scoped<GetData>, KnowledgeError> {
    let mut kernel = open().map_err(|failure| kernel_open_failure(&failure))?;
    let scopes = kernel.scopes.clone();
    let locations = kernel
        .database
        .chunk_locations(&scopes, chunk_id, collection_id, generation_id)
        .map_err(|_| kernel_failure())?;
    let mut locations = locations.into_iter();
    let Some(location) = locations.next() else {
        return Err(unknown_chunk());
    };
    if locations.next().is_some() {
        return Err(KnowledgeError::Refused {
            code: "ambiguous_chunk",
            message: concat!(
                "chunk matches more than one visible published generation; ",
                "specify collection and generation",
            ),
        });
    }

    ensure_current_scopes(&mut kernel, &scopes)?;
    let excerpt = kernel
        .database
        .resolve(&kernel.scopes, &location.chunk_set_id, chunk_id)
        .map_err(|error| resolve_failure(&error))?;
    ensure_current_scopes(&mut kernel, &scopes)?;

    Ok(Scoped {
        data: GetData {
            schema: GET_SCHEMA,
            collection: location.collection_id,
            generation: location.generation_id,
            excerpt: get_excerpt(chunk_id, location.title, excerpt),
        },
        kernel,
        scopes,
    })
}

/// Maps local kernel construction and refresh failures to safe public errors.
fn kernel_open_failure(failure: &Failure) -> KnowledgeError {
    match failure {
        Failure::Refused(_) => KnowledgeError::Refused {
            code: "invalid_configuration",
            message: "local access configuration is invalid",
        },
        Failure::Failed(_) => kernel_failure(),
    }
}

/// Reloads the local config on the admitted connection and fails closed on change.
pub(crate) fn ensure_current_scopes(
    kernel: &mut Kernel,
    admitted: &ScopeSet,
) -> Result<(), KnowledgeError> {
    kernel
        .refresh_scopes()
        .map_err(|failure| kernel_open_failure(&failure))?;
    if admitted == &kernel.scopes {
        Ok(())
    } else {
        Err(access_changed())
    }
}

/// A scope changed or config stopped granting the admitted snapshot.
fn access_changed() -> KnowledgeError {
    KnowledgeError::Refused {
        code: "access_changed",
        message: "permissions changed during the request; no result was delivered",
    }
}

/// The public refusal shared by unknown and inaccessible chunk IDs.
fn unknown_chunk() -> KnowledgeError {
    KnowledgeError::Refused {
        code: "not_found",
        message: "chunk is unknown or not readable in the selected generation",
    }
}

/// A generic kernel failure that never exposes the backend's source chain.
fn kernel_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "kernel_unavailable",
        message: "the local knowledge store is unavailable",
    }
}

/// Maps exact resolver errors without disclosing source existence or backend details.
fn resolve_failure(error: &EvidenceError) -> KnowledgeError {
    match error {
        EvidenceError::UnknownChunk { .. } => unknown_chunk(),
        EvidenceError::DigestMismatch { .. }
        | EvidenceError::SpanOutOfRange { .. }
        | EvidenceError::SpanOffBoundary { .. } => KnowledgeError::Failed {
            code: "integrity_error",
            message: "the source integrity check failed",
        },
        EvidenceError::Store(_) => kernel_failure(),
    }
}

/// Pairs locator metadata with the kernel's digest-checked source excerpt.
fn get_excerpt(chunk_id: &str, title: Option<String>, excerpt: Excerpt) -> GetExcerpt {
    GetExcerpt {
        chunk_id: chunk_id.to_owned(),
        document_id: excerpt.document_id,
        revision_id: excerpt.revision_id,
        section_id: excerpt.section_id,
        source_ref: excerpt.source_ref.clone(),
        title,
        version: excerpt.version,
        span: [excerpt.span.start, excerpt.span.end],
        digest: format!("sha256:{}", excerpt.digest.as_str()),
        text: excerpt.text,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{KnowledgeError, collections_with, get_with};
    use crate::{failure::Failure, kernel::Kernel};
    use maestro_kernel::{
        chunk_set::{Chunk, NewChunkSet},
        document::{Collection, Document, Revision, RevisionStatus, Source},
        evidence::Span,
        generation::NewGeneration,
        store::Database,
    };
    use serde_json::Map;
    use std::{
        collections::BTreeMap,
        env, fs,
        path::PathBuf,
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    const SOURCE: &str = "exact source text";
    const ACCESS_CHANGED: KnowledgeError = KnowledgeError::Refused {
        code: "access_changed",
        message: "permissions changed during the request; no result was delivered",
    };

    pub(crate) struct Scratch(PathBuf);

    impl Scratch {
        pub(crate) fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = env::temp_dir().join(format!(
                "maestro-knowledge-refresh-{}-{}",
                process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let data = root.join("data");
            let config = root.join("config");
            fs::create_dir_all(&data).expect("create data directory");
            fs::create_dir_all(&config).expect("create config directory");
            fs::write(
                config.join("config.toml"),
                "[access]\nread = ['workspace/default']\n",
            )
            .expect("write initial grant");
            let scratch = Self(root);
            scratch.seed();
            scratch
        }

        fn seed(&self) {
            let database = Database::open_in(&self.data()).expect("open scratch database");
            database
                .record_collection(&Collection {
                    id: "collection".to_owned(),
                    title: "Visible collection".to_owned(),
                    visibility: "private".to_owned(),
                    profiles: BTreeMap::new(),
                })
                .expect("record collection");
            database
                .record_source(&Source {
                    collection_id: "collection".to_owned(),
                    id: "source".to_owned(),
                    kind: "import".to_owned(),
                    transport: None,
                    reference: "source:docs".to_owned(),
                    profiles: BTreeMap::new(),
                })
                .expect("record source");
            database
                .record_document(&Document {
                    id: "document".to_owned(),
                    collection_id: "collection".to_owned(),
                    source_id: "source".to_owned(),
                    source_ref: "source:docs".to_owned(),
                })
                .expect("record document");
            let revision = Revision {
                id: "revision".to_owned(),
                document_id: "document".to_owned(),
                original_digest: database
                    .put(SOURCE.as_bytes(), "text/plain")
                    .expect("store source"),
                canonical_digest: database
                    .put(b"{}", "application/json")
                    .expect("store canonical source"),
                status: RevisionStatus::Valid,
                captured_at: None,
                metadata: Map::new(),
            };
            database
                .record_revision(&revision)
                .expect("record revision");
            database
                .begin_chunk_set(&NewChunkSet {
                    id: "chunk-set",
                    collection_id: "collection",
                    chunk_profile: "structural-500-700/1",
                    counter_contract_id: "test",
                })
                .expect("begin chunk set");
            database
                .record_chunks(
                    "chunk-set",
                    &revision.id,
                    &[Chunk {
                        id: "chunk".to_owned(),
                        revision_id: revision.id.clone(),
                        section_id: Some("section".to_owned()),
                        digest: database
                            .put(b"prepared chunk", "text/plain")
                            .expect("store prepared chunk"),
                        token_count: 1,
                        span: Span {
                            start: 0,
                            end: SOURCE.len(),
                        },
                    }],
                )
                .expect("record chunk");
            let manifest = database
                .put(b"{}", "application/json")
                .expect("store manifest");
            database
                .complete_chunk_set("chunk-set", &manifest)
                .expect("complete chunk set");
            let generation = database
                .create_generation(&NewGeneration {
                    collection_id: "collection".to_owned(),
                    chunk_set_id: "chunk-set".to_owned(),
                    embedding_profile: "embed:test".to_owned(),
                    sparse_profile: "bm25-en-fr/1".to_owned(),
                })
                .expect("create generation");
            database
                .verify_generation(generation.id, 1)
                .expect("verify generation");
            database
                .publish_generation(generation.id)
                .expect("publish generation");
        }

        fn data(&self) -> PathBuf {
            self.0.join("data")
        }

        fn config(&self) -> PathBuf {
            self.0.join("config")
        }

        pub(crate) fn kernel(
            &self,
            revoke_after_refreshes: Option<usize>,
        ) -> Result<Kernel, Failure> {
            let kernel = Kernel::open_at(&self.data(), &self.config())?;
            Ok(if let Some(after) = revoke_after_refreshes {
                kernel.with_test_refresh_hook(after, revoke_access)
            } else {
                kernel
            })
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove scratch database");
        }
    }

    fn revoke_access(kernel: &Kernel) {
        fs::write(
            kernel.config_dir.join("config.toml"),
            "[access]\nread = []\n",
        )
        .expect("revoke local access");
    }

    #[test]
    fn collections_refuses_access_revoked_before_delivery() {
        let scratch = Scratch::new();
        let result = collections_with(|| scratch.kernel(Some(0)));
        assert!(matches!(result, Err(ACCESS_CHANGED)));
    }

    #[test]
    fn get_refuses_access_revoked_before_source_read() {
        let scratch = Scratch::new();
        let result = get_with(
            || scratch.kernel(Some(0)),
            "chunk",
            Some("collection"),
            None,
        );
        assert!(matches!(result, Err(ACCESS_CHANGED)));
    }

    #[test]
    fn get_refuses_access_revoked_after_source_resolution() {
        let scratch = Scratch::new();
        let result = get_with(
            || scratch.kernel(Some(1)),
            "chunk",
            Some("collection"),
            None,
        );
        assert!(matches!(result, Err(ACCESS_CHANGED)));
    }

    #[test]
    fn collections_opens_the_kernel_once() {
        let scratch = Scratch::new();
        let mut opens = 0;
        let result = collections_with(|| {
            opens += 1;
            scratch.kernel(None)
        });
        assert!(result.is_ok());
        assert_eq!(opens, 1);
    }
}
