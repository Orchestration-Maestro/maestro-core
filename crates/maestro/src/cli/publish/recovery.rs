//! Frozen identity and restart selection for explicit projection recovery.

use crate::{cli::args::PublishArguments, failure::Failure, kernel::Kernel};
use maestro_kernel::{
    gateway::ModelCard,
    generation::GenerationState,
    job::{self, JobState},
    journal::Filter,
    retrieval::IDENTIFIER_PROFILE,
};
use maestro_knowledge::{
    index::{Progress, RebuildGuard},
    lexical,
};
use serde_json::{Value, json};
use std::collections::HashSet;
use ulid::Ulid;

/// The kind of the job a publication runs.
pub(in crate::cli) const KIND: &str = "knowledge.publish";

/// A journaled job whose identity could resume the requested replacement.
pub(in crate::cli) struct ResumeCandidate {
    /// Its latest attempt's job ID.
    pub(in crate::cli) id: Ulid,
    /// Whether the latest attempt is active or failed.
    pub(in crate::cli) state: JobState,
    /// The expected pointer and generation boundary frozen in its inputs.
    pub(in crate::cli) recovery: RebuildGuard,
    /// The target named by its last progress event, if any.
    pub(in crate::cli) progress_generation: Option<i64>,
    /// The target's state, if the journaled target is still visible.
    pub(in crate::cli) target_state: Option<GenerationState>,
    /// The exact immutable inputs to resubmit.
    pub(in crate::cli) inputs: Value,
}

/// The frozen publication identity across sparse and identifier profiles.
pub(in crate::cli) fn frozen_inputs(card: &str, chunk_set: &str, collection: &str) -> Value {
    json!({
        "card": card,
        "chunk_set": chunk_set,
        "collection": collection,
        "identifier_profile": IDENTIFIER_PROFILE,
        "sparse_profile": lexical::PROFILE,
    })
}

/// Freezes the replacement nonce and expected-current generation guard.
pub(in crate::cli) fn recovery_inputs(
    card: &str,
    chunk_set: &str,
    collection: &str,
    guard: RebuildGuard,
    again: Ulid,
) -> Value {
    json!({
        "card": card,
        "chunk_set": chunk_set,
        "collection": collection,
        "identifier_profile": IDENTIFIER_PROFILE,
        "sparse_profile": lexical::PROFILE,
        "again": again.to_string(),
        "expected_published": guard.expected_published,
        "generation_watermark": guard.generation_watermark,
    })
}

