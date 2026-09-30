//! Backend-neutral projection writer and reader contract tests.

use super::super::writer::{
    BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionBackendReader,
    ProjectionReader, ProjectionReadiness, ProjectionWriter,
};
use crate::graph::projection::{
    EdgeFamily, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        ProjectionReceipt, Provenance, ReviewState, Validity,
    },
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    slice,
};

fn scoped() -> (PathBuf, Database, ScopeSet) {
    let path = maestro_test_scratch::scratch_directory().unwrap();
    let database = Database::open_in(&path).unwrap();
    database
        .grant(
            "projection-test",
            &"workspace/default".parse::<Scope>().unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
    let scopes = database.visible("projection-test").unwrap();
    (path, database, scopes)
}

#[derive(Default)]
struct Fake {
    pending: BTreeMap<i64, Vec<ProjectionEdge>>,
    fact_counts: BTreeMap<i64, usize>,
    pending_facts: BTreeMap<i64, Vec<super::super::EntityFact>>,
    published: BTreeSet<i64>,
    fail_batch: bool,
    schema: Option<String>,
    omit_edge_index: bool,
}

#[derive(Clone)]
struct FakeReader {
    edges: Vec<ProjectionEdge>,
    facts: Vec<super::super::EntityFact>,
    fact_count: usize,
}
impl ProjectionBackendReader for FakeReader {
    fn verification(&self) -> Result<BuildVerification, String> {
        let mut family_counts = BTreeMap::new();
        for edge in &self.edges {
            *family_counts.entry(edge.family).or_insert(0) += 1;
        }
        Ok(BuildVerification {
            schema: "maestro-typed-edges/1".to_owned(),
            family_counts,
            fact_count: self.fact_count,
            content_digest: Digest::of(b"fake projection content"),
            indexes: BTreeSet::from([
                "edge_by_scope_family_source".to_owned(),
                "fact_by_scope_subject".to_owned(),
            ]),
        })
    }
    fn edges_adjacent(
        &self,
        _family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, String> {
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.source == *entity || edge.target == *entity)
            .cloned()
            .collect())
    }
    fn facts_for(&self, subject: &Digest) -> Result<Vec<super::super::EntityFact>, String> {
        Ok(self
            .facts
            .iter()
            .filter(|fact| fact.subject == *subject)
            .cloned()
            .collect())
    }
}
struct Ready {
    knowledge: usize,
    catalog: usize,
    facts: usize,
}
impl ProjectionReadiness for Ready {
    fn projection_ready(
        &self,
        _scopes: &ScopeSet,
        scope: &ProjectionScope,
    ) -> Result<Option<ProjectionReceipt>, String> {
        Ok(Some(ProjectionReceipt {
            collection_id: scope.collection_id.clone(),
            generation_id: scope.generation_id,
            claim_set_id: Digest::of(b"set"),
            file_name: "projection.db".to_owned(),
            schema_version: "maestro-typed-edges/1".to_owned(),
            knowledge_edge_count: self.knowledge,
            catalog_dependency_edge_count: self.catalog,
            entity_fact_count: self.facts,
            content_digest: Digest::of(b"fake projection content"),
        }))
    }
}
fn ready(knowledge: usize, catalog: usize, facts: usize) -> Ready {
    Ready {
        knowledge,
        catalog,
        facts,
    }
}
impl ProjectionBackend for Fake {
    type Reader = FakeReader;
    fn inject_batch_failure(&mut self) -> Result<(), String> {
        self.fail_batch = true;
        Ok(())
    }
    fn create_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String> {
        if self.published.contains(&scope.generation_id) {
            return Err("already published".to_owned());
        }
        Ok(())
    }

    fn write_batch(
        &mut self,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[super::super::EntityFact],
    ) -> Result<(), String> {
        if self.published.contains(&scope.generation_id) {
            return Err("already published".to_owned());
        }
        if self.fail_batch {
            self.fail_batch = false;
            return Err("batch aborted".to_owned());
        }
        self.pending
            .entry(scope.generation_id)
            .or_default()
            .extend_from_slice(edges);
        *self.fact_counts.entry(scope.generation_id).or_default() += facts.len();
        self.pending_facts
            .entry(scope.generation_id)
            .or_default()
            .extend_from_slice(facts);
        Ok(())
    }

