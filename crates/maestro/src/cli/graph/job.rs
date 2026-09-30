//! Foreground graph builds: extraction outside the fence lock, atomic receipts inside it.

use super::failure::claim_failure;
use crate::{
    cli::{
        foreground,
        lease::{Holder, TIMING},
        output::Output,
    },
    failure::Failure,
    kernel::Kernel,
};
use maestro_kernel::{
    document::Revision,
    facts::{BuildPlan, BuildRecord, ClaimSetRecord},
    job::{self, Job, JobState, NewJob},
    journal::{Event, Filter},
    scope::{ScopeSet, collection_path},
    store::Database,
};
use maestro_knowledge::graph::{build, rules::Extractor, verify::Source};
use serde_json::{Value, json};
use std::{collections::BTreeSet, iter};
use ulid::Ulid;

/// Maximum additional attempts for one source after its first started extraction.
const MAX_SOURCE_RETRIES: usize = 1;
/// Durable job progress event written immediately before a source extraction.
const SOURCE_STARTED: &str = "maestro.graph.build.source_started.v1";

/// Frozen work supplied to the foreground holder.
pub(super) struct Work<'a> {
    /// The full immutable input contract.
    pub(super) plan: &'a BuildPlan,
    /// Revisions in exactly the plan order.
    pub(super) revisions: &'a [Revision],
    /// Replaceable extraction implementation.
    pub(super) extractor: &'a dyn Extractor,
}

/// The authority-backed result, including a job identity for later attachment.
pub(super) struct Built {
    /// Build job.
    pub(super) job: Ulid,
    /// Receipts and retained rejections.
    pub(super) record: BuildRecord,
    /// No set when the completed extraction accepted nothing.
    pub(super) set: Option<ClaimSetRecord>,
}

/// Submit or resume exactly this plan, using the existing foreground lease runner.
pub(super) fn run(kernel: &Kernel, output: Output, work: &Work<'_>) -> Result<Built, Failure> {
    let estimate = check_token_budget(&kernel.database, work.revisions, work.extractor)?;
    let mut extractor_inputs = work.extractor.job_inputs();
    if let (Some(estimate), Some(inputs)) = (estimate, extractor_inputs.as_mut()) {
        let object = inputs
            .as_object_mut()
            .ok_or_else(|| Failure::failed("graph extractor inputs are not an object"))?;
        object.insert("estimated_tokens".to_owned(), json!(estimate));
    }
    let inputs = build::inputs(work.plan, extractor_inputs);
    let scope = collection_path(&work.plan.collection_id)
        .parse()
        .map_err(|error| Failure::refused_by(&error))?;
    let resource = format!("graph-build:{}", work.plan.collection_id);
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let ended = foreground::run_to_end(kernel, output, &new, TIMING, |holder| {
        outcome(work.run(kernel, holder))
    })?;
    check(&ended)?;
    let record = kernel
        .database
        .graph_build(&kernel.scopes, ended.id)
        .map_err(|error| claim_failure(&error))?
        .ok_or_else(|| Failure::failed("completed graph build has no receipts"))?;
    let set = record
        .claim_set_id
        .as_ref()
        .map(|id| kernel.database.claim_set(&kernel.scopes, id))
        .transpose()
        .map_err(|error| claim_failure(&error))?
        .flatten();
    Ok(Built {
        job: ended.id,
        record,
        set,
    })
}

impl Work<'_> {
    /// Resume at the durable receipt boundary, never repeating committed extraction.
    fn run(&self, kernel: &Kernel, holder: &Holder<'_>) -> Result<(), Failure> {
        let current = holder
            .fenced(|lease, _| {
                kernel
                    .database
                    .begin_graph_build(&kernel.scopes, lease, self.plan)
            })
            .map_err(|error| claim_failure(&error))?;
        let mut claims: usize = current.batches.iter().map(|batch| batch.claims.len()).sum();
        for (ordinal, revision) in self
            .revisions
            .iter()
            .enumerate()
            .skip(current.batches.len())
        {
            if self.extractor.token_budget().is_some() {
                start_source_attempt(&kernel.database, &kernel.scopes, holder, revision)?;
            }
            let batch = build::extract(&kernel.database, self.extractor, revision, ordinal)
                .map_err(|error| Failure::failed_by(&error))?;
            claims = claims.saturating_add(batch.claims.len());
            if claims > self.plan.budget.max_claims {
                return Err(Failure::refused(format!(
                    "max-claims bound {} exceeded (needed {claims}); raise --max-claims",
                    self.plan.budget.max_claims
                )));
            }
            holder
                .fenced(|lease, timing| {
                    kernel
                        .database
                        .record_graph_batch(&kernel.scopes, lease, timing, &batch)
                })
                .map_err(|error| claim_failure(&error))?;
        }
        if claims != 0 {
            holder
                .fenced(|lease, timing| {
                    kernel
                        .database
                        .finish_graph_build(&kernel.scopes, lease, timing)
                })
                .map_err(|error| claim_failure(&error))?;
        }
        Ok(())
    }
}

