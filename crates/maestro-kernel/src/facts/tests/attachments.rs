//! Graph attachments: a finished build's frozen claim set bound to one
//! unpublished generation of its collection, once, so that a published
//! generation's pins never observe claims of a later build.

use super::support::{
    Scratch, TERM, at, build_job, execute, granted, label, on, plan, retries, second_revision,
    timing,
};
use crate::{
    artifact::Digest,
    facts::{Batch, Claim, Error},
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
    let attachment = database.attach_claim_set(&all, pinned, &old).unwrap();
    assert_eq!(attachment.generation_id, pinned);
    assert_eq!(attachment.job, old.job);
    let set = database
        .graph_build(&all, old.job)
        .unwrap()
        .unwrap()
        .claim_set_id;
    assert_eq!(Some(attachment.claim_set_id.clone()), set);
    assert_eq!(
        database.attach_claim_set(&all, pinned, &old).unwrap(),
        attachment,
        "attaching the same set again changes nothing"
    );
    database.verify_generation(pinned, 1).unwrap();
    database.publish_generation(pinned).unwrap();

    let mut later_claims = vec![label(), retries()];
    for claim in &mut later_claims {
        claim.provenance.profile = Digest::of(b"second extraction profile");
    }
    let new = finished(&database, later_claims, "worker-2");
    assert!(matches!(
        database.attach_claim_set(&all, pinned, &new),
        Err(Error::Conflict(_))
    ));
    let next = generation(&database, "graph");
    let later = database.attach_claim_set(&all, next, &new).unwrap();
    assert_ne!(later.claim_set_id, attachment.claim_set_id);
    assert_eq!(
        database.graph_attachment(&all, pinned).unwrap(),
        Some(attachment),
        "the old pin still sees only its own claims"
    );
    let unpublished = generation(&database, "graph");
    database.verify_generation(unpublished, 1).unwrap();
    assert!(database.attach_claim_set(&all, unpublished, &new).is_ok());
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
        database.attach_claim_set(&all, target, &lease),
        Err(Error::Unfinished { .. })
    ));
    assert!(matches!(
        database.attach_claim_set(
            &all,
            target,
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
        database.attach_claim_set(&all, foreign, &done),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        database.attach_claim_set(&all, 999, &done),
        Err(Error::Conflict(_))
    ));
    let outsider = granted(&database, "alice", "workspace/default/collection/other");
    assert!(matches!(
        database.attach_claim_set(&outsider, target, &done),
        Err(Error::UnknownBuild(_))
    ));
    assert_eq!(database.graph_attachment(&all, target).unwrap(), None);
    database.attach_claim_set(&all, target, &done).unwrap();
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
    let stale = finished(&database, vec![label()], "worker-1");
    let current = database
        .take_job(stale.job, "worker-2", at(120), TERM)
        .unwrap();
    let target = generation(&database, "graph");
    let late = database.attach_claim_set(&all, target, &stale);
    assert!(
        matches!(late, Err(Error::Job(job::Error::Lost { number: 1, .. }))),
        "{late:?}"
    );
    assert_eq!(database.graph_attachment(&all, target).unwrap(), None);
    let attachment = database.attach_claim_set(&all, target, &current).unwrap();
    assert_eq!(attachment.job, current.job);
    assert_eq!(
        database.graph_attachment(&all, target).unwrap(),
        Some(attachment)
    );
}
