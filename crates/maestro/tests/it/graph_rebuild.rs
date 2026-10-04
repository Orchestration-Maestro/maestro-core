//! First publication, explicit recovery and honest immutable-receipt repair refusals.
use super::support::Home;

#[test]
fn graph_rebuild_bare_command_reports_generation_guidance() {
    let home = Home::new();
    let result = home.run(&["knowledge", "graph", "rebuild"]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result
            .stderr
            .contains("select the attached generation with --generation"),
        "{result:?}"
    );
    assert!(!home.data().join("graph").exists());
}

#[cfg(not(feature = "engine"))]
#[test]
fn graph_rebuild_generation_without_engine_refuses_without_graph() {
    let home = Home::new();
    let result = home.run(&["knowledge", "graph", "rebuild", "--generation", "1"]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("engine feature"), "{result:?}");
    assert!(!home.data().join("graph").exists());
}

#[cfg(all(feature = "engine", not(windows)))]
mod native {
    use super::super::{
        graph_build::exited,
        graph_rebuild_fixture::{Fixture, tree},
    };
    use maestro_kernel::{
        facts::{EXACT_RESOLVER_VERSION, ResolutionInput},
        scope::LOCAL,
    };
    use serde_json::json;
    use std::{
        fs,
        os::unix::fs::MetadataExt as _,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError},
        },
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn graph_rebuild_first_build_measures_loader_and_displays_flag_and_receipt_default() {
        let fixture = Fixture::new();
        let missing = fixture.run(&[]);
        exited(&missing, 2);
        assert!(missing.stderr.contains("supply --resolution"));
        let path = fixture.home.data().join("graph");
        let (stop, stopping) = mpsc::channel();
        let peak = Arc::new(AtomicU64::new(0));
        let peak_disk = Arc::new(AtomicU64::new(0));
        let sampled_disk = Arc::clone(&peak_disk);
        let sampled = Arc::clone(&peak);
        let sampler = thread::spawn(move || sample(&path, &stopping, (&sampled, &sampled_disk)));
        let started = Instant::now();
        let built = fixture.run(&["--resolution", fixture.resolution.as_str()]);
        let elapsed = started.elapsed();
        stop.send(()).unwrap();
        sampler.join().unwrap();
        exited(&built, 0);
        assert_eq!(
            built.json()["selection"]["resolution"],
            json!(fixture.resolution)
        );
        assert_eq!(built.json()["selection"]["resolution_source"], "flag");
        assert_eq!(built.json()["receipt"]["identity"]["entity_fact_count"], 4);
        println!(
            concat!(
                "G28D_REBUILD_US={} PEAK_LOGICAL_BYTES={} PEAK_ALLOCATED_BYTES={} ",
                "FINAL_LOGICAL_BYTES={} FINAL_ALLOCATED_BYTES={}"
            ),
            elapsed.as_micros(),
            peak.load(Ordering::Relaxed),
            peak_disk.load(Ordering::Relaxed),
            usage(&fixture.home.data().join("graph"))[0],
            usage(&fixture.home.data().join("graph"))[1]
        );
        let before = fixture
            .home
            .database()
            .projection_ready(
                &fixture.home.database().visible(LOCAL).unwrap(),
                fixture.generation,
            )
            .unwrap();
        let ready = fixture.run(&[]);
        exited(&ready, 0);
        assert_eq!(ready.json()["action"], "ready");
        assert_eq!(ready.json()["selection"]["resolution_source"], "receipt");
        assert_eq!(
            ready.json()["selection"]["resolution"],
            json!(fixture.resolution)
        );
        let db = fixture.home.database();
        assert_eq!(
            db.projection_ready(&db.visible(LOCAL).unwrap(), fixture.generation)
                .unwrap(),
            before
        );
        assert!(
            db.published_generation(&db.visible(LOCAL).unwrap(), "synthetic-graph")
                .unwrap()
                .is_none(),
            "graph rebuild never changes search publication"
        );
    }

    fn sample(path: &Path, stopping: &Receiver<()>, peaks: (&AtomicU64, &AtomicU64)) {
        let (sampled, sampled_disk) = peaks;
        loop {
            let [logical, disk] = usage(path);
            sampled.fetch_max(logical, Ordering::Relaxed);
            sampled_disk.fetch_max(disk, Ordering::Relaxed);
            if stopping.recv_timeout(Duration::from_millis(1)) != Err(RecvTimeoutError::Timeout) {
                break;
            }
        }
        let [logical, disk] = usage(path);
        sampled.fetch_max(logical, Ordering::Relaxed);
        sampled_disk.fetch_max(disk, Ordering::Relaxed);
    }

    /// Sample file lengths and filesystem allocation, including staging and journals.
    fn usage(path: &Path) -> [u64; 2] {
        let mut total = [0, 0];
        for entry in fs::read_dir(path).into_iter().flatten().flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let bytes = if metadata.is_dir() {
                usage(&entry.path())
            } else {
                [metadata.len(), metadata.blocks() * 512]
            };
            total[0] += bytes[0];
            total[1] += bytes[1];
        }
        total
    }

    #[test]
    fn graph_rebuild_explicit_resume_reuses_the_same_job_and_checkpointed_content() {
        let fixture = Fixture::new();
        let (job, staging) = fixture.interrupted();
        let manifest = fs::read(staging.join("loader/manifest.json")).ok();
        let result = fixture.run(&[
            "--resolution",
            fixture.resolution.as_str(),
            "--resume",
            &job.to_string(),
        ]);
        exited(&result, 0);
        let db = fixture.home.database();
        let ready = db
            .projection_ready(&db.visible(LOCAL).unwrap(), fixture.generation)
            .unwrap()
            .unwrap();
        assert_eq!(ready.identity.entity_fact_count, 4);
        assert!(
            staging.exists(),
            "staging is retained by the approved policy"
        );
        assert!(
            manifest.is_some(),
            "real immutable loader ownership record existed"
        );
        assert_eq!(
            db.job(&db.visible(LOCAL).unwrap(), job)
                .unwrap()
                .unwrap()
                .state
                .to_string(),
            "succeeded"
        );
    }

    #[test]
    fn graph_rebuild_changed_resolution_refuses_before_checkpoint_replay() {
        let fixture = Fixture::new();
        let (job, staging) = fixture.interrupted();
        let before = tree(&staging);
        let db = fixture.home.database();
        let scopes = db.visible(LOCAL).unwrap();
        let changed = db
            .record_resolution(
                &scopes,
                LOCAL,
                &ResolutionInput {
                    resolver_version: EXACT_RESOLVER_VERSION.into(),
                    sets: vec![fixture.set.clone()],
                    previous: Some(fixture.resolution.clone()),
                    decisions: vec![],
                },
                &|_| Ok(()),
            )
            .unwrap()
            .id;
        let refused = fixture.run(&[
            "--resolution",
            changed.as_str(),
            "--resume",
            &job.to_string(),
        ]);
        exited(&refused, 2);
        assert!(
            refused
                .stderr
                .contains("projection backend operation failed"),
            "{refused:?}"
        );
        assert_eq!(tree(&staging), before);
        assert!(
            db.projection_ready(&scopes, fixture.generation)
                .unwrap()
                .is_none()
        );
        let terminal = fixture.run(&[
            "--resolution",
            fixture.resolution.as_str(),
            "--resume",
            &job.to_string(),
        ]);
        exited(&terminal, 2);
        assert!(terminal.stderr.contains("terminal staging is preserved"));
        assert_eq!(tree(&staging), before);
    }

    #[test]
    fn graph_rebuild_published_missing_corrupt_and_changed_pins_are_read_only() {
        let fixture = Fixture::new();
        let built = fixture.run(&["--resolution", fixture.resolution.as_str()]);
        exited(&built, 0);
        let path = fixture.home.data().join("graph").join(
            built.json()["receipt"]["identity"]["file_name"]
                .as_str()
                .unwrap(),
        );
        let original = fs::read(&path).unwrap();
        for (case, content) in [
            ("corrupt or unavailable file", Some(b"corrupt".as_slice())),
            ("missing file", None),
        ] {
            if let Some(content) = content {
                fs::write(&path, content).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
            let refused = fixture.run(&[]);
            exited(&refused, 2);
            assert!(
                refused.stderr.contains(&format!(
                    "({case}); model-free repair of a published generation is pending design"
                )),
                "{refused:?}"
            );
            if let Some(content) = content {
                assert_eq!(fs::read(&path).unwrap(), content);
            } else {
                assert!(!path.exists());
            }
        }
        fs::write(&path, original).unwrap();
        let before = tree(&fixture.home.data().join("graph"));
        let changed = fixture.run(&[
            "--set",
            "graph.engine=ladybug",
            "--set",
            "graphdb.max_db_size=33554432",
        ]);
        exited(&changed, 2);
        assert!(changed.stderr.contains("(changed settings)"), "{changed:?}");
        assert_eq!(tree(&fixture.home.data().join("graph")), before);
        // Represent the retained legacy row shape already proved by the kernel migration tests.
        let connection =
            rusqlite::Connection::open(fixture.home.data().join("kernel.sqlite3")).unwrap();
        connection
            .execute_batch(
                "DROP TRIGGER graph_projection_receipts_never_changed;
            UPDATE graph_projection_receipts SET schema_version='maestro-typed-edges/1',
            resolution_id=NULL, resolver_version=NULL, settings_identity=NULL, frozen_lock=NULL;",
            )
            .unwrap();
        let missing_flag = fixture.run(&[]);
        exited(&missing_flag, 2);
        assert!(
            missing_flag.stderr.contains("supply --resolution"),
            "{missing_flag:?}"
        );
        let legacy = fixture.run(&["--resolution", fixture.resolution.as_str()]);
        exited(&legacy, 2);
        assert!(legacy.stderr.contains("(legacy receipt)"), "{legacy:?}");
        assert_eq!(tree(&fixture.home.data().join("graph")), before);
    }

    #[test]
    fn graph_rebuild_old_reader_keeps_its_generation_and_resolution_after_another_build() {
        use maestro_kernel::generation::NewGeneration;
        use maestro_knowledge::graph::{
            projection::TypedEdgeProjection as _, resolve::resolve_snapshot,
        };
        let mut fixture = Fixture::new();
        exited(
            &fixture.run(&["--resolution", fixture.resolution.as_str()]),
            0,
        );
        let db = fixture.home.database();
        let scopes = db.visible(LOCAL).unwrap();
        let old_scope = fixture.scope();
        let factory = fixture.factory();
        let reader = factory.reader(&db, &scopes, old_scope.clone()).unwrap();
        let resolution = db
            .resolution(&scopes, LOCAL, &fixture.resolution)
            .unwrap()
            .unwrap();
        let entity = resolve_snapshot(&resolution).unwrap()[0].id.clone();
        let before = reader.entity_facts(&scopes, &old_scope, &entity).unwrap();
        let old_receipt = db.projection_ready(&scopes, fixture.generation).unwrap();
        fixture.generation = db
            .create_generation(&NewGeneration {
                collection_id: old_scope.collection_id.clone(),
                chunk_set_id: "rebuild-chunks".into(),
                embedding_profile: "test".into(),
                sparse_profile: "test".into(),
            })
            .unwrap()
            .id;
        let original = db
            .graph_attachment(&scopes, old_scope.generation_id)
            .unwrap()
            .unwrap();
        exited(
            &fixture.home.run(&[
                "knowledge",
                "graph",
                "attach",
                "--build",
                &original.job.to_string(),
                "--generation",
                &fixture.generation.to_string(),
            ]),
            0,
        );
        db.verify_generation(fixture.generation, 0).unwrap();
        fixture.resolution = db
            .record_resolution(
                &scopes,
                LOCAL,
                &ResolutionInput {
                    resolver_version: EXACT_RESOLVER_VERSION.into(),
                    sets: vec![fixture.set.clone()],
                    previous: Some(fixture.resolution.clone()),
                    decisions: vec![],
                },
                &|_| Ok(()),
            )
            .unwrap()
            .id;
        exited(
            &fixture.run(&["--resolution", fixture.resolution.as_str()]),
            0,
        );
        assert_eq!(
            reader.entity_facts(&scopes, &old_scope, &entity).unwrap(),
            before
        );
        assert!(
            reader
                .entity_facts(&scopes, &fixture.scope(), &entity)
                .is_err()
        );
        assert_eq!(
            db.projection_ready(&scopes, old_scope.generation_id)
                .unwrap(),
            old_receipt
        );
        let next = factory.reader(&db, &scopes, fixture.scope()).unwrap();
        assert_eq!(
            next.entity_facts(&scopes, &fixture.scope(), &entity)
                .unwrap()
                .len(),
            before.len()
        );
    }

    #[test]
    fn graph_rebuild_requires_explicit_resume_and_retains_cancelled_job_staging() {
        use maestro_kernel::job::JobState;
        use std::time::SystemTime;
        let fixture = Fixture::new();
        let (job, staging) = fixture.interrupted();
        let before = tree(&staging);
        let missing = fixture.run(&["--resolution", fixture.resolution.as_str()]);
        exited(&missing, 2);
        assert!(
            missing
                .stderr
                .contains(&format!("repeat with --resume {job}")),
            "{missing:?}"
        );
        assert_eq!(tree(&staging), before);
        let db = fixture.home.database();
        let lease = db
            .take_job(job, "canceller", SystemTime::now(), Duration::from_secs(60))
            .unwrap();
        db.complete_job(&lease, JobState::Cancelled, &json!({}))
            .unwrap();
        let terminal = fixture.run(&[
            "--resolution",
            fixture.resolution.as_str(),
            "--resume",
            &job.to_string(),
        ]);
        exited(&terminal, 2);
        assert!(terminal.stderr.contains("terminal staging is preserved"));
        exited(
            &fixture.run(&["--resolution", fixture.resolution.as_str()]),
            0,
        );
        assert_eq!(
            tree(&staging),
            before,
            "new attempt cannot discard previous staging"
        );
    }
}