/// Verifies the full-plan estimate before job submission and extraction.
fn check_token_budget(
    database: &Database,
    revisions: &[Revision],
    extractor: &dyn Extractor,
) -> Result<Option<usize>, Failure> {
    let Some(limit) = extractor.token_budget() else {
        return Ok(None);
    };
    let mut estimated = 0_usize;
    for revision in revisions {
        let Ok(source) = Source::read(database, revision) else {
            return Err(Failure::refused(
                "graph source cannot be read for token accounting",
            ));
        };
        let Ok(cost) = extractor.estimated_tokens(&source) else {
            return Err(Failure::refused("graph token estimate is unavailable"));
        };
        estimated = estimated.saturating_add(cost);
    }
    if estimated > limit {
        return Err(Failure::refused(format!(
            "graph extraction token budget exceeded ({estimated} > {limit})"
        )));
    }
    Ok(Some(estimated))
}

/// Records one source start and refuses after its durable retry allowance is spent.
fn start_source_attempt(
    database: &Database,
    scopes: &ScopeSet,
    holder: &Holder<'_>,
    revision: &Revision,
) -> Result<(), Failure> {
    let stream = job::stream(holder.job_id());
    let history = database
        .events(
            scopes,
            &Filter {
                stream: &stream,
                after: 0,
                r#type: None,
            },
        )
        .map_err(|error| Failure::failed_by(&error))?;
    let attempt = next_source_attempt(&history, &revision.id)
        .filter(|_| within_retry_limit(&history, &revision.id))
        .ok_or_else(|| Failure::refused("graph source retry limit exceeded"))?;
    holder
        .step(&json!({
            "kind": SOURCE_STARTED,
            "revision": revision.id,
            "attempt": attempt,
        }))
        .map_err(|error| Failure::failed_by(&error))
}

/// The next permitted start number for `revision_id`, or none after its retry limit.
fn next_source_attempt(events: &[Event], revision_id: &str) -> Option<usize> {
    let prior_attempts = source_starts(events)
        .filter(|event| event.data.get("revision").and_then(Value::as_str) == Some(revision_id))
        .count();
    (prior_attempts <= MAX_SOURCE_RETRIES).then_some(prior_attempts + 1)
}

/// Limits restarts across the current job's lease-resume chain.
fn within_retry_limit(events: &[Event], revision_id: &str) -> bool {
    let starts: Vec<_> = source_starts(events).collect();
    let distinct_revisions = starts
        .iter()
        .filter_map(|event| event.data.get("revision").and_then(Value::as_str))
        .chain(iter::once(revision_id))
        .collect::<BTreeSet<_>>()
        .len();
    starts
        .len()
        .saturating_add(1)
        .saturating_sub(distinct_revisions)
        <= MAX_SOURCE_RETRIES
}

/// Selects durable source-start events from a job's event history.
fn source_starts(events: &[Event]) -> impl Iterator<Item = &Event> {
    events.iter().filter(|event| {
        event.r#type == job::PROGRESSED
            && event.data.get("kind").and_then(Value::as_str) == Some(SOURCE_STARTED)
    })
}

/// Preserve failure classification in the durable job outcome for followers.
pub(super) fn outcome(result: Result<(), Failure>) -> (JobState, Value) {
    match result {
        Ok(()) => (JobState::Succeeded, json!({})),
        Err(error) => (
            JobState::Failed,
            json!({"refused": matches!(error, Failure::Refused(_)), "error": error.to_string()}),
        ),
    }
}

