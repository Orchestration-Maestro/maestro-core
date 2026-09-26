//! Why a run or a comparison was refused.

use crate::suite::Unresolved;
use std::{error, fmt};

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
    /// Two names of expected sections of a question give one section.
    SameSection {
        /// The question's id.
        question: String,
        /// The section's ID.
        section_id: String,
    },
    /// Two names of a question give one document without sections, whole.
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
                "question {question} expects a section of {source_ref}, which the generation \
                 does not hold"
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
            | Self::OtherGeneration { .. } => None,
        }
    }
}

/// Why two runs could not be paired question by question: they ran different
/// suites, or their questions differ.
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
            Self::Unpaired { id } => (id, "is in one run only"),
            Self::Repeated { id } => (id, "is given twice in one run"),
            Self::Answerability { id } => (id, "is answerable in one run only"),
        };
        write!(
            formatter,
            "the question {id} {why}, so the runs cannot be paired"
        )
    }
}

impl error::Error for CompareError {}
