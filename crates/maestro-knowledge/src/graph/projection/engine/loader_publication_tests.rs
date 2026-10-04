//! Refusal matrix for the one native checkpointed loader.
#![cfg(unix)]
use super::{
    loader_tests::{fixture, snapshot, staging},
    public_fixture::now,
};
use crate::graph::projection::checkpoint::prefix;
use maestro_kernel::{artifact::Digest, facts::PROJECTION_REBUILD_REPAIR};
use std::fs;
#[test]
fn loader_changed_records_refuse_direct_publication() {
    for fault in ["digest", "ids", "manifest"] {
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
        if fault == "manifest" {
            fs::write(path.join("manifest.json"), b"{").unwrap();
        } else {
            let file = path.join("0000000001.json");
            let mut record: serde_json::Value =
                serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
            if fault == "digest" {
                record["verified"][2] = Digest::of(b"wrong digest").as_str().into();
            } else {
                record["ids"][0] = Digest::of(b"wrong ID").as_str().into();
            }
            fs::write(file, serde_json::to_vec(&record).unwrap()).unwrap();
        }
        let before: Vec<_> = fs::read_dir(&path)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect();
        let refused = producer.publish(&prefix(&snapshot, 2).unwrap()).map(|_| ());
        let error = refused.unwrap_err();
        assert!(
            format!("{error:?}").contains(PROJECTION_REBUILD_REPAIR),
            "{fault}: {error:?}"
        );
        assert!(path.exists());
        for (name, bytes) in before {
            assert_eq!(fs::read(path.join(name)).unwrap(), bytes);
        }
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
