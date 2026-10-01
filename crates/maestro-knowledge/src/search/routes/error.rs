//! Typed failures for the admission and diagnostic routes.

use maestro_kernel::{generation::Error as GenerationError, generation::GenerationState};
use std::{error, fmt};

use crate::index::ProjectionError;

/// Why a route could not search its pinned generation.
#[derive(Debug)]
pub enum RouteError {
    /// The kernel could not read the generation records at admission.
    GenerationLookup(GenerationError),
    /// No generation is recorded for this collection.
    UnknownGeneration {
        /// The collection searched for at admission.
        collection: String,
    },
    /// The collection has generations, but none is published.
    UnpublishedGeneration {
        /// The most recently recorded generation.
        generation: i64,
        /// Its state at admission.
        state: GenerationState,
    },
    /// The route profile does not match the profile pinned in the generation.
    ProfileMismatch {
        /// The profile required by the generation or analyzer.
        expected: String,
        /// The profile supplied by this route.
        found: String,
    },
    /// The embedder refused, was unreachable, timed out or had no free room.
    EmbedderUnavailable {
        /// Why the model port could not answer.
        reason: String,
    },
    /// The embedding response cannot be used as a dense query vector.
    InvalidVector(VectorError),
    /// The projection backend refused the route query or returned an invalid hit.
    Projection(ProjectionError),
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationLookup(error) => write!(formatter, "generation lookup failed: {error}"),
            Self::UnknownGeneration { collection } => {
                write!(
                    formatter,
                    "no generation is recorded for collection {collection}"
                )
            }
            Self::UnpublishedGeneration { generation, state } => write!(
                formatter,
                "generation {generation} is {state}, not published"
            ),
            Self::ProfileMismatch { expected, found } => {
                write!(
                    formatter,
                    "generation profile is {expected}, route has {found}"
                )
            }
            Self::EmbedderUnavailable { reason } => {
                write!(formatter, "embedder unavailable: {reason}")
            }
            Self::InvalidVector(error) => write!(formatter, "invalid embedding vector: {error}"),
            Self::Projection(error) => {
                write!(formatter, "projection backend route query failed: {error}")
            }
        }
    }
}

impl error::Error for RouteError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::GenerationLookup(error) => Some(error),
            Self::Projection(error) => Some(error),
            Self::InvalidVector(error) => Some(error),
            Self::UnknownGeneration { .. }
            | Self::UnpublishedGeneration { .. }
            | Self::ProfileMismatch { .. }
            | Self::EmbedderUnavailable { .. } => None,
        }
    }
}

/// Which dense query-vector validation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorError {
    /// The embedding response contained no vector or more than one.
    Count {
        /// How many vectors the port returned.
        found: usize,
    },
    /// The vector size differs from the embedder card's dimensions.
    Dimensions {
        /// The size recorded by the card.
        expected: usize,
        /// The size returned by the model port.
        found: usize,
    },
    /// A vector component is NaN or infinite.
    NonFinite,
    /// The all-zero vector has no cosine direction.
    Zero,
}

impl fmt::Display for VectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { found } => write!(formatter, "expected one query vector, found {found}"),
            Self::Dimensions { expected, found } => {
                write!(formatter, "expected {expected} dimensions, found {found}")
            }
            Self::NonFinite => formatter.write_str("a component is not finite"),
            Self::Zero => formatter.write_str("the vector is zero"),
        }
    }
}

impl error::Error for VectorError {}
