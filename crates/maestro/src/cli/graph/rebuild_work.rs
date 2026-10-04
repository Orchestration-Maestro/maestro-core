//! Existing foreground lease protocol feeding only the opaque checkpointed loader.
use super::{job, rebuild_descriptors::Descriptors};
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
    artifact::Digest,
    facts::EXACT_RESOLVER_VERSION,
    job::{self as kernel_job, NewJob},
    journal::Filter,
    scope::{LOCAL, collection_path},
};
use maestro_knowledge::graph::projection::{
    EngineSettings, ProjectionBuild, ProjectionFactory, ProjectionScope, ProjectionSnapshot,
};
use serde_json::json;
use std::time::SystemTime;
use ulid::Ulid;

/// The one frozen authority selection shared with optional descriptors.
pub(super) struct Selection {
    /// Exact target; no generation or search pointer mutation.
    pub(super) scope: ProjectionScope,
    /// Attached membership.
    pub(super) claim_set: Digest,
    /// Explicit or receipt-selected frozen resolution.
    pub(super) resolution: Digest,
}
impl Selection {
    /// Construct the opaque snapshot through the existing authority reader.
    pub(super) fn read(&self, kernel: &Kernel) -> Result<ProjectionSnapshot, Failure> {
        ProjectionSnapshot::read(
            &kernel.database,
            &kernel.scopes,
            LOCAL,
            (&self.claim_set, &self.resolution),
            &self.scope,
        )
        .map_err(|error| Failure::refused_by(&error))
    }
}

/// Replaceable factory/configuration with disabled-by-default descriptors.
pub(super) struct Work<'a> {
    /// Frozen scope and pins.
    pub(super) selection: &'a Selection,
    /// Already validated rows, never re-derived by another loader.
    pub(super) snapshot: &'a ProjectionSnapshot,
    /// Existing qualified backend lifecycle.
    pub(super) factory: &'a ProjectionFactory<'a>,
    /// Already admitted settings and complete lock.
    pub(super) settings: &'a EngineSettings,
    /// Optional pre-embedded G35 output and frozen profile; pilot passes none.
    pub(super) descriptors: Option<Descriptors<'a>>,
}
impl Work<'_> {
    /// Run or follow one project job; recovery must name that same live job explicitly.
    pub(super) fn run(
        &self,
        kernel: &Kernel,
        output: Output,
        resume: Option<Ulid>,
    ) -> Result<bool, Failure> {
        let generation = self.selection.scope.generation_id;
        let inputs = json!({"generation":generation});
        if let Some(id) = resume {
            validate_resume(kernel, id, (&inputs, self.selection))?;
        }
        let scope = collection_path(&self.selection.scope.collection_id)
            .parse()
            .map_err(|error| Failure::refused_by(&error))?;
        let resource = format!("graph-project:{generation}");
        let new = NewJob {
            kind: "knowledge.graph.project",
            inputs: &inputs,
            scope: &scope,
            resource: Some(&resource),
        };
        let policy = resume.map_or(foreground::Policy::FreshOnly, foreground::Policy::Named);
        let (ended, fresh) =
            foreground::run_selected(kernel, output, (&new, policy), TIMING, |holder| {
                job::outcome(self.execute(kernel, holder, resume))
            })?;
        job::check(&ended)?;
        Ok(fresh)
    }
    /// Load and publish outside the heartbeat mutex; each native operation fences itself.
    fn execute(
        &self,
        kernel: &Kernel,
        holder: &Holder<'_>,
        resume: Option<Ulid>,
    ) -> Result<(), Failure> {
        if resume.is_some_and(|id| id != holder.job_id()) {
            return Err(Failure::refused(
                "resume job differs from selected generation",
            ));
        }
        let lease = holder.fenced(|lease, _| lease.clone());
        let build = ProjectionBuild {
            scope: self.selection.scope.clone(),
            claim_set_id: self.selection.claim_set.clone(),
            resolution_id: self.selection.resolution.clone(),
            resolver_version: EXACT_RESOLVER_VERSION.into(),
            settings_identity: self.settings.identity(),
            frozen_lock: self.settings.frozen_lock().clone(),
            lease,
        };
        let clock = SystemTime::now;
        let mut producer = if resume.is_some() {
            self.factory
                .resume(&kernel.database, &kernel.scopes, build, &clock)
        } else {
            self.factory
                .producer(&kernel.database, &kernel.scopes, build, &clock)
        }
        .map_err(|error| Failure::refused_by(&error))?;
        producer
            .load(self.snapshot)
            .map_err(|error| Failure::refused_by(&error))?;
        let verified = producer
            .verify()
            .map_err(|error| Failure::failed_by(&error))?;
        producer
            .publish(&verified)
            .map_err(|error| Failure::failed_by(&error))?;
        if let Some(descriptors) = &self.descriptors {
            descriptors.run(
                kernel,
                (
                    &self.selection.scope,
                    &self.selection.claim_set,
                    &self.selection.resolution,
                ),
            )?;
        }
        Ok(())
    }
}