    fn verify_unpublished(&mut self, scope: &ProjectionScope) -> Result<BuildVerification, String> {
        let rows = self
            .pending
            .get(&scope.generation_id)
            .map_or(&[][..], Vec::as_slice);
        let mut family_counts = BTreeMap::new();
        for edge in rows {
            *family_counts.entry(edge.family).or_insert(0) += 1;
        }
        let mut indexes = BTreeSet::from([
            "edge_by_scope_family_source".to_owned(),
            "fact_by_scope_subject".to_owned(),
        ]);
        if self.omit_edge_index {
            indexes.remove("edge_by_scope_family_source");
        }
        Ok(BuildVerification {
            schema: self
                .schema
                .clone()
                .unwrap_or_else(|| "maestro-typed-edges/1".to_owned()),
            family_counts,
            fact_count: *self.fact_counts.get(&scope.generation_id).unwrap_or(&0),
            content_digest: Digest::of(b"fake projection content"),
            indexes,
        })
    }

    fn publish_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String> {
        if self.published.contains(&scope.generation_id) {
            return Err("already published".to_owned());
        }
        self.published.insert(scope.generation_id);
        Ok(())
    }

    fn open_published(&self, scope: &ProjectionScope) -> Result<Self::Reader, String> {
        if !self.published.contains(&scope.generation_id) {
            return Err("not published".to_owned());
        }
        Ok(FakeReader {
            edges: self
                .pending
                .get(&scope.generation_id)
                .cloned()
                .unwrap_or_default(),
            facts: self
                .pending_facts
                .get(&scope.generation_id)
                .cloned()
                .unwrap_or_default(),
            fact_count: *self.fact_counts.get(&scope.generation_id).unwrap_or(&0),
        })
    }
}

fn fact(scope: &ProjectionScope) -> super::super::EntityFact {
    super::super::EntityFact {
        claim: ClaimRecord {
            id: Digest::of(b"claim"),
            collection_id: scope.collection_id.clone(),
            claim: Claim {
                subject: EntityName {
                    kind: EntityKind::Parameter,
                    name: "mode".to_owned(),
                },
                predicate: Predicate::DefaultsTo,
                object: Object::Literal(Literal {
                    kind: LiteralKind::Text,
                    lexeme: "fast".to_owned(),
                }),
                conditions: BTreeMap::new(),
                version: Validity::Unknown,
                world: Validity::Unknown,
                provenance: Provenance {
                    extractor: "test".to_owned(),
                    profile: Digest::of(b"profile"),
                },
                supports: Vec::new(),
            },
            review: ReviewState::Unreviewed,
            recorded_at: "2026-01-01T00:00:00Z".to_owned(),
        },
        subject: Digest::of(b"entity"),
        scope: scope.clone(),
    }
}

pub(super) struct BackendContract<'a> {
    pub(super) scopes: &'a ScopeSet,
    pub(super) scope: &'a ProjectionScope,
    pub(super) edges: &'a [ProjectionEdge],
    pub(super) facts: &'a [super::super::EntityFact],
    pub(super) expected: &'a BuildVerification,
}

pub(super) fn verified_backend_contract<B: ProjectionBackend>(
    backend: &mut B,
    contract: &BackendContract<'_>,
) {
    assert!(
        ProjectionReader::open(
            backend,
            &ready(1, 0, 1),
            contract.scopes,
            contract.scope.clone()
        )
        .is_err()
    );
    let mut writer = ProjectionWriter::create(backend, contract.scope.clone()).unwrap();
    let before = writer.backend.verify_unpublished(contract.scope).unwrap();
    writer.backend.inject_batch_failure().unwrap();
    assert!(
        writer
            .write_batch(contract.scopes, contract.edges, contract.facts)
            .is_err()
    );
    assert_eq!(
        writer.backend.verify_unpublished(contract.scope).unwrap(),
        before
    );
    writer
        .write_batch(contract.scopes, contract.edges, contract.facts)
        .unwrap();
    assert_eq!(
        writer.verify_and_publish(contract.expected).unwrap(),
        *contract.expected
    );
    drop(writer);
    let reader = ProjectionReader::open(
        backend,
        &ready(1, 0, 1),
        contract.scopes,
        contract.scope.clone(),
    )
    .unwrap();
    assert_eq!(reader.scope(), contract.scope);
    assert_eq!(
        reader
            .entity_facts(contract.scopes, contract.scope, &contract.facts[0].subject)
            .unwrap(),
        contract.facts
    );
    let entity = contract.edges[0].source.clone();
    assert_eq!(
        reader
            .neighbors(
                contract.scopes,
                contract.scope,
                EdgeFamily::KnowledgeClaim,
                &entity
            )
            .unwrap(),
        vec![contract.edges[0].clone()]
    );
    let other_pin = ProjectionScope {
        generation_id: contract.scope.generation_id + 1,
        ..contract.scope.clone()
    };
    assert!(
        reader
            .neighbors(
                contract.scopes,
                &other_pin,
                EdgeFamily::KnowledgeClaim,
                &entity
            )
            .is_err()
    );
    assert!(backend.create_unpublished(contract.scope).is_err());
    assert!(
        backend
            .write_batch(contract.scope, contract.edges, contract.facts)
            .is_err()
    );
    assert!(backend.publish_unpublished(contract.scope).is_err());
}

