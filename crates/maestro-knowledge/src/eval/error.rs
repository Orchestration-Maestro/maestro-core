//! Why a run or a comparison was refused.

use crate::suite::Unresolved;
use maestro_kernel::artifact::Digest;
use std::{error, fmt};

/// Why v2 attempt reports could not be combined into one strict report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateError(String);

impl AggregateError {
    /// Wraps a reason that prevented v2 attempt aggregation.
    pub(super) fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl fmt::Display for AggregateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl error::Error for AggregateError {}

/// Why a run was refused: the caller's own error, a suite that does not fit
/// the generation it evaluates, or a bundle of another generation. `E` is
/// the error of the caller's lookup and retrieval.
#[derive(Debug)]
pub enum RunError<E> {
    /// The lookup of a canonical document failed.
    Documents {
        /// The document's `source_ref`.
        source_ref: String,
        /// The lookup's error.
        error: E,
    },
    /// The lookup found no document of a `source_ref` a question names: the
    /// generation does not hold it.
    NoDocument {
        /// The question's id.
        question: String,
        /// The `source_ref` it names.
        source_ref: String,
    },
    /// A name of an expected section gives no one section of its document,
    /// nor, with an empty heading path, a document without sections.
    Unresolved {
        /// The question's id.
        question: String,
        /// The document's `source_ref`.
        source_ref: String,
        /// The heading path the question names.
        heading_path: Vec<String>,
        /// Why it gives no one section.
        reason: Unresolved,
    },
    /// Expected-section names outside one shared group give one section.
    SameSection {
        /// The question's id.
        question: String,
        /// The section's ID.
        section_id: String,
    },
    /// Expected-document names outside one shared group give one whole document.
    SameDocument {
        /// The question's id.
        question: String,
        /// The document's ID.
        document_id: String,
    },
    /// The retrieval of a question failed.
    Retrieval {
        /// The question's id.
        question: String,
        /// The retrieval's error.
        error: E,
    },
    /// Durable attempt recording failed; no further items may run.
    Recording {
        /// The question whose receipt could not be recorded.
        question: String,
        /// Persistence error.
        reason: String,
    },
    /// A v2 header has no valid attempt identity or route.
    InvalidV2Header {
        /// Why the caller-provided header is invalid.
        reason: String,
    },
    /// The generated v2 report failed its strict reader validation.
    InvalidV2Report {
        /// Why the serialized report was refused.
        reason: String,
    },
    /// A question was answered from another collection or generation than
    /// the one the run evaluates.
    OtherGeneration {
        /// The question's id.
        question: String,
        /// The collection its bundle searched.
        collection: String,
        /// The generation its bundle was pinned to.
        generation: i64,
    },
}

impl<E: fmt::Display> fmt::Display for RunError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Documents { source_ref, error } => write!(
                formatter,
                "the canonical document of {source_ref} could not be read: {error}"
            ),
            Self::NoDocument {
                question,
                source_ref,
            } => write!(
                formatter,
                "question {question} expects {source_ref}, which the generation does not hold"
            ),
            Self::Unresolved {
                question,
                source_ref,
                heading_path,
                reason,
            } if heading_path.is_empty() => write!(
                formatter,
                "question {question} expects the whole of {source_ref}: {reason}"
            ),
            Self::Unresolved {
                question,
                source_ref,
                heading_path,
                reason,
            } => write!(
                formatter,
                "question {question} expects {} in {source_ref}, which names no one section: \
                 {reason}",
                heading_path.join(" › ")
            ),
            Self::SameSection {
                question,
                section_id,
            } => write!(
                formatter,
                "question {question} names the section {section_id} twice"
            ),
            Self::SameDocument {
                question,
                document_id,
            } => write!(
                formatter,
                "question {question} names the document {document_id} twice"
            ),
            Self::Retrieval { question, error } => write!(
                formatter,
                "question {question} could not be retrieved: {error}"
            ),
            Self::Recording { question, reason } => {
                write!(
                    formatter,
                    "the attempt receipt for {question} could not be recorded: {reason}"
                )
            }
            Self::InvalidV2Header { reason } => {
                write!(formatter, "the v2 report header is invalid: {reason}")
            }
            Self::InvalidV2Report { reason } => {
                write!(
                    formatter,
                    "the generated v2 report failed validation: {reason}"
                )
            }
            Self::OtherGeneration {
                question,
                collection,
                generation,
            } => write!(
                formatter,
                "question {question} was answered from generation {generation} of \
                 {collection}, not from the generation the run evaluates"
            ),
        }
    }
}

