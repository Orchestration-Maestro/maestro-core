use super::super::writer::{
    BuildVerification, ProjectionBackend, ProjectionReader, ProjectionWriter,
};
use crate::graph::projection::{EdgeFamily, ProjectionEdge, ProjectionScope};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        Provenance, ReviewState, Validity,
    },
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
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
    published: BTreeSet<i64>,
    fail_batch: bool,
}

impl ProjectionBackend for Fake {
    fn create_unpublished(&mut self, _scope: &ProjectionScope) -> Result<(), String> {
        Ok(())
    }

    fn write_batch(
        &mut self,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[super::super::EntityFact],
    ) -> Result<(), String> {
        if self.fail_batch {
            return Err("batch aborted".to_owned());
        }
        self.pending
            .entry(scope.generation_id)
            .or_default()
            .extend_from_slice(edges);
        *self.fact_counts.entry(scope.generation_id).or_default() += facts.len();
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
        Ok(BuildVerification {
            schema: "maestro-typed-edges/1".to_owned(),
            family_counts,
            fact_count: *self.fact_counts.get(&scope.generation_id).unwrap_or(&0),
            content_digest: Digest::of(b"fake projection content"),
            indexes: BTreeSet::from([
                "edge_by_scope_family_source".to_owned(),
                "fact_by_scope_subject".to_owned(),
            ]),
        })
    }

    fn publish_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String> {
        if self.published.contains(&scope.generation_id) {
            return Err("already published".to_owned());
        }
        self.published.insert(scope.generation_id);
        Ok(())
    }

    fn open_published(&self, scope: &ProjectionScope) -> Result<(), String> {
        self.published
            .contains(&scope.generation_id)
            .then_some(())
            .ok_or_else(|| "not published".to_owned())
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
    let mut writer = ProjectionWriter::create(backend, contract.scope.clone()).unwrap();
    writer
        .write_batch(contract.scopes, contract.edges, contract.facts)
        .unwrap();
    assert_eq!(
        writer.verify_and_publish(contract.expected).unwrap(),
        *contract.expected
    );
    drop(writer);
    assert_eq!(
        ProjectionReader::open(backend, contract.scope.clone())
            .unwrap()
            .scope(),
        contract.scope
    );
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
        relation: "DEPENDS_ON".to_owned(),
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
    let edges = [
        edge(3, EdgeFamily::KnowledgeClaim),
        edge(3, EdgeFamily::CatalogDependency),
    ];
    let expected = BuildVerification {
        schema: "maestro-typed-edges/1".to_owned(),
        family_counts: BTreeMap::from([
            (EdgeFamily::KnowledgeClaim, 1),
            (EdgeFamily::CatalogDependency, 1),
        ]),
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

#[test]
fn an_entity_claim_cannot_be_written_as_a_literal_subject_fact() {
    use maestro_kernel::facts::Object;

    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 6,
    };
    let (path, database, all) = scoped();
    let mut catalog_claim = fact(&scope);
    catalog_claim.claim.claim.predicate = Predicate::Requires;
    catalog_claim.claim.claim.object = Object::Entity(EntityName {
        kind: EntityKind::Component,
        name: "engine".to_owned(),
    });
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope).unwrap();
    assert!(writer.write_batch(&all, &[], &[catalog_claim]).is_err());
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.pending.get(&6).is_none_or(Vec::is_empty));
    assert_eq!(backend.fact_counts.get(&6), None);
}

#[test]
fn an_unverified_count_or_index_set_never_becomes_ready() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 5,
    };
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
    let wrong = BuildVerification {
        schema: "maestro-typed-edges/1".to_owned(),
        family_counts: BTreeMap::new(),
        fact_count: 0,
        content_digest: Digest::of(b"fake projection content"),
        indexes: BTreeSet::new(),
    };
    assert!(writer.verify_and_publish(&wrong).is_err());
    drop(writer);
    assert!(ProjectionReader::open(&backend, scope).is_err());
    assert!(!backend.published.contains(&5));
}
