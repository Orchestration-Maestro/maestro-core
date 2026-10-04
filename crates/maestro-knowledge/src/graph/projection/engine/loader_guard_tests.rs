//! Refusal matrix for the one native checkpointed loader.
#![cfg(unix)]
use super::{
    loader_tests::{fixture, snapshot, staging},
    public_fixture::now,
};
use crate::graph::projection::{
    TypedEdgeProjection,
    checkpoint::{Journal, Manifest, prefix},
};
use maestro_kernel::{artifact::Digest, facts::PROJECTION_REBUILD_REPAIR};
use std::fs;
#[test]
fn loader_corrupt_records_gaps_unknown_duplicates_and_durable_mismatch_refuse() {
    for fault in [
        "manifest",
        "schema",
        "gap",
        "unknown",
        "duplicate",
        "unparsable",
        "digest",
        "ids",
        "order",
        "directory",
        "durable",
    ] {
        let fixture = fixture(65);
        let snapshot = snapshot(&fixture);
        let clock = || now(0);
        let mut producer = fixture
            .factory()
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock,
            )
            .unwrap();
        producer.load(&snapshot).unwrap();
        let path = staging(&fixture).join("loader");
        match fault {
            "manifest" => fs::write(path.join("manifest.json"), b"{").unwrap(),
            "schema" => {
                let bytes = fs::read_to_string(path.join("manifest.json")).unwrap();
                fs::write(
                    path.join("manifest.json"),
                    bytes.replace("graph-loader/1", "graph-loader/9"),
                )
                .unwrap();
            }
            "gap" => fs::remove_file(path.join("0000000001.json")).unwrap(),
            "unknown" => {
                fs::remove_file(path.join("0000000002.json")).unwrap();
                fs::write(path.join("unknown"), b"{}").unwrap();
            }
            "duplicate" => {
                fs::remove_file(path.join("0000000002.json")).unwrap();
                fs::copy(path.join("0000000001.json"), path.join("1.json")).unwrap();
            }
            "unparsable" => fs::write(path.join("0000000001.json"), b"{").unwrap(),
            "digest" | "ids" | "order" => {
                let file = path.join("0000000001.json");
                let mut record: serde_json::Value =
                    serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
                match fault {
                    "digest" => record["verified"][2] = Digest::of(b"wrong digest").as_str().into(),
                    "ids" => record["ids"][0] = Digest::of(b"wrong ID").as_str().into(),
                    _ => record["ordinal"] = 2.into(),
                }
                fs::write(file, serde_json::to_vec(&record).unwrap()).unwrap();
            }
            "directory" => {
                fs::remove_file(path.join("0000000002.json")).unwrap();
                fs::create_dir(path.join("0000000002.json")).unwrap();
            }
            "durable" => {
                let mut extra = snapshot.edges[0].clone();
                extra.id = Digest::of(b"unexpected durable edge");
                producer.write_batch(&[extra], &[]).unwrap();
            }
            _ => panic!(),
        }
        drop(producer);
        let refused = fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock,
            )
            .and_then(|mut producer| producer.load(&snapshot));
        let error = refused.unwrap_err();
        assert!(
            format!("{error:?}").contains(PROJECTION_REBUILD_REPAIR),
            "{fault}: {error:?}"
        );
        assert!(path.exists());
        assert!(
            fixture
                .authority
                .database
                .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn loader_changed_snapshot_pins_and_unrelated_build_refuse_without_writes() {
    let fixture = fixture(65);
    let mut rows = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    for pin in 0..4 {
        let mut changed = snapshot(&fixture);
        match pin {
            0 => changed.scope.collection_id = "other".into(),
            1 => changed.claim_set.id = Digest::of(b"changed set"),
            2 => changed.resolution_id = Digest::of(b"changed resolution"),
            _ => changed.resolver_version = "changed".into(),
        }
        assert!(producer.load(&changed).is_err());
        assert!(!staging(&fixture).join("loader").exists());
    }
    drop(producer);
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    // Fresh start cannot silently reuse an existing job-derived staging directory.
    assert!(
        fixture
            .factory()
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    // A loader-owned build refuses later same-pin row drift.
    fs::remove_dir(staging(&fixture).join("loader")).ok();
    let journal = Journal::create(
        &staging(&fixture),
        Manifest::expected(&fixture.build, &rows).unwrap(),
    )
    .unwrap();
    drop(journal);
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    rows.edges[0].relation = "PART_OF".into();
    assert!(resumed.load(&rows).is_err());
}

#[test]
fn loader_publish_cleanup_failure_is_visible_and_installed_reader_survives() {
    let fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.load(&snapshot).unwrap();
    producer.session.journal.as_mut().unwrap().fail_cleanup = true;
    let expected = prefix(&snapshot, 2).unwrap();
    let error = producer.publish(&expected).unwrap_err();
    assert!(format!("{error:?}").contains("projection is published"));
    let reader = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(
        reader
            .entity_facts(
                &fixture.authority.scopes,
                &fixture.build.scope,
                &snapshot.facts[0].subject
            )
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fs::read_dir(staging(&fixture).join("loader"))
            .unwrap()
            .count(),
        3
    );
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
}

#[test]
fn loader_disk_full_after_commit_preserves_uncertain_batch_for_explicit_resume() {
    let fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    let mut journal = Journal::create(
        &staging(&fixture),
        Manifest::expected(&fixture.build, &snapshot).unwrap(),
    )
    .unwrap();
    journal.fail_record = true;
    producer.session.journal = Some(journal);
    assert!(producer.load(&snapshot).is_err());
    assert_eq!(producer.verify().unwrap(), prefix(&snapshot, 1).unwrap());
    assert!(!staging(&fixture).join("loader/0000000001.json").exists());
    drop(producer);
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot).unwrap();
    assert_eq!(resumed.verify().unwrap(), prefix(&snapshot, 2).unwrap());
}

#[test]
fn loader_resume_refuses_each_changed_manifest_build_pin_and_expired_or_foreign_lease() {
    let fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.load(&snapshot).unwrap();
    drop(producer);
    let manifest_path = staging(&fixture).join("loader/manifest.json");
    let original = fs::read(&manifest_path).unwrap();
    for key in [
        "scope",
        "generation",
        "claim_set",
        "pins0",
        "pins1",
        "pins2",
        "pins3",
        "job",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
        match key {
            "scope" => value[key][0] = "foreign".into(),
            "generation" => value["scope"][1] = (fixture.build.scope.generation_id + 1).into(),
            "pins0" | "pins1" | "pins2" | "pins3" => {
                let slot: usize = key.trim_start_matches("pins").parse().unwrap();
                value["pins"][slot] = "changed pin".into();
            }
            "claim_set" => value[key] = Digest::of(b"changed claim set").as_str().into(),
            _ => value[key] = "foreign-job".into(),
        }
        fs::write(&manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            fixture
                .factory()
                .resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.clone(),
                    &clock
                )
                .is_err(),
            "{key}"
        );
        fs::write(&manifest_path, &original).unwrap();
    }
    for index in 0..5 {
        let mut changed = fixture.build.clone();
        match index {
            0 => changed.settings_identity = Digest::of(b"changed settings"),
            1 => changed.frozen_lock = Digest::of(b"changed lock"),
            2 => changed.resolution_id = Digest::of(b"changed resolution"),
            3 => changed.lease.holder = "foreign".into(),
            _ => changed.lease.number += 1,
        }
        assert!(
            fixture
                .factory()
                .resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    changed,
                    &clock
                )
                .is_err()
        );
    }
    let expired = || now(60);
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &expired
            )
            .is_err()
    );
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot).unwrap();
}

