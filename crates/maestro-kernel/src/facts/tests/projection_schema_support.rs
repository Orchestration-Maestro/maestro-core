//! Synthetic pre-0031 authority and raw SQL helpers, never a runtime insertion API.
use super::{
    projection::legacy_attached as attached,
    support::{Scratch, timing},
};
use crate::{
    facts::{ProjectionReceipt, ResolutionInput},
    job::NewJob,
    scope::ScopeSet,
    store::Database,
};
use rusqlite::{Connection, params};

/// Own the scratch directory for the lifetime of its raw connection.
pub(super) struct Fixture {
    pub(super) connection: Connection,
    pub(super) receipt: ProjectionReceipt,
    /// Existing runtime writer used only to submit synthetic test jobs.
    pub(super) database: Database,
    _scratch: Scratch,
}

impl Fixture {
    pub(super) fn new(published: bool) -> Self {
        let (scratch, database, _scopes, receipt) = attached();
        if published {
            let row = &receipt;
            scratch
                .outside()
                .execute(
                    "INSERT INTO graph_projection_receipts
                 (generation_id, collection_id, claim_set_id, file_name, schema_version,
                  knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count,
                  content_digest, resolution_id, resolver_version, settings_identity, frozen_lock)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        row.identity.generation_id,
                        row.identity.collection_id,
                        row.identity.claim_set_id.as_str(),
                        row.identity.file_name,
                        row.identity.schema_version,
                        i64::try_from(row.identity.knowledge_edge_count).unwrap(),
                        i64::try_from(row.identity.catalog_dependency_edge_count).unwrap(),
                        i64::try_from(row.identity.entity_fact_count).unwrap(),
                        row.identity.content_digest.as_str(),
                        row.resolution_id.as_str(),
                        row.resolver_version,
                        row.settings_identity.as_str(),
                        row.frozen_lock.as_str()
                    ],
                )
                .unwrap();
        }
        let connection = scratch.outside();
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=OFF;")
            .unwrap();
        Self {
            connection,
            receipt,
            database,
            _scratch: scratch,
        }
    }

    pub(super) fn migrate(&self) {
        self.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let sql = include_str!("../../../migrations/0031_graph_projection_builds.sql");
        self.connection.execute_batch(sql).unwrap();
        self.connection.execute_batch("COMMIT").unwrap();
    }

    pub(super) fn generation(&self) -> i64 {
        self.receipt.identity.generation_id
    }

    pub(super) fn job(&self, marker: &str) -> String {
        let scope = "workspace/default/collection/graph".parse().unwrap();
        self.database
            .submit_job(
                &NewJob {
                    kind: "knowledge.graph.project",
                    inputs: &serde_json::json!({"schema": "graph-project/2", "test": marker}),
                    scope: &scope,
                    resource: None,
                },
                timing(6).now,
            )
            .unwrap()
            .id
            .to_string()
    }

    pub(super) fn alternate_resolution(&self) -> String {
        self.database
            .record_resolution(
                &ScopeSet::default_workspace(),
                "projection",
                &ResolutionInput {
                    resolver_version: self.receipt.resolver_version.clone(),
                    sets: vec![self.receipt.identity.claim_set_id.clone()],
                    previous: Some(self.receipt.resolution_id.clone()),
                    decisions: vec![],
                },
                &|_| Ok(()),
            )
            .unwrap()
            .id
            .as_str()
            .to_owned()
    }

    pub(super) fn reserve(&self, previous: Option<i64>) -> i64 {
        self.insert_build(
            &self.job(&format!("build-{}", self.count("graph_projection_builds"))),
            previous,
        )
        .unwrap();
        self.connection.last_insert_rowid()
    }

    pub(super) fn insert_build(&self, job: &str, previous: Option<i64>) -> rusqlite::Result<usize> {
        self.connection.execute(
            "INSERT INTO graph_projection_builds
             (generation_id, collection_id, claim_set_id, project_job_id, expected_active_build_id,
              schema_version, resolution_id, resolver_version, settings_identity, frozen_lock)
             VALUES (?1, ?2, ?3, ?4, ?5, 'maestro-typed-edges/3', ?6, ?7, ?8, ?9)",
            params![
                self.generation(),
                self.receipt.identity.collection_id,
                self.receipt.identity.claim_set_id.as_str(),
                job,
                previous,
                self.receipt.resolution_id.as_str(),
                self.receipt.resolver_version,
                self.receipt.settings_identity.as_str(),
                self.receipt.frozen_lock.as_str()
            ],
        )
    }

    pub(super) fn insert_receipt(&self, build: i64, name: &str) -> rusqlite::Result<usize> {
        self.connection.execute(
            "INSERT INTO graph_projection_receipts
             (build_id, file_name, knowledge_edge_count, catalog_dependency_edge_count,
              entity_fact_count, content_digest) VALUES (?1, ?2, 0, 0, 1, ?3)",
            params![build, name, self.receipt.identity.content_digest.as_str()],
        )
    }

    pub(super) fn advance(&self, build: i64) -> rusqlite::Result<usize> {
        self.connection.execute(
            "UPDATE graph_projection_active SET build_id = ?1 WHERE generation_id = ?2",
            params![build, self.generation()],
        )
    }

    pub(super) fn head(&self) -> i64 {
        self.connection
            .query_row(
                "SELECT build_id FROM graph_projection_active WHERE generation_id = ?1",
                [self.generation()],
                |row| row.get(0),
            )
            .unwrap()
    }

    pub(super) fn count(&self, table: &str) -> i64 {
        self.connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    /// SQL text contains only synthetic, fixture-owned identities.
    pub(super) fn build_sql(&self, job: &str, previous: Option<i64>) -> String {
        format!(
            "INSERT INTO graph_projection_builds
            (generation_id, collection_id, claim_set_id, project_job_id, expected_active_build_id,
             schema_version, resolution_id, resolver_version, settings_identity, frozen_lock)
            VALUES ({}, 'graph', '{}', '{}', {}, 'maestro-typed-edges/3', '{}', '{}', '{}', '{}')",
            self.generation(),
            self.receipt.identity.claim_set_id.as_str(),
            job,
            previous.map_or_else(|| "NULL".into(), |id| id.to_string()),
            self.receipt.resolution_id.as_str(),
            self.receipt.resolver_version,
            self.receipt.settings_identity.as_str(),
            self.receipt.frozen_lock.as_str()
        )
    }

    pub(super) fn receipt_sql(&self, build: i64, name: &str) -> String {
        format!(
            "INSERT INTO graph_projection_receipts
            (build_id, file_name, knowledge_edge_count, catalog_dependency_edge_count,
             entity_fact_count, content_digest) VALUES ({build}, '{name}', 0, 0, 1, '{}')",
            self.receipt.identity.content_digest.as_str()
        )
    }

    /// Test corruption/lifecycle races without fabricating search publication events.
    pub(super) fn set_state(&self, state: &str) {
        self.connection
            .execute(
                "UPDATE generations SET state = ?1 WHERE id = ?2",
                params![state, self.generation()],
            )
            .unwrap();
    }

    pub(super) fn foreign_keys_clean(&self) {
        assert_eq!(self.count("pragma_foreign_key_check"), 0);
    }
}
