//! End-to-end selection and retry of journaled publication attempts.

use super::*;

#[test]
fn selected_inputs_does_not_adopt_a_failed_job_with_a_different_frozen_tuple() {
    for (field, value) in [
        ("collection", json!("other")),
        ("chunk_set", json!("other-set")),
        ("card", json!("other-card")),
        ("identifier_profile", json!("other-identifiers")),
        ("sparse_profile", json!("other-sparse")),
    ] {
        let fixture = fixture();
        let mut inputs = publish::recovery_inputs(
            fixture.card.digest().as_str(),
            CHUNK_SET,
            COLLECTION,
            RebuildGuard {
                expected_published: Some(fixture.original),
                generation_watermark: fixture.original,
            },
            Ulid::generate(),
        );
        inputs[field] = value;
        let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_001);
        let job = fixture
            .kernel
            .database
            .submit_job(
                &NewJob {
                    kind: KIND,
                    inputs: &inputs,
                    scope: &scope,
                    resource: Some(RESOURCE),
                },
                now,
            )
            .unwrap();
        let lease = fixture
            .kernel
            .database
            .take_job(job.id, "different-tuple", now, Duration::from_secs(60))
            .unwrap();
        fixture
            .kernel
            .database
            .complete_job(&lease, JobState::Failed, &json!({"error": "old tuple"}))
            .unwrap();

        let (fresh, recovery, resuming) = selected(&fixture, true).unwrap();
        assert_ne!(fresh["again"], inputs["again"]);
        assert_eq!(fresh["card"], fixture.card.digest().as_str());
        assert_eq!(
            recovery,
            Some(RebuildGuard {
                expected_published: Some(fixture.original),
                generation_watermark: fixture.original,
            })
        );
        assert!(!resuming);
    }
}

#[test]
fn selected_inputs_freezes_a_new_recovery_but_keeps_ordinary_inputs_unchanged() {
    let fixture = fixture();
    let (recovery_inputs, recovery, resuming) = selected(&fixture, true).unwrap();
    assert_eq!(recovery_inputs["expected_published"], fixture.original);
    assert_eq!(recovery_inputs["generation_watermark"], fixture.original);
    assert_eq!(recovery_inputs["card"], fixture.card.digest().as_str());
    assert_eq!(recovery_inputs["chunk_set"], CHUNK_SET);
    assert!(Ulid::from_string(recovery_inputs["again"].as_str().unwrap()).is_ok());
    assert_eq!(
        recovery,
        Some(RebuildGuard {
            expected_published: Some(fixture.original),
            generation_watermark: fixture.original,
        })
    );
    assert!(!resuming);

    let (ordinary, guard, resuming) = selected(&fixture, false).unwrap();
    assert_eq!(
        ordinary,
        publish::frozen_inputs(fixture.card.digest().as_str(), CHUNK_SET, COLLECTION)
    );
    assert_eq!(guard, None);
    assert!(!resuming);
}

#[test]
fn selected_inputs_resubmits_a_failed_attempt_with_its_verified_target_and_progress() {
    let fixture = fixture();
    let target = generation(&fixture, GenerationState::Verified);
    let (failed, original_inputs, progress) = attempt(
        &fixture,
        original_guard(&fixture),
        target.id,
        JobState::Failed,
        1,
    );

    let (inputs, recovery, resuming) = selected(&fixture, true).unwrap();
    assert_eq!(inputs, original_inputs);
    assert_eq!(
        recovery,
        Some(RebuildGuard {
            expected_published: Some(fixture.original),
            generation_watermark: fixture.original,
        })
    );
    assert!(resuming);
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let retry = fixture
        .kernel
        .database
        .submit_job(
            &NewJob {
                kind: KIND,
                inputs: &inputs,
                scope: &scope,
                resource: Some(RESOURCE),
            },
            SystemTime::UNIX_EPOCH + Duration::from_secs(10_002),
        )
        .unwrap();
    assert_eq!(retry.attempt, 2);
    assert_ne!(retry.id, failed.id);
    assert_eq!(
        publish::last_progress(&fixture.kernel, retry.id).unwrap(),
        Some(progress)
    );
}

