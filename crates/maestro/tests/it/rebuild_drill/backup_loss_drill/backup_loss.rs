//! The opt-in real-service backup, projection-loss, restore and ranking proof.

use super::super::{
    authority::{self, assert_point_payloads},
    fixture::{self, SearchEnvironment, capture_one, capture_snapshot},
    qdrant::QdrantScratch,
    ranking_oracle::{self, Snapshot},
    resume::{self, Interrupted},
    wipe_safety::{self, QdrantOwner},
};
use super::setup::{
    COLLECTION, InitialPublication, Sources, assert_success, load_sources, prepare_collection,
    publish_args, publish_initial, recovery_args, run_cli,
};
use crate::{backup_restore, knowledge_publish::StubRouter, support::Home};
use maestro_kernel::{artifact::Digest, evidence::RouteStatus, scope::LOCAL};
use rusqlite::{Connection, OpenFlags, OptionalExtension as _};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Drill {
    owner: QdrantOwner,
    qdrant: QdrantScratch,
    router: StubRouter,
    home: Home,
    sources: Sources,
    initial: InitialPublication,
}

struct Baseline {
    snapshot: Snapshot,
    point_ids: BTreeSet<String>,
    progress: Value,
    source_authority: Value,
    interrupted: Interrupted,
    report_path: PathBuf,
    digest: Digest,
}

struct Backup {
    path: PathBuf,
    manifest: Value,
}

struct Recovered {
    document: Value,
    generation: i64,
}

struct VerifiedRecovery {
    recovered: Recovered,
    digest: Digest,
}

/// Runs only with an explicitly owned disposable Qdrant instance.
#[test]
#[ignore = "destructively wipes only this owned synthetic kernel and Qdrant service"]
fn backup_wipe_restore_rebuild_preserves_rankings() {
    let drill = Drill::new();
    let baseline = drill.capture_baseline();
    let backup = drill.backup(&baseline);
    drill.wipe(&baseline, &backup);
    let restored_identity = drill.restore(&baseline, &backup);
    drill.assert_projection_lost(&baseline);
    let recovered = drill.rebuild(&baseline);
    let verified = drill.verify_rebuilt_rankings(&baseline, recovered);
    drill.finish(&baseline, &backup, &restored_identity, &verified);
}

impl Drill {
    fn new() -> Self {
        let owner = wipe_safety::require_qdrant_owner().unwrap_or_else(|error| panic!("{error}"));
        let qdrant = QdrantScratch::new(&owner).unwrap();
        let router = StubRouter::serve();
        let home = Home::new();
        home.add_synthetic();
        let sources = load_sources(&home);
        prepare_collection(&home, &owner, &router, sources.embedder.as_str());
        let initial = publish_initial(&home, &owner, &router, sources.embedder.as_str());
        Self {
            owner,
            qdrant,
            router,
            home,
            sources,
            initial,
        }
    }

