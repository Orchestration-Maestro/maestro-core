//! Record-time semantic validation must not poison immutable snapshot history.

use super::support::{markdown, rule_text, source};
use crate::graph::{
    resolve::{EXACT_RESOLVER_VERSION, resolve_snapshot, validate_snapshot},
    rules::{Extractor as _, TableRule},
};
use maestro_kernel::{
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    facts::{ClaimSet, Decision, DecisionKind, Endpoint, Error, Mention, ResolutionInput},
    scope::{Right, ScopeSet, WORKSPACE},
    store::Database,
};
use rusqlite::Connection;
use serde_json::Map;
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A real admitted table and a scoped reviewer in an isolated database.
struct Fixture {
    /// Directory removed after the database closes.
    path: PathBuf,
    /// Kernel authority under test.
    database: Database,
    /// Request scopes pinned to this reviewer.
    scopes: ScopeSet,
    /// Fresh resolution over the table's frozen claims.
    input: ResolutionInput,
    /// Two distinct sourced subjects.
    pair: [Mention; 2],
}

impl Fixture {
    /// Admit the existing synthetic table through the public kernel API.
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-resolution-write-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let database = Database::open_in(&path).unwrap();
        database
            .record_collection(&Collection {
                id: "graph".into(),
                title: "Graph".into(),
                visibility: "public".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        database
            .record_source(&Source {
                collection_id: "graph".into(),
                id: "docs".into(),
                kind: "import".into(),
                transport: None,
                reference: "fixture:defaults".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        database
            .record_document(&Document {
                id: "document".into(),
                collection_id: "graph".into(),
                source_id: "docs".into(),
                source_ref: "fixture:defaults".into(),
            })
            .unwrap();
        let text = markdown();
        let source = source(&text);
        database
            .record_revision(&Revision {
                id: source.revision_id().into(),
                document_id: "document".into(),
                original_digest: database.put(text.as_bytes(), "text/markdown").unwrap(),
                canonical_digest: database.put(b"{}", "application/json").unwrap(),
                status: RevisionStatus::Valid,
                captured_at: None,
                metadata: Map::new(),
            })
            .unwrap();
        database
            .record_disposition(&Disposition {
                revision_id: source.revision_id().into(),
                outcome: Outcome::Accepted,
                reasons: vec![],
                rule_ids: vec![],
                decided_by: "test".into(),
            })
            .unwrap();
        database
            .grant("reviewer", &WORKSPACE.parse().unwrap(), Right::Read, "test")
            .unwrap();
        let scopes = database.visible("reviewer").unwrap();
        let claims = TableRule::parse(&rule_text())
            .unwrap()
            .extract(&source)
            .claims;
        let set = database
            .record_claim_set(
                &scopes,
                &ClaimSet {
                    collection_id: "graph".into(),
                    claims,
                },
            )
            .unwrap();
        let pair = [0, 1].map(|index| Mention {
            claim: set.claims[index].id.clone(),
            endpoint: Endpoint::Subject,
        });
        let input = ResolutionInput {
            resolver_version: EXACT_RESOLVER_VERSION.into(),
            sets: vec![set.id],
            previous: None,
            decisions: vec![],
        };
        Self {
            path,
            database,
            scopes,
            input,
            pair,
        }
    }

    /// A review between distinct identity groups, optionally in reverse order.
    fn decision(&self, kind: DecisionKind, reverse: bool) -> Decision {
        let [left, right] = if reverse { [1, 0] } else { [0, 1] };
        Decision {
            left: self.pair[left].clone(),
            right: self.pair[right].clone(),
            kind,
            reason: "reviewed evidence".into(),
        }
    }

    /// Count immutable rows independently of the returned result.
    fn count(&self) -> i64 {
        Connection::open(self.path.join("kernel.sqlite3"))
            .unwrap()
            .query_row("SELECT count(*) FROM graph_resolutions", [], |row| {
                row.get(0)
            })
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

#[test]
fn unmatched_separation_is_refused_before_recording() {
    let mut fixture = Fixture::new();
    fixture.input.decisions = vec![fixture.decision(DecisionKind::Separate, false)];
    let result = fixture.database.record_resolution(
        &fixture.scopes,
        "reviewer",
        &fixture.input,
        &validate_snapshot,
    );
    assert!(matches!(result, Err(Error::ResolutionRejected)));
    assert_eq!(fixture.count(), 0);
}

#[test]
fn alias_cycle_is_refused_before_recording_a_descendant() {
    let mut fixture = Fixture::new();
    fixture.input.decisions = vec![fixture.decision(DecisionKind::Alias, false)];
    let parent = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "reviewer",
            &fixture.input,
            &validate_snapshot,
        )
        .unwrap();
    fixture.input.previous = Some(parent.id.clone());
    fixture.input.decisions = vec![fixture.decision(DecisionKind::Alias, true)];
    let result = fixture.database.record_resolution(
        &fixture.scopes,
        "reviewer",
        &fixture.input,
        &validate_snapshot,
    );
    assert!(matches!(result, Err(Error::ResolutionRejected)));
    assert_eq!(fixture.count(), 1);
    assert_eq!(
        fixture
            .database
            .resolution(&fixture.scopes, "reviewer", &parent.id)
            .unwrap(),
        Some(parent)
    );
}

#[test]
fn valid_reversal_records_resolves_and_extends_the_existing_chain() {
    let mut fixture = Fixture::new();
    let original = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "reviewer",
            &fixture.input,
            &validate_snapshot,
        )
        .unwrap();
    let entities = resolve_snapshot(&original).unwrap();
    fixture.input.previous = Some(original.id);
    fixture.input.decisions = vec![fixture.decision(DecisionKind::Alias, false)];
    let aliased = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "reviewer",
            &fixture.input,
            &validate_snapshot,
        )
        .unwrap();
    assert_eq!(
        resolve_snapshot(&aliased).unwrap().len(),
        entities.len() - 1
    );
    fixture.input.previous = Some(aliased.id.clone());
    fixture.input.decisions = vec![fixture.decision(DecisionKind::Separate, false)];
    let reversed = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "reviewer",
            &fixture.input,
            &validate_snapshot,
        )
        .unwrap();
    assert_eq!(resolve_snapshot(&reversed).unwrap(), entities);
    assert_eq!(reversed.history.len(), 2);
    fixture.input.previous = Some(reversed.id.clone());
    fixture.input.decisions.clear();
    let extended = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "reviewer",
            &fixture.input,
            &validate_snapshot,
        )
        .unwrap();
    assert_eq!(extended.history, reversed.history);
    assert_eq!(resolve_snapshot(&extended).unwrap(), entities);
    assert_eq!(fixture.count(), 4);
    assert_eq!(
        fixture
            .database
            .resolution(&fixture.scopes, "reviewer", &aliased.id)
            .unwrap(),
        Some(aliased)
    );
}