#[test]
fn selected_inputs_reconciles_an_active_job_whose_own_target_was_published() {
    let fixture = fixture();
    let target = generation(&fixture, GenerationState::Published);
    let (active, original_inputs, _) = attempt(
        &fixture,
        original_guard(&fixture),
        target.id,
        JobState::Running,
        1,
    );

    let (inputs, recovery, resuming) = selected(&fixture, true).unwrap();
    assert_eq!(inputs, original_inputs);
    assert_eq!(
        recovery,
        Some(RebuildGuard {
            expected_published: Some(fixture.original),
            generation_watermark: fixture.original,
        })
    );
    assert!(resuming);
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let found = fixture
        .kernel
        .database
        .submit_job(
            &NewJob {
                kind: KIND,
                inputs: &inputs,
                scope: &scope,
                resource: Some(RESOURCE),
            },
            SystemTime::UNIX_EPOCH + Duration::from_secs(10_002),
        )
        .unwrap();
    assert_eq!(found.id, active.id);
    assert_eq!(found.state, JobState::Running);
}

#[test]
fn selected_inputs_refuses_a_failed_target_after_an_unrelated_publish_and_names_it() {
    let fixture = fixture();
    let target = generation(&fixture, GenerationState::Verified);
    let (failed, _, _) = attempt(
        &fixture,
        original_guard(&fixture),
        target.id,
        JobState::Failed,
        1,
    );
    let _unrelated = generation(&fixture, GenerationState::Published);

    let error = selected(&fixture, true).unwrap_err();
    assert!(matches!(error, Failure::Refused(_)));
    assert!(error.to_string().contains(&failed.id.to_string()));
    assert!(error.to_string().contains("generation 1"));
    assert!(!error.to_string().contains("Some("));
}

#[test]
fn several_failed_attempts_for_the_tuple_are_refused_and_named() {
    let fixture = fixture();
    let first_target = generation(&fixture, GenerationState::Verified);
    let second_target = generation(&fixture, GenerationState::Verified);
    let guard = original_guard(&fixture);
    let (first, _, _) = attempt(&fixture, guard, first_target.id, JobState::Failed, 1);
    let (second, _, _) = attempt(&fixture, guard, second_target.id, JobState::Failed, 2);

    let error = selected(&fixture, true).unwrap_err();
    assert!(matches!(error, Failure::Refused(_)));
    assert!(error.to_string().contains(&first.id.to_string()));
    assert!(error.to_string().contains(&second.id.to_string()));
}

#[test]
fn selected_inputs_does_not_reuse_an_older_failed_attempt_after_success() {
    let fixture = fixture();
    let target = generation(&fixture, GenerationState::Verified);
    let (failed, original_inputs, _) = attempt(
        &fixture,
        original_guard(&fixture),
        target.id,
        JobState::Failed,
        1,
    );
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let retry_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10_002);
    let retry = fixture
        .kernel
        .database
        .submit_job(
            &NewJob {
                kind: KIND,
                inputs: &original_inputs,
                scope: &scope,
                resource: Some(RESOURCE),
            },
            retry_time,
        )
        .unwrap();
    let lease = fixture
        .kernel
        .database
        .take_job(
            retry.id,
            "successful-retry",
            retry_time,
            Duration::from_secs(60),
        )
        .unwrap();
    fixture
        .kernel
        .database
        .publish_generation_if_current(target.id, Some(fixture.original))
        .unwrap();
    fixture
        .kernel
        .database
        .complete_job(
            &lease,
            JobState::Succeeded,
            &json!({"generation": target.id}),
        )
        .unwrap();
    assert_eq!(retry.attempt, 2);
    assert_ne!(retry.id, failed.id);

    let (inputs, recovery, resuming) = selected(&fixture, true).unwrap();
    assert_ne!(inputs["again"], original_inputs["again"]);
    assert_eq!(
        recovery,
        Some(RebuildGuard {
            expected_published: Some(target.id),
            generation_watermark: target.id,
        })
    );
    assert!(!resuming);
}
