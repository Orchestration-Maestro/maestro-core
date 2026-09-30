//! Graph attachments: a finished build's frozen claim set bound to one
//! unpublished generation of its collection, once, so that a published
//! generation's pins never observe claims of a later build.

use super::support::{
    Scratch, TERM, at, build_job, execute, granted, label, on, plan, retries, second_revision,
    timing,
};
use crate::{
    artifact::Digest,
    facts::{Batch, Claim, Error, ProjectionReceipt},
    generation::NewGeneration,
    job::{self, Lease},
    scope::ScopeSet,
    store::Database,
};
use ulid::Ulid;

/// A new building generation of the collection `collection`, whose chunk
/// set `<collection>-set` is recorded complete first if it is not yet.
fn generation(database: &Database, collection: &str) -> i64 {
    execute(
        database,
        &format!(
            "INSERT INTO chunk_sets (id, collection_id, chunk_profile,
               counter_contract_id, state)
             SELECT '{collection}-set', '{collection}', 'structural/1', 'native', 'building'
             WHERE NOT EXISTS (SELECT 1 FROM chunk_sets WHERE id = '{collection}-set')"
        ),
    )
    .unwrap();
    database
        .create_generation(&NewGeneration {
            collection_id: collection.to_owned(),
            chunk_set_id: format!("{collection}-set"),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap()
        .id
}

/// Runs a whole build of `rev-a` alone, whose one batch holds `claims`,
/// under a lease of `holder`, and returns its lease.
fn finished(database: &Database, claims: Vec<Claim>, holder: &str) -> Lease {
    let all = ScopeSet::default_workspace();
    let mut plan = plan(&["rev-a"], 10, 0);
    plan.provenance = claims.first().unwrap().provenance.clone();
    let mut lease: Lease = build_job(database, &plan, holder);
    database.begin_graph_build(&all, &lease, &plan).unwrap();
    let batch = Batch {
        ordinal: 0,
        claims,
        rejections: Vec::new(),
    };
    database
        .record_graph_batch(&all, &mut lease, timing(1), &batch)
        .unwrap();
    database
        .finish_graph_build(&all, &mut lease, timing(2))
        .unwrap();
    lease
}

#[test]
fn a_generation_gets_one_finished_claim_set_before_it_is_published() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let old = finished(&database, vec![label()], "worker-1");
    let pinned = generation(&database, "graph");
    let attachment = database
        .attach_claim_set(
            &all,
            pinned,
            old.job,
            &attachment_lease(&database, old.job, pinned),
        )
        .unwrap();
    assert_eq!(attachment.generation_id, pinned);
    assert_eq!(attachment.job, old.job);
    let set = database
        .graph_build(&all, old.job)
        .unwrap()
        .unwrap()
        .claim_set_id;
    assert_eq!(Some(attachment.claim_set_id.clone()), set);
    assert_eq!(
        database
            .attach_claim_set(
                &all,
                pinned,
                old.job,
                &attachment_lease(&database, old.job, pinned)
            )
            .unwrap(),
        attachment,
        "attaching the same set again changes nothing"
    );
    database.verify_generation(pinned, 1).unwrap();
    database
        .record_projection_ready(
            &all,
            &ProjectionReceipt {
                collection_id: "graph".to_owned(),
                generation_id: pinned,
                claim_set_id: attachment.claim_set_id.clone(),
                file_name: format!("projection-{pinned}.db"),
                schema_version: "maestro-typed-edges/1".to_owned(),
                knowledge_edge_count: 0,
                entity_fact_count: 1,
                content_digest: Digest::of(b"verified projection"),
            },
        )
        .unwrap();
    database.publish_generation(pinned).unwrap();

    let mut later_claims = vec![label(), retries()];
    for claim in &mut later_claims {
        claim.provenance.profile = Digest::of(b"second extraction profile");
    }
    let new = finished(&database, later_claims, "worker-2");
    assert!(matches!(
        database.attach_claim_set(
            &all,
            pinned,
            new.job,
            &attachment_lease(&database, new.job, pinned)
        ),
        Err(Error::Conflict(_))
    ));
    let next = generation(&database, "graph");
    let later = database
        .attach_claim_set(
            &all,
            next,
            new.job,
            &attachment_lease(&database, new.job, next),
        )
        .unwrap();
    assert_ne!(later.claim_set_id, attachment.claim_set_id);
    assert_eq!(
        database.graph_attachment(&all, pinned).unwrap(),
        Some(attachment),
        "the old pin still sees only its own claims"
    );
    let unpublished = generation(&database, "graph");
    database.verify_generation(unpublished, 1).unwrap();
    assert!(
        database
            .attach_claim_set(
                &all,
                unpublished,
                new.job,
                &attachment_lease(&database, new.job, unpublished)
            )
            .is_ok()
    );
}