#[test]
fn loader_empty_rows_verify_but_cannot_publish_against_nonempty_authority() {
    let fixture = fixture(65);
    let mut snapshot = snapshot(&fixture);
    snapshot.edges.clear();
    snapshot.facts.clear();
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.load(&snapshot).unwrap();
    assert_eq!(
        fs::read_dir(staging(&fixture).join("loader"))
            .unwrap()
            .count(),
        1
    );
    let expected = prefix(&snapshot, 0).unwrap();
    assert_eq!(producer.verify().unwrap(), expected);
    let error = producer.publish(&expected).unwrap_err();
    assert!(
        format!("{error:?}")
            .contains("projection receipt differs from its authoritative claim set")
    );
    assert!(staging(&fixture).join("loader/manifest.json").exists());
}

#[test]
fn loader_resumed_writer_and_publish_require_validated_load_and_complete_manifest() {
    let fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.load(&snapshot).unwrap();
    drop(producer);
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    assert!(resumed.write_batch(&[], &[]).is_err());
    let expected = resumed.verify().unwrap();
    assert!(resumed.publish(&expected).is_err());
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot).unwrap();
    let mut extra = snapshot.edges[0].clone();
    extra.id = Digest::of(b"manually appended outside loader");
    resumed.write_batch(&[extra], &[]).unwrap();
    let changed = resumed.verify().unwrap();
    assert!(resumed.publish(&changed).is_err());
    assert!(
        fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap()
            .is_none()
    );
}
