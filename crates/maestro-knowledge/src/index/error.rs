//! Why a publication stopped: before any work, for a card or a chunk set it
//! cannot build from; part way, for an embedder or a Qdrant that failed, a
//! batch whose vectors were refused, a kernel that failed or a caller that
//! stopped it, all of which leave the generation building for a rerun to
//! resume; or at a check the generation's collection failed, which fails
//! the generation for good.

use super::{dense::Failure, qdrant::QdrantError};
use crate::prepare;
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{self, ChunkSetState},
    document,
    gateway::Role,
    generation, retrieval, store,
};
use std::{error, fmt};

/// Why a publication stopped.
#[derive(Debug)]
pub enum Error {
    /// The card is not an embedder's: nothing is published.
    NotAnEmbedder {
        /// The card's digest.
        card: Digest,
        /// The role it records.
        role: Role,
    },
    /// No chunk set of this id is recorded that the caller's scopes cover:
    /// nothing is published.
    UnknownChunkSet(String),
    /// The chunk set was counted for another v2 card; no generation or
    /// model/Qdrant call is started.
    CounterContractMismatch {
        /// The candidate card's required counter contract.
        expected: String,
        /// The chunk set's recorded counter contract.
        actual: String,
    },
    /// The chunk set is not complete, so nothing reads it as a whole: nothing
    /// is published.
    Incomplete {
        /// The chunk set's id.
        chunk_set: String,
        /// Its state.
        state: ChunkSetState,
    },
    /// The batch that starts at this chunk of the set, counted from 0, has no
    /// vectors: the embedder refused or timed out, or its vectors were
    /// refused. Nothing of the batch is written, and the generation stays
    /// building.
    Embedding {
        /// The place of the batch's first chunk in the chunk set.
        at: u64,
        /// Why.
        failure: Failure,
    },
    /// What the kernel holds for this chunk cannot be read as a point needs
    /// it: the generation stays building.
    Unreadable {
        /// The chunk's id.
        chunk: String,
        /// Why.
        reason: String,
    },
    /// Qdrant failed: the generation stays as it was, building or verified.
    Qdrant(QdrantError),
    /// A published generation's collection is missing, so it cannot be
    /// reported as published.
    MissingCollection(String),
    /// The published generation changed from the frozen expected pointer.
    PublishedChanged {
        /// What was published at admission, or none.
        expected: Option<i64>,
        /// What is published now, or none.
        found: Option<i64>,
    },
    /// A recovery target does not match its job's frozen tuple or watermark.
    RecoveryTarget {
        /// The target generation's ID.
        generation: i64,
    },
    /// More than one generation could be the unjournaled recovery target.
    AmbiguousRecoveryTarget {
        /// All candidate generation IDs.
        generations: Vec<i64>,
    },
    /// The collection alias points at a generation unrelated to this job.
    UnrelatedAlias {
        /// The collection alias.
        alias: String,
        /// The unexpected alias target.
        found: String,
    },
    /// The generation's collection failed a check, which failed the
    /// generation for good: the alias stays where it was.
    Unverified {
        /// The generation's id.
        generation: i64,
        /// The check it failed.
        reason: Unverified,
    },
    /// The caller's observer stopped the publication, as a job does once its
    /// lease is lost: the generation stays building.
    Stopped,
    /// The kernel failed to create or move the generation.
    Generation(generation::Error),
    /// The kernel failed to read the chunk set or its chunks.
    ChunkSet(chunk_set::Error),
    /// The kernel failed to read a chunk's revision, document or
    /// occurrences.
    Records(document::Error),
    /// The kernel failed to read an artifact.
    Artifacts(store::Error),
    /// The preparation manifest could not supply search members.
    Preparation(prepare::Error),
    /// The kernel's derived-search storage failed.
    Search(retrieval::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnEmbedder { card, role } => write!(
                formatter,
                "the model card sha256:{} is a {role}'s: a generation's dense vectors need an \
                 embedder's",
                card.as_str()
            ),
            Self::UnknownChunkSet(id) => write!(
                formatter,
                "no chunk set {id} is recorded that the caller can read"
            ),
            Self::Incomplete { chunk_set, state } => write!(
                formatter,
                "the chunk set {chunk_set} is {state}: only a complete chunk set is published"
            ),
            Self::CounterContractMismatch { expected, actual } => write!(
                formatter,
                "the v2 card requires counter {expected}, but the chunk set records {actual}"
            ),
            Self::Embedding { at, failure } => write!(
                formatter,
                "the batch from chunk {at} has no vectors, and the generation stays building: \
                 {failure}"
            ),
            Self::Unreadable { chunk, reason } => write!(
                formatter,
                "the chunk {chunk} cannot be read as its point needs: {reason}"
            ),
            Self::Qdrant(error) => write!(formatter, "Qdrant failed: {error}"),
            Self::MissingCollection(collection) => write!(
                formatter,
                "the published generation's Qdrant collection {collection} is missing"
            ),
            Self::PublishedChanged { expected, found } => write!(
                formatter,
                "published generation changed from {expected:?} to {found:?} during replacement"
            ),
            Self::RecoveryTarget { generation } => write!(
                formatter,
                "generation {generation} does not match the frozen recovery target"
            ),
            Self::AmbiguousRecoveryTarget { generations } => write!(
                formatter,
                "multiple generations could be the recovery target: {generations:?}"
            ),
            Self::UnrelatedAlias { alias, found } => write!(
                formatter,
                "alias {alias} points to unrelated collection {found}"
            ),
            Self::Unverified { generation, reason } => write!(
                formatter,
                "generation {generation} failed its check, and the alias stays where it was: \
                 {reason}"
            ),
            Self::Stopped => formatter.write_str(
                "the publication was stopped by its caller: its generation stays building, and \
                 a rerun resumes it",
            ),
            Self::Generation(error) => write!(formatter, "the generation failed: {error}"),
            Self::ChunkSet(error) => write!(formatter, "the chunk set failed: {error}"),
            Self::Records(error) => write!(formatter, "the kernel's records failed: {error}"),
            Self::Artifacts(error) => write!(formatter, "the artifact store failed: {error}"),
            Self::Preparation(error) => {
                write!(formatter, "the preparation manifest failed: {error}")
            }
            Self::Search(error) => write!(formatter, "the search projection failed: {error}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Embedding { failure, .. } => Some(failure),
            Self::Qdrant(error) => Some(error),
            Self::Generation(error) => Some(error),
            Self::ChunkSet(error) => Some(error),
            Self::Records(error) => Some(error),
            Self::Artifacts(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::Search(error) => Some(error),
            Self::NotAnEmbedder { .. }
            | Self::UnknownChunkSet(_)
            | Self::Incomplete { .. }
            | Self::CounterContractMismatch { .. }
            | Self::Unreadable { .. }
            | Self::MissingCollection(_)
            | Self::PublishedChanged { .. }
            | Self::RecoveryTarget { .. }
            | Self::AmbiguousRecoveryTarget { .. }
            | Self::UnrelatedAlias { .. }
            | Self::Unverified { .. }
            | Self::Stopped => None,
        }
    }
}

/// The check a generation's collection failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unverified {
    /// Its collection in Qdrant is missing.
    NoCollection,
    /// Its vectors are not those the generation's profiles call for: a dense
    /// vector `dense` of the card's dimensions compared by cosine, and a
    /// sparse vector `bm25` weighted by IDF.
    Vectors {
        /// The card's dimensions.
        dimensions: u64,
        /// The vectors the collection has, as Qdrant reports them.
        found: String,
    },
    /// It holds another number of points than the chunk set has chunks.
    Count {
        /// The chunks of the set.
        expected: u64,
        /// The points of the collection.
        found: u64,
    },
    /// The point of this chunk is missing.
    Missing {
        /// The chunk's id.
        chunk: String,
    },
    /// Its search payload, readiness or supporting index is inconsistent.
    Search {
        /// The deterministic projection mismatch.
        reason: String,
    },
}