/// Scope and immutable created inputs are checked before taking or submitting a job.
fn validate_resume(
    kernel: &Kernel,
    id: Ulid,
    request: (&serde_json::Value, &Selection),
) -> Result<(), Failure> {
    let (inputs, selection) = request;
    let record = kernel
        .database
        .job(&kernel.scopes, id)
        .map_err(|error| Failure::refused_by(&error))?
        .ok_or_else(|| Failure::refused("unknown or unauthorized resume job"))?;
    if record.scope.as_str() != collection_path(&selection.scope.collection_id) {
        return Err(Failure::refused(
            "resume job differs from selected collection",
        ));
    }
    if record.kind != "knowledge.graph.project"
        || !matches!(
            record.state,
            kernel_job::JobState::Queued | kernel_job::JobState::Running
        )
    {
        return Err(Failure::refused(
            "resume requires a live graph project job; terminal staging is preserved",
        ));
    }
    let history = kernel
        .database
        .events(
            &kernel.scopes,
            &Filter {
                stream: &kernel_job::stream(id),
                after: 0,
                r#type: Some(kernel_job::CREATED),
            },
        )
        .map_err(|error| Failure::refused_by(&error))?;
    if history
        .first()
        .is_none_or(|event| event.data.get("inputs") != Some(inputs))
    {
        return Err(Failure::refused(
            "resume job differs from selected generation",
        ));
    }
    Ok(())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::cli::graph::tests::rebuild_support::authority;
    use kernel_job::JobState;

    #[test]
    fn graph_rebuild_resume_preflight_refuses_unknown_foreign_kind_scope_generation_and_terminal() {
        let (fixture, selection) = authority();
        let kernel = &fixture.kernel;
        let inputs = json!({"generation":selection.scope.generation_id});
        assert!(validate_resume(kernel, Ulid::nil(), (&inputs, &selection)).is_err());
        let scope = collection_path(&selection.scope.collection_id)
            .parse()
            .unwrap();
        let foreign = "workspace/default/collection/other".parse().unwrap();
        for (kind, scope, created) in [
            ("other", &scope, inputs.clone()),
            ("knowledge.graph.project", &foreign, inputs.clone()),
            ("knowledge.graph.project", &scope, json!({"generation":-1})),
        ] {
            let job = kernel
                .database
                .submit_job(
                    &NewJob {
                        kind,
                        inputs: &created,
                        scope,
                        resource: None,
                    },
                    SystemTime::now(),
                )
                .unwrap();
            assert!(validate_resume(kernel, job.id, (&inputs, &selection)).is_err());
            assert_eq!(
                kernel
                    .database
                    .job(&kernel.scopes, job.id)
                    .unwrap()
                    .unwrap()
                    .state,
                JobState::Queued
            );
        }
        let new = NewJob {
            kind: "knowledge.graph.project",
            inputs: &inputs,
            scope: &scope,
            resource: None,
        };
        for state in [JobState::Failed, JobState::Cancelled, JobState::Succeeded] {
            let job = kernel.database.submit_job(&new, SystemTime::now()).unwrap();
            validate_resume(kernel, job.id, (&inputs, &selection)).unwrap();
            let lease = kernel
                .database
                .take_job(job.id, "test", SystemTime::now(), TIMING.term)
                .unwrap();
            validate_resume(kernel, job.id, (&inputs, &selection)).unwrap();
            kernel
                .database
                .complete_job(&lease, state, &json!({}))
                .unwrap();
            assert!(validate_resume(kernel, job.id, (&inputs, &selection)).is_err());
        }
    }
    #[test]
    fn graph_rebuild_named_policy_never_submits_or_selects_by_key() {
        let (fixture, selection) = authority();
        let kernel = &fixture.kernel;
        let inputs = json!({"generation":selection.scope.generation_id});
        let scope = collection_path(&selection.scope.collection_id)
            .parse()
            .unwrap();
        let new = NewJob {
            kind: "knowledge.graph.project",
            inputs: &inputs,
            scope: &scope,
            resource: None,
        };
        let job = kernel
            .database
            .submit_job(&new, SystemTime::UNIX_EPOCH)
            .unwrap();
        let unrelated = NewJob {
            kind: "must.not.submit",
            ..new
        };
        let (ended, fresh) = foreground::run_selected(
            kernel,
            Output::new(true),
            (&unrelated, foreground::Policy::Named(job.id)),
            TIMING,
            |holder| {
                assert_eq!(holder.job_id(), job.id);
                (JobState::Succeeded, json!({}))
            },
        )
        .unwrap();
        assert_eq!(ended.id, job.id);
        assert!(fresh);
        assert_eq!(ended.state, JobState::Succeeded);
    }

    #[test]
    fn graph_rebuild_fresh_policy_never_supersedes_held_resource() {
        let (fixture, selection) = authority();
        let kernel = &fixture.kernel;
        let scope = collection_path(&selection.scope.collection_id)
            .parse()
            .unwrap();
        let inputs = json!({"generation":selection.scope.generation_id});
        let new = NewJob {
            kind: "knowledge.graph.project",
            inputs: &inputs,
            scope: &scope,
            resource: Some("shared"),
        };
        let job = kernel
            .database
            .submit_job(&new, SystemTime::UNIX_EPOCH)
            .unwrap();
        let changed = json!({"generation":-1});
        let request = NewJob {
            inputs: &changed,
            ..new
        };
        let refused = foreground::run_selected(
            kernel,
            Output::new(true),
            (&request, foreground::Policy::FreshOnly),
            TIMING,
            |_| panic!("held resource must never run"),
        )
        .unwrap_err();
        assert!(matches!(refused, Failure::Refused(_)));
        assert_eq!(
            kernel
                .database
                .job(&kernel.scopes, job.id)
                .unwrap()
                .unwrap()
                .state,
            JobState::Queued
        );
    }
}
