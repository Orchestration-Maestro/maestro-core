//! Dense vectors: a batch of prepared inputs embedded by the embedder a model
//! card names, through the model port in free room, and checked before
//! anything holds them.

use maestro_kernel::gateway::{self, ModelCard, ModelPort, Room};
use std::{error, fmt, num::NonZeroUsize, time::Duration};
use tokio::time;

/// The profile of the dense vectors the embedder `card` names gives:
/// `dense/1:sha256:<the card's digest>`. The card pins the model file and
/// the router's build, so a new model or a new build is a new profile, and
/// version 1 embeds a chunk's prepared input as it is, the text its chunk
/// set counted, with no template around it.
pub(super) fn embedding_profile(card: &ModelCard) -> String {
    format!("dense/1:sha256:{}", card.digest().as_str())
}

/// The dense vector of each of `inputs`, in their order, from the embedder
/// `card` names through `port`, which loads it only into free room, so that
/// indexing never unloads another model (FR-S1-015a). The call must answer
/// within `deadline`, and its vectors pass every check of [`Refusal`]: one
/// per input, each of the card's dimensions, every value finite and its
/// norm not zero. A card without dimensions, which is not an embedder's, the
/// port refuses before any call.
///
/// # Errors
///
/// [`Failure::Port`] when the port refuses, the embedder not fitting in free
/// room among its reasons; [`Failure::TimedOut`] when it gives no answer
/// within `deadline`; and [`Failure::Refused`] when a vector breaks a check:
/// the whole batch is refused.
pub(super) async fn embed<P: ModelPort>(
    port: &P,
    card: &ModelCard,
    inputs: &[String],
    deadline: Duration,
) -> Result<Vec<Vec<f32>>, Failure> {
    let vectors = time::timeout(deadline, port.embed(card, Room::Free, inputs))
        .await
        .map_err(|_| Failure::TimedOut(deadline))?
        .map_err(Failure::Port)?;
    let dimensions = card.fields().dimensions.map_or(0, NonZeroUsize::get);
    check(&vectors, inputs.len(), dimensions).map_err(Failure::Refused)?;
    Ok(vectors)
}

/// Refuses `vectors` unless there is one for each of `inputs` inputs, each of
/// `dimensions` values, every value finite, and not every value zero.
fn check(vectors: &[Vec<f32>], inputs: usize, dimensions: usize) -> Result<(), Refusal> {
    if vectors.len() != inputs {
        return Err(Refusal::Count {
            inputs,
            vectors: vectors.len(),
        });
    }
    for (input, vector) in vectors.iter().enumerate() {
        if vector.len() != dimensions {
            return Err(Refusal::Dimensions {
                input,
                expected: dimensions,
                found: vector.len(),
            });
        }
        if !vector.iter().all(|value| value.is_finite()) {
            return Err(Refusal::NonFinite { input });
        }
        if vector.iter().all(|value| *value == 0.0) {
            return Err(Refusal::Zero { input });
        }
    }
    Ok(())
}

/// Why the vectors of a batch were refused: the first check one broke. An
/// input is named by its place in the batch, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The port gave another number of vectors than inputs.
    Count {
        /// How many inputs the batch had.
        inputs: usize,
        /// How many vectors came back.
        vectors: usize,
    },
    /// A vector has another size than the card's dimensions.
    Dimensions {
        /// The input whose vector it is.
        input: usize,
        /// The card's dimensions.
        expected: usize,
        /// The vector's.
        found: usize,
    },
    /// A value of a vector is NaN or infinite.
    NonFinite {
        /// The input whose vector it is.
        input: usize,
    },
    /// Every value of a vector is zero, which gives no direction to compare
    /// by cosine.
    Zero {
        /// The input whose vector it is.
        input: usize,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { inputs, vectors } => {
                write!(formatter, "{vectors} vectors came back for {inputs} inputs")
            }
            Self::Dimensions {
                input,
                expected,
                found,
            } => write!(
                formatter,
                "the vector of input {input} has {found} dimensions, where the card records \
                 {expected}"
            ),
            Self::NonFinite { input } => write!(
                formatter,
                "the vector of input {input} holds a value that is not finite"
            ),
            Self::Zero { input } => write!(
                formatter,
                "the vector of input {input} is zero, which has no direction"
            ),
        }
    }
}

/// Why a batch has no dense vectors.
#[derive(Debug)]
pub enum Failure {
    /// The port refused, the embedder not fitting in free room among its
    /// reasons.
    Port(gateway::Error),
    /// The port gave no answer within this deadline.
    TimedOut(Duration),
    /// A vector broke a check, which refuses the whole batch.
    Refused(Refusal),
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Port(error) => write!(formatter, "the embedder refused: {error}"),
            Self::TimedOut(deadline) => write!(
                formatter,
                "the embedder gave no answer within {} s",
                deadline.as_secs_f64()
            ),
            Self::Refused(refusal) => write!(formatter, "the batch is refused: {refusal}"),
        }
    }
}

impl error::Error for Failure {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Port(error) => Some(error),
            Self::TimedOut(_) | Self::Refused(_) => None,
        }
    }
}