impl fmt::Display for Unverified {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCollection => formatter.write_str("its collection in Qdrant is missing"),
            Self::Vectors { dimensions, found } => write!(
                formatter,
                "its collection has {found}, where the generation needs a dense vector `dense` \
                 of {dimensions} dimensions compared by cosine and a sparse vector `bm25` \
                 weighted by IDF"
            ),
            Self::Count { expected, found } => write!(
                formatter,
                "its collection holds {found} points, where its chunk set has {expected} chunks"
            ),
            Self::Missing { chunk } => write!(
                formatter,
                "its collection holds no point of the chunk {chunk}"
            ),
            Self::Search { reason } => {
                write!(formatter, "its search projection is invalid: {reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Unverified};
    use crate::prepare;
    use maestro_kernel::retrieval;
    use std::error::Error as StdError;

    #[test]
    fn recovery_failures_keep_diagnostics_and_sources() {
        for error in [
            Error::CounterContractMismatch {
                expected: "expected".to_owned(),
                actual: "actual".to_owned(),
            },
            Error::MissingCollection("missing".to_owned()),
            Error::PublishedChanged {
                expected: Some(1),
                found: Some(2),
            },
            Error::RecoveryTarget { generation: 3 },
            Error::AmbiguousRecoveryTarget {
                generations: vec![3, 4],
            },
            Error::UnrelatedAlias {
                alias: "alias".to_owned(),
                found: "collection".to_owned(),
            },
        ] {
            assert!(!error.to_string().is_empty());
            assert!(StdError::source(&error).is_none());
        }

        let preparation = Error::Preparation(prepare::Error::Stopped);
        assert!(
            preparation
                .to_string()
                .contains("preparation manifest failed")
        );
        assert!(StdError::source(&preparation).is_some());

        let search = Error::Search(retrieval::Error::InputConflict);
        assert!(search.to_string().contains("search projection failed"));
        assert!(StdError::source(&search).is_some());
    }

    #[test]
    fn each_unverified_finding_has_a_diagnostic() {
        for finding in [
            Unverified::NoCollection,
            Unverified::Vectors {
                dimensions: 3,
                found: "wrong vectors".to_owned(),
            },
            Unverified::Count {
                expected: 1,
                found: 2,
            },
            Unverified::Missing {
                chunk: "chunk".to_owned(),
            },
            Unverified::Search {
                reason: "missing payload".to_owned(),
            },
        ] {
            assert!(!finding.to_string().is_empty());
        }
    }
}
