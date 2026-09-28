//! Scoped application operations shared by CLI and MCP.

use super::super::{
    output::RESPONSE_LIMIT_BYTES,
    requests::{GetRequest, GetSelector},
};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    evidence::{ChunkLocation, Error as EvidenceError, Excerpt, SectionLocation},
    scope::ScopeSet,
};
use maestro_knowledge::search::evidence::{SectionExcerpt, SectionReadError, read_section};
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

/// Identity, provenance and exact original source bytes for a chunk or section.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, Serialize)]
pub(crate) struct GetExcerpt {
    /// The chunk selected, absent when the request selected a section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chunk_id: Option<String>,
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
    /// The canonical section path for a section request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) section_path: Option<Vec<String>>,
    /// The half-open UTF-8 byte range in the original source.
    pub(crate) span: [usize; 2],
    /// SHA-256 of the exact excerpt bytes.
    pub(crate) digest: String,
    /// Verbatim original source text.
    pub(crate) text: String,
}

/// Generation metadata for the exact identifier's indexed membership.
struct GetLocation {
    /// The selected published collection.
    collection_id: String,
    /// The exact published generation selected for the read.
    generation_id: i64,
    /// The chunk set indexed by that generation.
    chunk_set_id: String,
    /// The indexed chunk title, absent for section lookup.
    title: Option<String>,
}

impl From<ChunkLocation> for GetLocation {
    fn from(location: ChunkLocation) -> Self {
        Self {
            collection_id: location.collection_id,
            generation_id: location.generation_id,
            chunk_set_id: location.chunk_set_id,
            title: location.title,
        }
    }
}

impl From<SectionLocation> for GetLocation {
    fn from(location: SectionLocation) -> Self {
        Self {
            collection_id: location.collection_id,
            generation_id: location.generation_id,
            chunk_set_id: location.chunk_set_id,
            title: None,
        }
    }
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

/// Locates and resolves one exact identifier through a single injected kernel.
pub(crate) fn get_with(
    open: impl FnOnce() -> Result<Kernel, Failure>,
    request: &GetRequest,
) -> Result<Scoped<GetData>, KnowledgeError> {
    let selector = request.selector().ok_or(KnowledgeError::Refused {
        code: "invalid_arguments",
        message: "exactly one of chunk_id and section_id is required",
    })?;
    let mut kernel = open().map_err(|failure| kernel_open_failure(&failure))?;
    let scopes = kernel.scopes.clone();
    let locations = match selector {
        GetSelector::Chunk(id) => kernel
            .database
            .chunk_locations(
                &scopes,
                id,
                request.collection.as_deref(),
                request.generation,
            )
            .map_err(|_| kernel_failure())?
            .into_iter()
            .map(GetLocation::from)
            .collect::<Vec<_>>(),
        GetSelector::Section(id) => kernel
            .database
            .section_locations(
                &scopes,
                id,
                request.collection.as_deref(),
                request.generation,
            )
            .map_err(|_| kernel_failure())?
            .into_iter()
            .map(GetLocation::from)
            .collect::<Vec<_>>(),
    };
    let mut locations = locations.into_iter();
    let Some(location) = locations.next() else {
        return Err(match selector {
            GetSelector::Chunk(_) => unknown_chunk(),
            GetSelector::Section(_) => unknown_section(),
        });
    };
    if locations.next().is_some() {
        return Err(match selector {
            GetSelector::Chunk(_) => KnowledgeError::Refused {
                code: "ambiguous_chunk",
                message: concat!(
                    "chunk matches more than one visible published generation; ",
                    "specify collection and generation",
                ),
            },
            GetSelector::Section(_) => KnowledgeError::Refused {
                code: "ambiguous_section",
                message: concat!(
                    "section matches more than one visible published generation; ",
                    "specify collection and generation",
                ),
            },
        });
    }

    ensure_current_scopes(&mut kernel, &scopes)?;
    let excerpt = match selector {
        GetSelector::Chunk(chunk_id) => {
            let excerpt = kernel
                .database
                .resolve(&kernel.scopes, &location.chunk_set_id, chunk_id)
                .map_err(|error| resolve_failure(&error))?;
            get_excerpt(chunk_id, location.title, excerpt)
        }
        GetSelector::Section(section_id) => {
            let result = read_section(
                &kernel.database,
                &kernel.scopes,
                &location.chunk_set_id,
                section_id,
                RESPONSE_LIMIT_BYTES,
            );
            let section = match result {
                Ok(section) => section,
                Err(error) => {
                    ensure_current_scopes(&mut kernel, &scopes)?;
                    return Err(section_read_failure(&error));
                }
            };
            get_section_excerpt(section)
        }
    };
    ensure_current_scopes(&mut kernel, &scopes)?;

    Ok(Scoped {
        data: GetData {
            schema: GET_SCHEMA,
            collection: location.collection_id,
            generation: location.generation_id,
            excerpt,
        },
        kernel,
        scopes,
    })
}

/// Maps local kernel construction and refresh failures to safe public errors.
pub(crate) fn kernel_open_failure(failure: &Failure) -> KnowledgeError {
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

/// The public refusal shared by unknown and inaccessible section IDs.
fn unknown_section() -> KnowledgeError {
    KnowledgeError::Refused {
        code: "not_found",
        message: "section is unknown or not readable in the selected generation",
    }
}

/// A generic kernel failure that never exposes the backend's source chain.
pub(crate) fn kernel_failure() -> KnowledgeError {
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

/// Maps authoritative section-reader errors without exposing backend details.
pub(super) fn section_read_failure(error: &SectionReadError) -> KnowledgeError {
    match error {
        SectionReadError::NotFound => unknown_section(),
        SectionReadError::Ambiguous => KnowledgeError::Refused {
            code: "ambiguous_section",
            message: "section identity is ambiguous in this chunk set",
        },
        SectionReadError::TooLarge { .. } => KnowledgeError::Refused {
            code: "response_too_large",
            message: "the exact excerpt exceeds the response limit",
        },
        SectionReadError::Integrity => KnowledgeError::Failed {
            code: "integrity_error",
            message: "the source integrity check failed",
        },
        SectionReadError::Store(_) => kernel_failure(),
    }
}

/// Pairs locator metadata with the kernel's digest-checked source excerpt.
fn get_excerpt(chunk_id: &str, title: Option<String>, excerpt: Excerpt) -> GetExcerpt {
    GetExcerpt {
        chunk_id: Some(chunk_id.to_owned()),
        document_id: excerpt.document_id,
        revision_id: excerpt.revision_id,
        section_id: excerpt.section_id,
        source_ref: excerpt.source_ref,
        title,
        version: excerpt.version,
        section_path: None,
        span: [excerpt.span.start, excerpt.span.end],
        digest: format!("sha256:{}", excerpt.digest.as_str()),
        text: excerpt.text,
    }
}

/// Keeps section metadata from the authoritative T032 reader intact.
fn get_section_excerpt(excerpt: SectionExcerpt) -> GetExcerpt {
    GetExcerpt {
        chunk_id: None,
        document_id: excerpt.document_id,
        revision_id: excerpt.revision_id,
        section_id: Some(excerpt.section_id),
        source_ref: excerpt.source_ref,
        title: Some(excerpt.title),
        version: excerpt.version,
        section_path: Some(excerpt.section_path),
        span: excerpt.span,
        digest: format!("sha256:{}", excerpt.digest.as_str()),
        text: excerpt.text,
    }
}
