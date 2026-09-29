//! Public evidence request types and their stable error boundary.

use super::super::assembly_settings::{CounterMode, EvidenceSettings};
use maestro_canonicalization::{Error as CanonicalError, TokenCounter};
use maestro_kernel::{
    chunk_set, document, generation, retrieval, store, telemetry::stage::Outcome,
};
use std::{error, fmt, sync::Arc};

/// How the complete serialized passage array is measured.
pub enum EvidenceCounter {
    /// Count UTF-8 bytes of complete serialized passages as an explicitly marked estimate.
    Utf8Bytes,
    /// Count UTF-8 bytes of the answer-bound passage fields, excluding provenance.
    AnswerBoundUtf8Bytes,
    /// Count exact token IDs from a verified tokenizer contract.
    Exact(Arc<dyn TokenCounter + Send + Sync>),
}

impl fmt::Debug for EvidenceCounter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8Bytes => formatter.write_str("Utf8Bytes"),
            Self::AnswerBoundUtf8Bytes => formatter.write_str("AnswerBoundUtf8Bytes"),
            Self::Exact(counter) => formatter
                .debug_tuple("Exact")
                .field(&counter.contract_id())
                .finish(),
        }
    }
}

impl EvidenceSettings {
    /// Resolves the named counter without silently substituting an estimate.
    ///
    /// # Errors
    /// Exact mode is unavailable until the resolved answerer's tokenizer is qualified.
    pub fn counter(self) -> Result<EvidenceCounter, EvidenceError> {
        match self.evidence_counter {
            CounterMode::Utf8 => Ok(EvidenceCounter::Utf8Bytes),
            CounterMode::Utf8AnswerBound => Ok(EvidenceCounter::AnswerBoundUtf8Bytes),
            CounterMode::Exact => Err(EvidenceError::InvalidRequest(
                concat!(
                    "exact evidence counting is unavailable: ",
                    "the resolved answerer's tokenizer must be qualified"
                )
                .to_owned(),
            )),
        }
    }

    /// Refuses a counter other than the one `evidence_counter` names, so the
    /// setting stays the single source of the applied counter.
    ///
    /// # Errors
    /// The counter's strategy differs from the configured one.
    pub(crate) fn check_counter(self, counter: &EvidenceCounter) -> Result<(), EvidenceError> {
        let configured = match counter {
            EvidenceCounter::Utf8Bytes => CounterMode::Utf8,
            EvidenceCounter::AnswerBoundUtf8Bytes => CounterMode::Utf8AnswerBound,
            EvidenceCounter::Exact(_) => CounterMode::Exact,
        };
        if configured == self.evidence_counter {
            Ok(())
        } else {
            Err(EvidenceError::InvalidRequest(
                "evidence counter differs from the configured evidence_counter".to_owned(),
            ))
        }
    }
}

/// Why authoritative evidence could not be assembled.
#[derive(Debug)]
pub enum EvidenceError {
    /// The bounded handoff violates the evidence contract.
    InvalidRequest(String),
    /// The requested collection or generation is missing or inaccessible.
    NotVisible,
    /// The pinned generation record could not be read.
    Generation(generation::Error),
    /// The pinned chunk set record could not be read.
    ChunkSet(chunk_set::Error),
    /// An authorized revision, document or disposition could not be read.
    Records(document::Error),
    /// An artifact or permission-snapshot read failed.
    Store(store::Error),
    /// A controlled, scoped kernel search read failed.
    Kernel(retrieval::Error),
    /// Canonical evidence is inconsistent with its authorized records.
    Integrity(String),
    /// An exact token counter failed.
    Counter(CanonicalError),
    /// Evidence JSON could not be serialized or parsed.
    Json(serde_json::Error),
    /// The inherited deadline elapsed or the caller cancelled its worker.
    TimedOut,
    /// The caller's read grants changed during assembly.
    PermissionsChanged,
    /// The blocking worker could not be joined.
    WorkerFailed,
}

impl EvidenceError {
    /// How an assembly stage that failed with this error ended: refused by
    /// its contract or its caller's rights, out of time, or failed.
    pub(super) const fn outcome(&self) -> Outcome {
        match self {
            Self::InvalidRequest(_) | Self::NotVisible | Self::PermissionsChanged => {
                Outcome::Refused
            }
            Self::TimedOut => Outcome::Timeout,
            Self::Generation(_)
            | Self::ChunkSet(_)
            | Self::Records(_)
            | Self::Store(_)
            | Self::Kernel(_)
            | Self::Integrity(_)
            | Self::Counter(_)
            | Self::Json(_)
            | Self::WorkerFailed => Outcome::Error,
        }
    }
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(reason) | Self::Integrity(reason) => formatter.write_str(reason),
            Self::NotVisible => formatter.write_str("evidence is unknown or inaccessible"),
            Self::Generation(_) => formatter.write_str("the pinned generation could not be read"),
            Self::ChunkSet(_) => formatter.write_str("the pinned chunk set could not be read"),
            Self::Records(_) => {
                formatter.write_str("authorized evidence records could not be read")
            }
            Self::Store(_) => formatter.write_str("an evidence artifact could not be read"),
            Self::Kernel(_) => formatter.write_str("the controlled evidence read failed"),
            Self::Counter(_) => formatter.write_str("the exact evidence counter failed"),
            Self::Json(_) => formatter.write_str("evidence JSON could not be processed"),
            Self::TimedOut => formatter.write_str("evidence assembly reached its deadline"),
            Self::PermissionsChanged => {
                formatter.write_str("evidence permissions changed during assembly")
            }
            Self::WorkerFailed => formatter.write_str("the evidence worker failed"),
        }
    }
}

impl error::Error for EvidenceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Generation(error) => Some(error),
            Self::ChunkSet(error) => Some(error),
            Self::Records(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Kernel(error) => Some(error),
            Self::Counter(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidRequest(_)
            | Self::NotVisible
            | Self::Integrity(_)
            | Self::TimedOut
            | Self::PermissionsChanged
            | Self::WorkerFailed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EvidenceError;
    use std::error::Error as _;

    #[test]
    fn display_keeps_the_public_error_message() {
        assert_eq!(
            EvidenceError::InvalidRequest("invalid handoff".to_owned()).to_string(),
            "invalid handoff"
        );
        assert_eq!(
            EvidenceError::TimedOut.to_string(),
            "evidence assembly reached its deadline"
        );
    }

    #[test]
    fn wrapped_json_errors_remain_available_as_sources() {
        let source = serde_json::from_str::<serde_json::Value>("not JSON").unwrap_err();
        let error = EvidenceError::Json(source);

        assert!(error.source().is_some());
    }
}
