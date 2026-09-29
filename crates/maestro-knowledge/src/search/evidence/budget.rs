//! Token-counting helpers for compact serialized evidence.

use super::types::{EvidenceCounter, EvidenceError};
use maestro_kernel::evidence::Passage;
use std::{error, fmt};

/// The answer-bound wire's allowance above the budget, in bytes, for the
/// passage JSON the answerer never reads: section, document and revision
/// IDs, source reference, span, digest, window flag and alternates. With
/// digest IDs and a 100-byte URL that is about 520 bytes a passage and 105
/// an alternate, so about ten passages fit. It is the room the former fixed
/// 12,000-byte ceiling left above the default 6,000-byte budget, which keeps
/// the default's selection unchanged.
const EVIDENCE_WIRE_ALLOWANCE: usize = 6_000;

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
        EvidenceCounter::AnswerBoundUtf8Bytes => Ok(CounterInfo {
            counter: Some("evidence-answer-bound-utf8-bytes/1".to_owned()),
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

/// The largest compact passage JSON, in bytes, that an answer-bound trial
/// under a `evidence_bytes` budget may serialize to: the budget plus
/// [`EVIDENCE_WIRE_ALLOWANCE`], so 12,000 at the default 6,000, 18,000 at
/// 12,000 and 30,000 at the 24,000 ceiling.
pub(crate) fn evidence_wire_ceiling(evidence_bytes: u32) -> usize {
    usize::try_from(evidence_bytes).map_or(usize::MAX, |budget| {
        budget.saturating_add(EVIDENCE_WIRE_ALLOWANCE)
    })
}

/// Counts the compact JSON serialization of a complete passage trial under
/// a `evidence_bytes` budget.
pub(crate) fn count_passages(
    passages: &[Passage],
    counter: &EvidenceCounter,
    info: &CounterInfo,
    evidence_bytes: u32,
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
    if matches!(counter, EvidenceCounter::AnswerBoundUtf8Bytes)
        && serialized.len() > evidence_wire_ceiling(evidence_bytes)
    {
        // Admitted budgets are below this sentinel: try a smaller source window.
        return Ok(u32::MAX);
    }
    let count = match counter {
        EvidenceCounter::Utf8Bytes => serialized.len(),
        EvidenceCounter::AnswerBoundUtf8Bytes => answer_bound_passages(passages)?.len(),
        EvidenceCounter::Exact(counter) => counter
            .token_ids(&serialized)
            .map_err(CounterError::Counter)?
            .len(),
    };
    u32::try_from(count).map_err(|_| CounterError::Invalid("evidence token count exceeds u32"))
}

/// Serializes the answer-bound fields, excluding provenance and wire metadata.
fn answer_bound_passages(passages: &[Passage]) -> Result<String, CounterError> {
    let answer_bound = passages
        .iter()
        .map(|passage| {
            serde_json::json!({
                "n": passage.n,
                "title": passage.title,
                "section_path": passage.section_path,
                "text": passage.text,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&answer_bound)
        .map(|text| text.replace('<', "\\u003c").replace('>', "\\u003e"))
        .map_err(CounterError::Json)
}

/// Serializes only passages, the evidence budget's counted input.
pub(crate) fn serialized_passages(passages: &[Passage]) -> Result<String, CounterError> {
    serde_json::to_string(passages).map_err(CounterError::Json)
}
