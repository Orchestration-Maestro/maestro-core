//! Token-counting helpers for compact serialized evidence.

use super::types::{EvidenceCounter, EvidenceError};
use maestro_kernel::evidence::Passage;
use std::{error, fmt};

/// The counter identity and estimate flag written into the bundle.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CounterInfo {
    /// Stable contract name, when the counter has one.
    pub(crate) counter: Option<String>,
    /// Whether the count is a conservative UTF-8 byte estimate.
    pub(crate) estimated: bool,
}

/// Why counting, serialization, or a helper precondition failed.
#[derive(Debug)]
pub(crate) enum CounterError {
    /// The exact tokenizer failed; preserve its source for the public error.
    Counter(maestro_canonicalization::Error),
    /// Passage JSON could not be serialized.
    Json(serde_json::Error),
    /// A local evidence-counting precondition was violated.
    Invalid(&'static str),
}

impl fmt::Display for CounterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Counter(_) => formatter.write_str("exact evidence counter failed"),
            Self::Json(_) => formatter.write_str("evidence passages could not be serialized"),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}

impl error::Error for CounterError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Counter(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<CounterError> for EvidenceError {
    fn from(error: CounterError) -> Self {
        match error {
            CounterError::Counter(error) => Self::Counter(error),
            CounterError::Json(error) => Self::Json(error),
            CounterError::Invalid(reason) => Self::InvalidRequest(reason.to_owned()),
        }
    }
}

/// Returns the wire identity and estimate status of a counter.
pub(crate) fn counter_info(counter: &EvidenceCounter) -> Result<CounterInfo, CounterError> {
    match counter {
        EvidenceCounter::Utf8Bytes => Ok(CounterInfo {
            counter: Some("evidence-utf8-bytes/1".to_owned()),
            estimated: true,
        }),
        EvidenceCounter::Exact(counter) => {
            let contract = counter.contract_id();
            if contract.trim().is_empty() {
                return Err(CounterError::Invalid(
                    "exact token counter has a blank contract ID",
                ));
            }
            Ok(CounterInfo {
                counter: Some(contract.to_owned()),
                estimated: false,
            })
        }
    }
}

/// Verifies that a counter still matches its accepted identity.
pub(crate) fn verify_counter(
    counter: &EvidenceCounter,
    expected: &CounterInfo,
) -> Result<(), CounterError> {
    if &counter_info(counter)? != expected {
        return Err(CounterError::Invalid(
            "token counter contract ID changed during assembly",
        ));
    }
    if let EvidenceCounter::Exact(counter) = counter {
        counter.verify().map_err(CounterError::Counter)?;
    }
    Ok(())
}

/// Counts the compact JSON serialization of a complete passage trial.
pub(crate) fn count_passages(
    passages: &[Passage],
    counter: &EvidenceCounter,
    info: &CounterInfo,
) -> Result<u32, CounterError> {
    if &counter_info(counter)? != info {
        return Err(CounterError::Invalid(
            "token counter contract ID changed during assembly",
        ));
    }
    if passages.is_empty() {
        return Ok(0);
    }
    let serialized = serialized_passages(passages)?;
    let count = match counter {
        EvidenceCounter::Utf8Bytes => serialized.len(),
        EvidenceCounter::Exact(counter) => counter
            .token_ids(&serialized)
            .map_err(CounterError::Counter)?
            .len(),
    };
    u32::try_from(count).map_err(|_| CounterError::Invalid("evidence token count exceeds u32"))
}

/// Serializes only passages, the evidence budget's counted input.
pub(crate) fn serialized_passages(passages: &[Passage]) -> Result<String, CounterError> {
    serde_json::to_string(passages).map_err(CounterError::Json)
}