    fn search_environment(&self) -> SearchEnvironment<'_> {
        SearchEnvironment {
            home: &self.home,
            router: &self.router,
            qdrant_url: &self.owner.url,
        }
    }

    fn capture_baseline(&self) -> Baseline {
        assert_eq!(self.sources.questions.len(), 56);
        assert_eq!(
            self.sources
                .questions
                .iter()
                .filter(|question| question.answerable)
                .count(),
            48
        );
        let template = fixture::frozen_template(fixture::FrozenInputs {
            home: &self.home,
            qdrant_url: &self.owner.url,
            chunk_set: &self.initial.set_id,
            card: self.sources.embedder.as_str(),
            reranker: self.sources.reranker.as_str(),
            collection_digest: &self.sources.declaration,
            corpus_digest: &self.sources.corpus,
            suite_digest: &self.sources.suite,
        });
        let (snapshot, point_ids, progress) = capture_snapshot(
            &self.search_environment(),
            &template,
            &self.sources.questions,
            self.initial.job,
        );
        ranking_oracle::validate_snapshot(&snapshot, "baseline").unwrap();
        assert_eq!(snapshot.generation, self.initial.generation);
        assert_eq!(snapshot.frozen.chunk_set, self.initial.set_id);
        assert_eq!(snapshot.frozen.point_ids.len(), point_ids.len());
        let report_path = self.home.root().join("rankings-baseline.json");
        let bytes = serde_json::to_vec_pretty(&snapshot).unwrap();
        let digest = Digest::of(&bytes);
        fs::write(&report_path, bytes).unwrap();
        fs::write(
            self.home.root().join("rankings-baseline.sha256"),
            digest.as_str(),
        )
        .unwrap();
        let (old_progress, source_authority) = authority::source_evidence(
            &self.home,
            self.initial.job,
            self.initial.generation,
            self.sources.embedder.as_str(),
            &self.initial.outcome,
        );
        assert_eq!(old_progress, progress);
        assert_eq!(source_authority["chunk_set"], self.initial.set_id);
        assert_eq!(
            source_authority["embedder_card"],
            self.sources.embedder.as_str()
        );
        let interrupted = resume::create(
            &self.home,
            &self.initial.set_id,
            self.sources.embedder.as_str(),
            self.initial.generation,
            &old_progress,
        );
        Baseline {
            snapshot,
            point_ids,
            progress: old_progress,
            source_authority,
            interrupted,
            report_path,
            digest,
        }
    }

    fn backup(&self, baseline: &Baseline) -> Backup {
        authority::record_legacy_sentinels(&self.home);
        let path = backup_restore::backup(&self.home, "backup");
        let manifest = authority::verify_backup(
            &path,
            &[
                self.sources.embedder.clone(),
                self.sources.reranker.clone(),
                self.sources.suite.clone(),
            ],
        );
        let backup_database = Connection::open_with_flags(
            path.join("kernel.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let interrupted: Option<i64> = backup_database
            .query_row(
                "SELECT 1 FROM jobs WHERE id = ?1",
                [baseline.interrupted.job.to_string()],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(interrupted, Some(1), "backup omitted the interrupted job");
        Backup { path, manifest }
    }

    fn wipe(&self, baseline: &Baseline, backup: &Backup) {
        let root = self.home.root().to_path_buf();
        let data = self.home.data();
        wipe_safety::validate_wipe_layout(&root, &data, &backup.path).unwrap();
        let bindings = self.home.config().join("bindings.toml");
        fs::remove_file(&bindings).unwrap();
        assert!(
            !bindings.exists(),
            "recovery must not resolve the import directory"
        );
        self.qdrant
            .delete_generation(baseline.snapshot.generation)
            .unwrap();
        self.qdrant.require_empty().unwrap();
        wipe_safety::wipe_kernel(&root, &data, &backup.path).unwrap();
        for name in [
            "kernel.sqlite3",
            "kernel.sqlite3-wal",
            "kernel.sqlite3-shm",
            "artifacts",
        ] {
            assert!(
                !data.join(name).exists(),
                "owned source entry {name} survived the wipe"
            );
        }
        authority::assert_legacy_sentinels(&self.home);
        let saved = authority::read_baseline(&self.home, &baseline.report_path, &baseline.digest);
        assert_eq!(saved.generation, baseline.snapshot.generation);
        assert_eq!(saved.frozen.point_ids.len(), baseline.point_ids.len());
    }

    fn restore(&self, baseline: &Baseline, backup: &Backup) -> Value {
        let restored = backup_restore::restore(&self.home, &backup.path);
        assert_eq!(restored.code, Some(0), "{restored:?}");
        authority::assert_legacy_sentinels(&self.home);
        let identity = authority::verify_restored_authority(authority::RestoreFacts {
            home: &self.home,
            job: self.initial.job,
            generation: self.initial.generation,
            set_id: &self.initial.set_id,
            embedder: &self.sources.embedder,
            reranker: &self.sources.reranker,
            suite: &self.sources.suite,
            outcome: &self.initial.outcome,
            old_progress: &baseline.progress,
            authority: &baseline.source_authority,
            grants: &self.sources.grants,
            manifest: &backup.manifest,
        });
        assert_eq!(identity["generation"], self.initial.generation);
        assert_eq!(identity["chunk_set"], self.initial.set_id);
        resume::assert_restored(&self.home, &baseline.interrupted);
        identity
    }

    fn assert_projection_lost(&self, baseline: &Baseline) {
        let ordinary = run_cli(
            &self.home,
            &publish_args(&self.initial.set_id, self.sources.embedder.as_str()),
            &self.owner.url,
            self.router.url(),
        );
        assert_eq!(ordinary.code, Some(1), "{ordinary:?}");
        let document = ordinary.json();
        assert_eq!(document["state"], "succeeded");
        assert_eq!(document["historical_generation"], self.initial.generation);
        assert_eq!(document["current_generation"], self.initial.generation);
        assert_eq!(document["projection_ready"], false);
        assert_eq!(document["outcome"], self.initial.outcome);
        let question = baseline.snapshot.questions.first().unwrap();
        let degraded = capture_one(&self.search_environment(), &baseline.snapshot, question);
        assert!(
            degraded.candidates.is_empty()
                || degraded
                    .routes
                    .values()
                    .any(|route| *route != RouteStatus::Ok)
        );
        assert_ne!(degraded.candidates, question.candidates);
        assert_ne!(degraded.routes.get("dense"), Some(&RouteStatus::Ok));
        assert_ne!(degraded.routes.get("lexical"), Some(&RouteStatus::Ok));
        let database = self.home.database();
        let scopes = database.visible(LOCAL).unwrap();
        assert_eq!(
            database.generations(&scopes, COLLECTION).unwrap().len(),
            2,
            "ordinary cached publish created no replacement"
        );
    }

    fn rebuild(&self, baseline: &Baseline) -> Recovered {
        let result = run_cli(
            &self.home,
            &recovery_args(&self.initial.set_id, self.sources.embedder.as_str()),
            &self.owner.url,
            self.router.url(),
        );
        assert_success("explicit rebuild", &result);
        let document = result.json();
        let generation = document["current_generation"].as_i64().unwrap();
        assert_ne!(generation, self.initial.generation);
        assert_eq!(document["historical_generation"], generation);
        assert_eq!(document["projection_ready"], true);
        assert_eq!(document["resuming"], true);
        assert_eq!(document["job"], baseline.interrupted.job.to_string());
        assert_eq!(document["attempt"], 1);
        assert_eq!(
            resume::takeover_number(&self.home, &baseline.interrupted),
            2
        );
        assert_eq!(
            resume::replayed_indices(&self.home, &baseline.interrupted),
            resume::expected_replayed_indices(baseline.progress["chunks"].as_u64().unwrap())
        );
        Recovered {
            document,
            generation,
        }
    }

    fn verify_rebuilt_rankings(
        &self,
        baseline: &Baseline,
        recovered: Recovered,
    ) -> VerifiedRecovery {
        let verify = run_cli(
            &self.home,
            &["--json", "knowledge", "verify", "--collection", COLLECTION],
            &self.owner.url,
            self.router.url(),
        );
        assert_success("verify rebuilt projection", &verify);
        let ready = run_cli(
            &self.home,
            &publish_args(&self.initial.set_id, self.sources.embedder.as_str()),
            &self.owner.url,
            self.router.url(),
        );
        assert_success("cached publish reconciliation", &ready);
        let document = ready.json();
        assert_eq!(document["historical_generation"], self.initial.generation);
        assert_eq!(document["current_generation"], recovered.generation);
        assert_eq!(document["projection_ready"], true);
        assert_eq!(document["outcome"], self.initial.outcome);

        let questions = fixture::question_inputs(&baseline.snapshot.questions);
        let (after, point_ids, _) = capture_snapshot(
            &self.search_environment(),
            &baseline.snapshot.frozen,
            &questions,
            self.initial.job,
        );
        ranking_oracle::validate_snapshot(&after, "rebuilt").unwrap();
        ranking_oracle::compare_rankings(&baseline.snapshot, &after).unwrap();
        assert_eq!(point_ids, baseline.point_ids);
        authority::assert_projection_is_ready(authority::ProjectionExpectation {
            home: &self.home,
            qdrant: &self.qdrant,
            generation: recovered.generation,
            old_generation: self.initial.generation,
            chunk_set: &self.initial.set_id,
            baseline: &baseline.snapshot,
        });
        assert_point_payloads(
            &self.qdrant,
            &self.home,
            recovered.generation,
            &baseline.snapshot,
        );
        let bytes = serde_json::to_vec_pretty(&after).unwrap();
        let digest = Digest::of(&bytes);
        fs::write(self.home.root().join("rankings-rebuilt.json"), bytes).unwrap();
        fs::write(
            self.home.root().join("rankings-rebuilt.sha256"),
            digest.as_str(),
        )
        .unwrap();
        VerifiedRecovery { recovered, digest }
    }

    fn assert_missing_alias_report(&self, recovered: &Recovered) {
        self.qdrant.delete_alias(recovered.generation).unwrap();
        let missing_alias = run_cli(
            &self.home,
            &publish_args(&self.initial.set_id, self.sources.embedder.as_str()),
            &self.owner.url,
            self.router.url(),
        );
        assert_eq!(missing_alias.code, Some(1), "{missing_alias:?}");
        let document = missing_alias.json();
        assert_eq!(document["historical_generation"], self.initial.generation);
        assert_eq!(document["current_generation"], recovered.generation);
        assert_eq!(document["projection_ready"], false);
        assert_eq!(document["outcome"], self.initial.outcome);
    }

    fn finish(
        &self,
        baseline: &Baseline,
        backup: &Backup,
        restored_identity: &Value,
        verified: &VerifiedRecovery,
    ) {
        let recovered = &verified.recovered;
        self.assert_missing_alias_report(recovered);
        self.qdrant.cleanup().unwrap();
        self.qdrant.require_empty().unwrap();
        let answerable = baseline
            .snapshot
            .questions
            .iter()
            .filter(|question| question.answerable)
            .count();
        println!(
            "T033B_ACCEPTED {}",
            json!({
                "schema": "maestro-t033b-acceptance/1",
                "questions": baseline.snapshot.questions.len(),
                "answerable": answerable,
                "old_generation": self.initial.generation,
                "replacement_generation": recovered.generation,
                "chunk_set": self.initial.set_id,
                "point_count": baseline.point_ids.len(),
                "point_ids_equal": true,
                "ordered_rankings_equal": true,
                "baseline_sha256": baseline.digest.as_str(),
                "rebuilt_sha256": verified.digest.as_str(),
                "backup_schema": backup.manifest["schema"],
                "restored_generation": restored_identity["generation"],
                "qdrant_version": self.qdrant.version,
                "historical_job_unchanged": true,
                "recovery_job": recovered.document["job"],
            })
        );
    }
}