#[test]
fn only_a_finished_build_of_the_generations_collection_is_attached() {
    let scratch = Scratch::new();
    let database = scratch.open();
    second_revision(&database);
    let all = ScopeSet::default_workspace();
    let plan = plan(&["rev-a", "rev-b"], 10, 0);
    let lease = build_job(&database, &plan, "worker-1");
    database.begin_graph_build(&all, &lease, &plan).unwrap();
    let target = generation(&database, "graph");
    assert!(matches!(
        database.attach_claim_set(
            &all,
            target,
            lease.job,
            &attachment_lease(&database, lease.job, target)
        ),
        Err(Error::Unfinished { .. })
    ));
    assert!(matches!(
        database.attach_claim_set(
            &all,
            target,
            Ulid::nil(),
            &Lease {
                job: Ulid::nil(),
                ..lease.clone()
            }
        ),
        Err(Error::UnknownBuild(_))
    ));
    let done = finished(&database, vec![on("rev-a", label())], "worker-2");
    let foreign = generation(&database, "other");
    assert!(matches!(
        database.attach_claim_set(
            &all,
            foreign,
            done.job,
            &attachment_lease(&database, done.job, foreign)
        ),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        database.attach_claim_set(
            &all,
            999,
            done.job,
            &attachment_lease(&database, done.job, 999)
        ),
        Err(Error::Conflict(_))
    ));
    let outsider = granted(&database, "alice", "workspace/default/collection/other");
    assert!(matches!(
        database.attach_claim_set(&outsider, target, done.job, &done),
        Err(Error::UnknownBuild(_))
    ));
    assert_eq!(database.graph_attachment(&all, target).unwrap(), None);
    database
        .attach_claim_set(
            &all,
            target,
            done.job,
            &attachment_lease(&database, done.job, target),
        )
        .unwrap();
    assert_eq!(database.graph_attachment(&outsider, target).unwrap(), None);
    for statement in [
        "DELETE FROM graph_attachments",
        "UPDATE graph_attachments SET attached_at = 'now'",
        "INSERT OR REPLACE INTO graph_attachments (generation_id, job_id, claim_set_id)
         SELECT generation_id, job_id, claim_set_id FROM graph_attachments",
    ] {
        assert!(execute(&database, statement).is_err(), "{statement}");
    }
}

#[test]
fn a_stale_holder_cannot_attach_after_takeover() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let build = finished(&database, vec![label()], "builder");
    database
        .complete_job(&build, job::JobState::Succeeded, &serde_json::json!({}))
        .unwrap();
    let target = generation(&database, "graph");
    let stale = attachment_lease(&database, build.job, target);

    let current = database
        .take_job(stale.job, "worker-2", at(120), TERM)
        .unwrap();
    let late = database.attach_claim_set(&all, target, build.job, &stale);
    assert!(
        matches!(late, Err(Error::Job(job::Error::Lost { number: 1, .. }))),
        "{late:?}"
    );
    assert_eq!(database.graph_attachment(&all, target).unwrap(), None);
    let attachment = database
        .attach_claim_set(&all, target, build.job, &current)
        .unwrap();
    assert_eq!(attachment.job, build.job);
    assert_eq!(
        database.graph_attachment(&all, target).unwrap(),
        Some(attachment)
    );
}

#[test]
fn a_completed_build_attaches_under_a_separate_job_lease() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let build = finished(&database, vec![label()], "builder");
    database
        .complete_job(&build, job::JobState::Succeeded, &serde_json::json!({}))
        .unwrap();
    let target = generation(&database, "graph");
    let lease = attachment_lease(&database, build.job, target);
    let attached = database
        .attach_claim_set(&all, target, build.job, &lease)
        .unwrap();
    assert_eq!(attached.job, build.job);
}

/// A distinct attachment job in the collection, not a reopened terminal build.
fn attachment_lease(database: &Database, build: Ulid, target: i64) -> Lease {
    let inputs = serde_json::json!({"build": build.to_string(), "generation": target});
    let scope = "workspace/default/collection/graph".parse().unwrap();
    let new = job::NewJob {
        kind: "knowledge.graph.attach",
        inputs: &inputs,
        scope: &scope,
        resource: None,
    };
    let attach = database.submit_job(&new, at(3)).unwrap();
    attach.lease.unwrap_or_else(|| {
        database
            .take_job(attach.id, "attachment", at(3), TERM)
            .unwrap()
    })
}

#[test]
fn an_attachment_lease_of_another_collection_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let build = finished(&database, vec![label()], "builder");
    let target = generation(&database, "graph");
    let scope = "workspace/default/collection/other".parse().unwrap();
    let inputs = serde_json::json!({"build": build.job.to_string(), "generation": target});
    let job = database
        .submit_job(
            &job::NewJob {
                kind: "knowledge.graph.attach",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            at(3),
        )
        .unwrap();
    let lease = database.take_job(job.id, "foreign", at(3), TERM).unwrap();
    assert!(matches!(
        database.attach_claim_set(&all, target, build.job, &lease),
        Err(Error::Unauthorized)
    ));
    assert!(database.graph_attachment(&all, target).unwrap().is_none());
}

#[test]
fn an_attachment_lease_must_name_the_exact_build_and_generation() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let build = finished(&database, vec![label()], "builder");
    let target = generation(&database, "graph");
    let wrong_build = attachment_lease(&database, Ulid::nil(), target);
    let wrong_generation = attachment_lease(&database, build.job, target + 1);
    for lease in [&build, &wrong_build, &wrong_generation] {
        assert!(
            matches!(
                database.attach_claim_set(&all, target, build.job, lease),
                Err(Error::Unauthorized)
            ),
            "unrelated lease authorized attachment: {lease:?}"
        );
        assert!(database.graph_attachment(&all, target).unwrap().is_none());
    }
}