/// Freezes the job inputs for ordinary publication or explicit recovery.
pub(in crate::cli) fn selected_inputs(
    kernel: &Kernel,
    resource: &str,
    arguments: &PublishArguments,
    card: &ModelCard,
    chunk_set: &str,
) -> Result<(Value, Option<RebuildGuard>, bool), Failure> {
    let collection = arguments.collection.as_str();
    if !arguments.again {
        return Ok((
            frozen_inputs(card.digest().as_str(), chunk_set, collection),
            None,
            false,
        ));
    }
    let expected_published = kernel
        .database
        .published_generation(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?
        .map(|generation| generation.id);
    if let Some(inputs) = resumable_inputs(kernel, resource, arguments, card, expected_published)? {
        let recovery = recovery(&inputs)?;
        return Ok((inputs, Some(recovery), true));
    }
    let generation_watermark = kernel
        .database
        .generations(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?
        .iter()
        .map(|generation| generation.id)
        .max()
        .unwrap_or(0);
    let guard = RebuildGuard {
        expected_published,
        generation_watermark,
    };
    let inputs = recovery_inputs(
        card.digest().as_str(),
        chunk_set,
        collection,
        guard,
        Ulid::generate(),
    );
    let recovery = recovery(&inputs)?;
    Ok((inputs, Some(recovery), false))
}

/// Reuses the unique visible current or failed replacement job with the same tuple.
pub(super) fn resumable_inputs(
    kernel: &Kernel,
    resource: &str,
    arguments: &PublishArguments,
    card: &ModelCard,
    expected: Option<i64>,
) -> Result<Option<Value>, Failure> {
    let collection = arguments.collection.as_str();
    let chunk_set = arguments
        .chunk_set
        .as_deref()
        .ok_or_else(|| Failure::refused("knowledge publish --again requires --chunk-set"))?;
    let card_digest = card.digest().as_str();
    let jobs = kernel
        .database
        .jobs_for_resource(&kernel.scopes, KIND, resource)
        .map_err(|error| Failure::failed_by(&error))?;
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for job in jobs {
        if !seen.insert(job.idempotency_key.clone())
            || !matches!(
                job.state,
                JobState::Queued | JobState::Running | JobState::Failed
            )
        {
            continue;
        }
        let stream = job::stream(job.id);
        let events = kernel
            .database
            .events(
                &kernel.scopes,
                &Filter {
                    stream: &stream,
                    after: 0,
                    r#type: Some(job::CREATED),
                },
            )
            .map_err(|error| Failure::failed_by(&error))?;
        let inputs = events
            .last()
            .and_then(|event| event.data.get("inputs"))
            .ok_or_else(|| {
                Failure::failed(format!("publication job {} has no frozen inputs", job.id))
            })?;
        if inputs.get("again").and_then(Value::as_str).is_none()
            || inputs.get("collection").and_then(Value::as_str) != Some(collection)
            || inputs.get("chunk_set").and_then(Value::as_str) != Some(chunk_set)
            || inputs.get("card").and_then(Value::as_str) != Some(card_digest)
            || inputs.get("identifier_profile").and_then(Value::as_str) != Some(IDENTIFIER_PROFILE)
            || inputs.get("sparse_profile").and_then(Value::as_str) != Some(lexical::PROFILE)
        {
            continue;
        }
        let recovery = recovery(inputs)?;
        let progress = last_progress(kernel, job.id)?;
        let progress_generation = progress.as_ref().map(|progress| progress.generation);
        let target_state = if let Some(generation) = progress_generation {
            kernel
                .database
                .generation(&kernel.scopes, generation)
                .map_err(|error| Failure::failed_by(&error))?
                .map(|generation| generation.state)
        } else {
            None
        };
        candidates.push(ResumeCandidate {
            id: job.id,
            state: job.state,
            recovery,
            progress_generation,
            target_state,
            inputs: inputs.clone(),
        });
    }
    select_resume(&candidates, expected)
}

/// Chooses one unambiguous resumable job; stale and ambiguous jobs are named.
pub(in crate::cli) fn select_resume(
    candidates: &[ResumeCandidate],
    expected: Option<i64>,
) -> Result<Option<Value>, Failure> {
    match candidates {
        [] => Ok(None),
        [candidate] if can_resume(candidate, expected) => Ok(Some(candidate.inputs.clone())),
        [candidate] => Err(Failure::refused(format!(
            "publication job {} cannot resume: it expected {}, but {} is published; \
             refusing target {}",
            candidate.id,
            generation_name(candidate.recovery.expected_published),
            generation_name(expected),
            candidate.progress_generation.map_or_else(
                || "without a journaled target".to_owned(),
                |generation| generation_name(Some(generation)),
            ),
        ))),
        _ => {
            let ids = candidates
                .iter()
                .map(|candidate| candidate.id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(Failure::refused(format!(
                "multiple publication jobs match ({ids}); refusing to choose a recovery job"
            )))
        }
    }
}

/// Whether a journaled attempt still owns the only safe recovery transition.
fn can_resume(candidate: &ResumeCandidate, expected: Option<i64>) -> bool {
    let pointer_matches = candidate.recovery.expected_published == expected;
    match candidate.state {
        JobState::Queued | JobState::Running => match candidate.progress_generation {
            None => pointer_matches,
            Some(_)
                if matches!(
                    candidate.target_state,
                    Some(GenerationState::Building | GenerationState::Verified)
                ) =>
            {
                pointer_matches
            }
            Some(target)
                if candidate.target_state == Some(GenerationState::Published)
                    && expected == Some(target)
                    && candidate.recovery.expected_published != Some(target) =>
            {
                true
            }
            Some(_) => false,
        },
        JobState::Failed => {
            pointer_matches
                && match candidate.progress_generation {
                    None => true,
                    Some(_) => matches!(
                        candidate.target_state,
                        Some(GenerationState::Building | GenerationState::Verified)
                    ),
                }
        }
        JobState::Succeeded | JobState::Cancelled => false,
    }
}

/// A visible generation ID or its absence, in language for a person.
fn generation_name(generation: Option<i64>) -> String {
    generation.map_or_else(|| "none".to_owned(), |id| format!("generation {id}"))
}

/// Reads the guarded recovery fields from frozen job inputs.
pub(in crate::cli) fn recovery(inputs: &Value) -> Result<RebuildGuard, Failure> {
    let expected_published = match inputs.get("expected_published") {
        Some(Value::Null) => None,
        Some(Value::Number(number)) => {
            Some(number.as_i64().filter(|id| *id > 0).ok_or_else(|| {
                Failure::failed("a publication has an invalid expected generation ID")
            })?)
        }
        _ => {
            return Err(Failure::failed(
                "a publication has no expected generation ID",
            ));
        }
    };
    let generation_watermark = inputs
        .get("generation_watermark")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(|| Failure::failed("a publication has an invalid generation watermark"))?;
    let nonce = inputs
        .get("again")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::failed("a replacement publication has no nonce"))?;
    Ulid::from_string(nonce).map_err(|error| Failure::failed_by(&error))?;
    Ok(RebuildGuard {
        expected_published,
        generation_watermark,
    })
}

/// The last publication step, including the previous attempt's step when a
/// retry has not yet journaled one of its own.
pub(in crate::cli) fn last_progress(
    kernel: &Kernel,
    job_id: Ulid,
) -> Result<Option<Progress>, Failure> {
    let mut job_id = job_id;
    loop {
        if let Some(event) = kernel
            .database
            .last_progress(&kernel.scopes, job_id)
            .map_err(|error| Failure::failed_by(&error))?
        {
            return serde_json::from_value(event.data)
                .map(Some)
                .map_err(|error| Failure::failed_by(&error));
        }
        let stream = job::stream(job_id);
        let events = kernel
            .database
            .events(
                &kernel.scopes,
                &Filter {
                    stream: &stream,
                    after: 0,
                    r#type: Some(job::CREATED),
                },
            )
            .map_err(|error| Failure::failed_by(&error))?;
        let Some(previous) = events
            .last()
            .and_then(|event| event.data.get("previous_attempt"))
        else {
            return Ok(None);
        };
        let previous = previous.as_str().ok_or_else(|| {
            Failure::failed("a publication attempt has an invalid previous-attempt ID")
        })?;
        job_id = Ulid::from_string(previous).map_err(|error| Failure::failed_by(&error))?;
    }
}

#[cfg(test)]
mod tests {
    use super::{ResumeCandidate, can_resume};
    use maestro_kernel::{generation::GenerationState, job::JobState};
    use maestro_knowledge::index::RebuildGuard;
    use serde_json::json;
    use ulid::Ulid;

    fn candidate(
        state: JobState,
        expected: Option<i64>,
        progress_generation: Option<i64>,
        target_state: Option<GenerationState>,
    ) -> ResumeCandidate {
        ResumeCandidate {
            id: Ulid::generate(),
            state,
            recovery: RebuildGuard {
                expected_published: expected,
                generation_watermark: expected.unwrap_or_default(),
            },
            progress_generation,
            target_state,
            inputs: json!({}),
        }
    }

    #[test]
    fn resume_requires_the_frozen_pointer_and_an_unpublished_or_own_published_target() {
        use GenerationState::{Building, Failed, Published, Verified};
        use JobState::{Cancelled, Failed as FailedJob, Queued, Running, Succeeded};

        assert!(can_resume(&candidate(Queued, Some(1), None, None), Some(1)));
        assert!(!can_resume(
            &candidate(Running, Some(1), None, None),
            Some(2)
        ));
        for target_state in [Building, Verified] {
            assert!(can_resume(
                &candidate(Running, Some(1), Some(2), Some(target_state)),
                Some(1)
            ));
        }
        assert!(can_resume(
            &candidate(Running, Some(1), Some(2), Some(Published)),
            Some(2)
        ));
        assert!(!can_resume(
            &candidate(Running, Some(1), Some(2), Some(Published)),
            Some(1)
        ));
        assert!(!can_resume(
            &candidate(Running, Some(1), Some(2), Some(Failed)),
            Some(1)
        ));

        assert!(can_resume(
            &candidate(FailedJob, Some(1), None, None),
            Some(1)
        ));
        assert!(!can_resume(
            &candidate(FailedJob, Some(1), None, None),
            Some(2)
        ));
        assert!(can_resume(
            &candidate(FailedJob, Some(1), Some(2), Some(Verified)),
            Some(1)
        ));
        assert!(!can_resume(
            &candidate(FailedJob, Some(1), Some(2), Some(Published)),
            Some(1)
        ));
        for state in [Succeeded, Cancelled] {
            assert!(!can_resume(&candidate(state, Some(1), None, None), Some(1)));
        }
    }
}