/// A follower reports the same terminal refusal/failure as the worker.
pub(super) fn check(job: &Job) -> Result<(), Failure> {
    if job.state == JobState::Succeeded {
        return Ok(());
    }
    let outcome = job.outcome.as_ref();
    let message = outcome
        .and_then(|value| value["error"].as_str())
        .unwrap_or("graph job did not succeed");
    if outcome.is_some_and(|value| value["refused"] == true) {
        Err(Failure::refused(message))
    } else {
        Err(Failure::failed(message))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Event, MAX_SOURCE_RETRIES, SOURCE_STARTED, Ulid, check_token_budget, job,
        next_source_attempt, within_retry_limit,
    };
    use crate::{
        cli::graph::tests::runner_tests::support::{extractor, fixture},
        failure::Failure,
    };
    use maestro_kernel::artifact::Digest;
    use maestro_knowledge::graph::rules::TableRule;
    use serde_json::json;
    use std::slice;

    fn progress(stream: &str, sequence: u64, revision: &str) -> Event {
        Event {
            id: Ulid::nil(),
            stream: stream.to_owned(),
            sequence,
            r#type: job::PROGRESSED.to_owned(),
            subject: stream.to_owned(),
            scope: "workspace/default/collection/test".to_owned(),
            time: "2026-01-01T00:00:00Z".to_owned(),
            data: json!({"kind": SOURCE_STARTED, "revision": revision}),
        }
    }

    #[test]
    fn retry_budget_is_shared_by_all_source_restarts_in_one_job() {
        let events = [
            progress("job/test", 1, "revision-a"),
            progress("job/test", 2, "revision-b"),
            progress("job/test", 3, "revision-a"),
        ];
        assert_eq!(MAX_SOURCE_RETRIES, 1);
        assert!(!within_retry_limit(&events, "revision-b"));
        assert!(!within_retry_limit(&events, "revision-a"));
        assert_eq!(next_source_attempt(&events, "revision-a"), None);
        assert_eq!(next_source_attempt(&events, "revision-c"), Some(1));

        let resumed = [
            progress("job/test", 1, "revision-a"),
            progress("job/test", 2, "revision-a"),
        ];
        assert!(within_retry_limit(&resumed, "revision-b"));
        assert_eq!(next_source_attempt(&resumed, "revision-a"), None);
    }

    #[test]
    fn check_token_budget_accepts_exact_limit_and_returns_estimate() {
        let fixture = fixture(&["Source text.\n"]);
        let (extractor, _, _, _) = extractor(&fixture, 1025);
        assert_eq!(
            check_token_budget(&fixture.kernel.database, &fixture.revisions, &extractor).unwrap(),
            Some(1025)
        );
    }

    #[test]
    fn check_token_budget_refuses_one_token_over_limit() {
        let fixture = fixture(&["Source text.\n"]);
        let (extractor, _, _, _) = extractor(&fixture, 1024);
        let result = check_token_budget(&fixture.kernel.database, &fixture.revisions, &extractor);
        assert!(matches!(result, Err(Failure::Refused(message))
        if message == "graph extraction token budget exceeded (1025 > 1024)"));
    }

    #[test]
    fn check_token_budget_accumulates_individually_fitting_sources() {
        let fixture = fixture(&["First source.\n", "Second source.\n"]);
        let (extractor, _, _, _) = extractor(&fixture, 1025);
        for revision in &fixture.revisions {
            assert_eq!(
                check_token_budget(
                    &fixture.kernel.database,
                    slice::from_ref(revision),
                    &extractor
                )
                .unwrap(),
                Some(1025)
            );
        }
        let result = check_token_budget(&fixture.kernel.database, &fixture.revisions, &extractor);
        assert!(matches!(result, Err(Failure::Refused(message))
        if message == "graph extraction token budget exceeded (2050 > 1025)"));
    }

    #[test]
    fn check_token_budget_refuses_unreadable_source() {
        let mut fixture = fixture(&["Source text.\n"]);
        fixture.revisions[0].canonical_digest = Digest::of(b"missing artifact");
        let (extractor, _, _, _) = extractor(&fixture, 1025);
        let result = check_token_budget(&fixture.kernel.database, &fixture.revisions, &extractor);
        assert!(matches!(result, Err(Failure::Refused(message))
        if message == "graph source cannot be read for token accounting"));
    }

    #[test]
    fn check_token_budget_refuses_unavailable_estimate() {
        let oversized = "x".repeat(4096 * 9);
        let fixture = fixture(&[&oversized]);
        let (extractor, _, _, _) = extractor(&fixture, 1025);
        let result = check_token_budget(&fixture.kernel.database, &fixture.revisions, &extractor);
        assert!(matches!(result, Err(Failure::Refused(message))
        if message == "graph token estimate is unavailable"));
    }

    #[test]
    fn check_token_budget_without_budget_does_not_read_sources() {
        let mut fixture = fixture(&["Source text.\n"]);
        fixture.revisions[0].canonical_digest = Digest::of(b"missing artifact");
        let rule = TableRule::parse(
            &json!({
                "schema": "maestro-graph-table-rule/1",
                "id": "synthetic",
                "source_sha256": Digest::of(b"synthetic"),
                "heading_path": ["Defaults"],
                "columns": ["Name", "Type", "Value"],
                "subject_column": "Name",
                "type_column": "Type",
                "lexeme_column": "Value",
                "subject_kind": "Parameter",
                "predicate": "DEFAULTS_TO"
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            check_token_budget(&fixture.kernel.database, &fixture.revisions, &rule).unwrap(),
            None
        );
    }
}