fn edge(generation_id: i64, family: EdgeFamily) -> ProjectionEdge {
    ProjectionEdge {
        id: Digest::of(format!("edge-{generation_id}-{family:?}").as_bytes()),
        scope: ProjectionScope {
            collection_id: "c".to_owned(),
            generation_id,
        },
        family,
        source: Digest::of(b"source"),
        target: Digest::of(b"target"),
        relation: Predicate::Requires.as_str().to_owned(),
    }
}

#[test]
fn backend_contract_keeps_families_separate_and_publishes_only_verified_builds() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 3,
    };
    let (path, database, all) = scoped();
    let mut backend = Fake::default();
    let edges = [edge(3, EdgeFamily::KnowledgeClaim)];
    let expected = BuildVerification {
        schema: "maestro-typed-edges/1".to_owned(),
        family_counts: BTreeMap::from([(EdgeFamily::KnowledgeClaim, 1)]),
        fact_count: 1,
        content_digest: Digest::of(b"fake projection content"),
        indexes: BTreeSet::from([
            "edge_by_scope_family_source".to_owned(),
            "fact_by_scope_subject".to_owned(),
        ]),
    };
    let facts = [fact(&scope)];
    verified_backend_contract(
        &mut backend,
        &BackendContract {
            scopes: &all,
            scope: &scope,
            edges: &edges,
            facts: &facts,
            expected: &expected,
        },
    );
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.published.contains(&3));
}

struct CatalogRelations;
impl CatalogRelationVocabulary for CatalogRelations {
    fn accepts(&self, relation: &str) -> bool {
        relation == "depends_on"
    }
}

#[test]
fn catalog_edges_round_trip_only_in_their_family_pin_and_scope() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 9,
    };
    let (path, database, scopes) = scoped();
    let catalog = ProjectionEdge {
        relation: "depends_on".to_owned(),
        ..edge(9, EdgeFamily::CatalogDependency)
    };
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
    assert!(
        writer
            .write_batch(&scopes, slice::from_ref(&catalog), &[])
            .is_err(),
        "unregistered vocabulary fails closed"
    );
    writer
        .write_batch_with_catalog_vocabulary(
            &scopes,
            slice::from_ref(&catalog),
            &[],
            &CatalogRelations,
        )
        .unwrap();
    let build = writer.backend.verify_unpublished(&scope).unwrap();
    writer.verify_and_publish(&build).unwrap();
    drop(writer);
    let reader = ProjectionReader::open(&backend, &ready(0, 1, 0), &scopes, scope.clone()).unwrap();
    let source = catalog.source.clone();
    assert_eq!(
        reader
            .neighbors(&scopes, &scope, EdgeFamily::CatalogDependency, &source)
            .unwrap(),
        [catalog]
    );
    assert!(
        reader
            .neighbors(&scopes, &scope, EdgeFamily::KnowledgeClaim, &source)
            .unwrap()
            .is_empty()
    );
    let other_pin = ProjectionScope {
        generation_id: 10,
        ..scope.clone()
    };
    assert!(
        reader
            .neighbors(&scopes, &other_pin, EdgeFamily::CatalogDependency, &source)
            .is_err()
    );
    let denied = database.visible("no-grants").unwrap();
    assert!(
        reader
            .neighbors(&denied, &scope, EdgeFamily::CatalogDependency, &source)
            .is_err()
    );
    drop(database);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_batch_does_not_publish_or_leave_a_partial_batch() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 4,
    };
    let (path, database, all) = scoped();
    let mut backend = Fake {
        fail_batch: true,
        ..Fake::default()
    };
    let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
    assert!(
        writer
            .write_batch(&all, &[edge(4, EdgeFamily::CatalogDependency)], &[])
            .is_err()
    );
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.pending.get(&4).is_none_or(Vec::is_empty));
    assert!(!backend.published.contains(&4));
}

#[cfg(test)]
#[path = "writer/extra.rs"]
mod extra;
