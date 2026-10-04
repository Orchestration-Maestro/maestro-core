//! Independent loader admission, successor and publication guard proofs.
#![cfg(unix)]
use super::{
    loader_tests::{fixture, snapshot, staging},
    public_fixture::now,
};
use crate::graph::projection::{
    checkpoint::{Journal, Manifest, prefix},
    content,
};
use maestro_kernel::artifact::Digest;
use std::fs;

#[test]
fn loader_incorrect_uncertain_successor_refuses_without_completion_writes() {
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
    Journal::create(
        &staging(&fixture),
        Manifest::expected(&fixture.build, &snapshot).unwrap(),
    )
    .unwrap();
    let mut edges = snapshot.edges[..64].to_vec();
    edges[0].target = Digest::of(b"incorrect successor target");
    producer.write_batch(&edges, &[]).unwrap();
    let before = producer.verify().unwrap();
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
    assert!(resumed.load(&snapshot).is_err());
    assert_eq!(resumed.verify().unwrap(), before);
    assert_eq!(
        fs::read_dir(staging(&fixture).join("loader"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn loader_publication_independently_requires_last_checkpoint_and_manifest_digest() {
    for missing in [true, false] {
        let fixture = fixture(65);
        let mut snapshot = snapshot(&fixture);
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
        if !missing {
            snapshot.edges[0].target = Digest::of(b"same counts wrong content");
        }
        producer.load(&snapshot).unwrap();
        let expected = producer.verify().unwrap();
        if missing {
            fs::remove_file(staging(&fixture).join("loader/0000000002.json")).unwrap();
        } else {
            // Native and authoritative counts are unchanged; only the held manifest differs.
            let original = super::loader_tests::snapshot(&fixture);
            producer.session.journal =
                Some(Journal::open(&staging(&fixture), &fixture.build, 64 * 1024 * 1024).unwrap());
            let journal = producer.session.journal.as_mut().unwrap();
            journal.manifest = Manifest::expected(&fixture.build, &original).unwrap();
            fs::write(
                staging(&fixture).join("loader/manifest.json"),
                serde_json::to_vec(&journal.manifest).unwrap(),
            )
            .unwrap();
            journal.validate_checkpoint(&snapshot, 1).unwrap();
            journal.validate_checkpoint(&snapshot, 2).unwrap();
        }
        assert!(producer.publish(&expected).is_err());
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
fn loader_receiptless_final_and_missing_native_preserve_directory_entries() {
    for final_exists in [true, false] {
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
        let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
        let native = staging(&fixture).join(&name);
        if final_exists {
            fs::copy(&native, fixture.native.path.join(&name)).unwrap();
        } else {
            fs::remove_file(&native).unwrap();
        }
        let before: Vec<_> = fs::read_dir(staging(&fixture))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let manifest = fs::read(staging(&fixture).join("loader/manifest.json")).unwrap();
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
        let after: Vec<_> = fs::read_dir(staging(&fixture))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(after, before);
        assert_eq!(
            fs::read(staging(&fixture).join("loader/manifest.json")).unwrap(),
            manifest
        );
    }
}

#[test]
fn loader_cleanup_removes_highest_checkpoint_first_and_manifest_last() {
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
    producer
        .session
        .journal
        .as_mut()
        .unwrap()
        .fail_cleanup_after = Some(1);
    assert!(producer.publish(&prefix(&snapshot, 2).unwrap()).is_err());
    let path = staging(&fixture).join("loader");
    assert!(!path.join("0000000002.json").exists());
    assert!(path.join("0000000001.json").exists());
    assert!(path.join("manifest.json").exists());
}