impl<E: error::Error + 'static> error::Error for RunError<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Documents { error, .. } | Self::Retrieval { error, .. } => Some(error),
            Self::Unresolved { reason, .. } => Some(reason),
            Self::NoDocument { .. }
            | Self::SameSection { .. }
            | Self::SameDocument { .. }
            | Self::Recording { .. }
            | Self::InvalidV2Header { .. }
            | Self::InvalidV2Report { .. }
            | Self::OtherGeneration { .. } => None,
        }
    }
}

/// Why two runs could not be paired question by question: they evaluated
/// different collections, ran different suites or different files of one,
/// or their questions differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    /// A question is in one run only.
    Unpaired {
        /// The question's id.
        id: String,
    },
    /// A question is given twice in one run.
    Repeated {
        /// The question's id.
        id: String,
    },
    /// A question's attempt numbers, seeds or warm-up assignments differ.
    Attempts {
        /// The question's id.
        id: String,
    },
    /// Candidate and baseline use different evaluation modes.
    Mode,
    /// Candidate and baseline belong to different frozen runs.
    RunId,
    /// Candidate and baseline measure different independent routes.
    Route,
    /// A question's frozen language/cross-lingual labels differ.
    Labels {
        /// The question's id.
        id: String,
    },
    /// A question is answerable in one run only.
    Answerability {
        /// The question's id.
        id: String,
    },
    /// The two runs ran different suites.
    Suite {
        /// The baseline's suite.
        baseline: String,
        /// The candidate's suite.
        candidate: String,
    },
    /// The two runs evaluated different collections.
    Collection {
        /// The baseline's collection.
        baseline: String,
        /// The candidate's collection.
        candidate: String,
    },
    /// The reports use different protocol versions, so their metrics are not
    /// comparable without v2 frozen input identities.
    Version,
    /// The reports were produced under different frozen bake-off manifests.
    ManifestDigest {
        /// The baseline manifest digest.
        baseline: Digest,
        /// The candidate manifest digest.
        candidate: Digest,
    },
    /// The corpus revisions or quality decisions differ.
    CorpusDigest {
        /// The baseline corpus digest.
        baseline: Digest,
        /// The candidate corpus digest.
        candidate: Digest,
    },
    /// The canonical/original input freeze differs.
    InputDigest {
        /// The baseline input digest.
        baseline: Digest,
        /// The candidate input digest.
        candidate: Digest,
    },
    /// The two runs read their suite from files of different digests: the
    /// suite changed between them.
    SuiteDigest {
        /// The suite.
        suite: String,
        /// The digest of the baseline's file.
        baseline: Digest,
        /// The digest of the candidate's file.
        candidate: Digest,
    },
}

impl fmt::Display for CompareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (id, why) = match self {
            Self::Suite {
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline ran the suite {baseline} and the candidate the suite \
                     {candidate}, so the runs cannot be paired"
                );
            }
            Self::Collection {
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline evaluated the collection {baseline} and the candidate the \
                     collection {candidate}, so the runs cannot be paired"
                );
            }
            Self::SuiteDigest {
                suite,
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline and the candidate ran the suite {suite} from different files, \
                     of digests {} and {}, so the runs cannot be paired",
                    baseline.as_str(),
                    candidate.as_str()
                );
            }
            Self::Version => {
                return formatter.write_str(
                    "v1 and v2 reports cannot be paired without v2 frozen input digests",
                );
            }
            Self::ManifestDigest {
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline and candidate were produced under different manifests: {} and {}",
                    baseline.as_str(),
                    candidate.as_str()
                );
            }
            Self::CorpusDigest {
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline and candidate evaluated different frozen corpora: {} and {}",
                    baseline.as_str(),
                    candidate.as_str()
                );
            }
            Self::InputDigest {
                baseline,
                candidate,
            } => {
                return write!(
                    formatter,
                    "the baseline and candidate evaluated different frozen inputs: {} and {}",
                    baseline.as_str(),
                    candidate.as_str()
                );
            }
            Self::Attempts { id } => (id, "has a different attempt plan across runs"),
            Self::Labels { id } => (id, "has different frozen subgroup labels across runs"),
            Self::Unpaired { id } => (id, "is in one run only"),
            Self::Repeated { id } => (id, "is given twice in one run"),
            Self::Answerability { id } => (id, "is answerable in one run only"),
            Self::Mode => {
                return formatter.write_str("the runs have different evaluation modes");
            }
            Self::RunId => {
                return formatter.write_str("the runs have different frozen run IDs");
            }
            Self::Route => {
                return formatter.write_str("the runs measure different independent routes");
            }
        };
        write!(
            formatter,
            "the question {id} {why}, so the runs cannot be paired"
        )
    }
}

impl error::Error for CompareError {}
