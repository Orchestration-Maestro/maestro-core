//! Backend-neutral projection writer and reader contract tests.

use super::super::{
    content,
    writer::{
        BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionBackendReader,
        ProjectionReader, ProjectionReadiness, ProjectionWriter,
    },
};
use crate::graph::projection::{
    EdgeFamily, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
use maestro_kernel::{
    artifact::Digest,
    facts::ProjectionReceipt,
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
    published: BTreeSet<(ProjectionScope, String)>,
    fail_batch: bool,
    schema: Option<String>,
    omit_edge_index: bool,
}

#[derive(Clone)]
struct FakeReader {
    edges: Vec<ProjectionEdge>,
    facts: Vec<super::super::EntityFact>,
    fact_count: usize,
    digest: Digest,
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
            content_digest: self.digest.clone(),
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
struct Ready(ProjectionReceipt);
impl ProjectionReadiness for Ready {
    fn projection_ready(
        &self,
        _scopes: &ScopeSet,
        scope: &ProjectionScope,
    ) -> Result<Option<ProjectionReceipt>, String> {
        Ok((self.0.collection_id == scope.collection_id
            && self.0.generation_id == scope.generation_id)
            .then(|| self.0.clone()))
    }
}
fn ready(scope: &ProjectionScope, build: &BuildVerification) -> Ready {
    let claim_set_id = Digest::of(b"set");
    Ready(ProjectionReceipt {
        collection_id: scope.collection_id.clone(),
        generation_id: scope.generation_id,
        claim_set_id: claim_set_id.clone(),
        file_name: content::basename(scope, &claim_set_id).unwrap(),
        schema_version: build.schema.clone(),
        knowledge_edge_count: build
            .family_counts
            .get(&EdgeFamily::KnowledgeClaim)
            .copied()
            .unwrap_or(0),
        catalog_dependency_edge_count: build
            .family_counts
            .get(&EdgeFamily::CatalogDependency)
            .copied()
            .unwrap_or(0),
        entity_fact_count: build.fact_count,
        content_digest: build.content_digest.clone(),
    })
}
impl ProjectionBackend for Fake {
    type Reader = FakeReader;
    fn inject_batch_failure(&mut self) -> Result<(), String> {
        self.fail_batch = true;
        Ok(())
    }
    fn create_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String> {
        if self
            .published
            .iter()
            .any(|(published_scope, _)| published_scope == scope)
        {
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
        if self
            .published
            .iter()
            .any(|(published_scope, _)| published_scope == scope)
        {
            return Err("already published".to_owned());
        }
        if self.fail_batch {
            self.fail_batch = false;
            return Err("batch aborted".to_owned());
        }
        let existing_edges = self
            .pending
            .get(&scope.generation_id)
            .map_or(&[][..], Vec::as_slice);
        let existing_facts = self
            .pending_facts
            .get(&scope.generation_id)
            .map_or(&[][..], Vec::as_slice);
        if edges
            .iter()
            .any(|edge| existing_edges.iter().any(|existing| existing.id == edge.id))
            || facts.iter().any(|fact| {
                existing_facts
                    .iter()
                    .any(|existing| existing.claim.id == fact.claim.id)
            })
        {
            return Err("duplicate record ID".to_owned());
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
            content_digest: content::digest(
                rows,
                self.pending_facts
                    .get(&scope.generation_id)
                    .map_or(&[][..], Vec::as_slice),
            )?,
            indexes,
        })
    }

    fn publish_unpublished(
        &mut self,
        scope: &ProjectionScope,
        file_name: &str,
    ) -> Result<(), String> {
        if self
            .published
            .iter()
            .any(|(published_scope, _)| published_scope == scope)
            || !content::is_canonical_basename(file_name)
        {
            return Err("invalid or already published".to_owned());
        }
        self.published.insert((scope.clone(), file_name.to_owned()));
        Ok(())
    }

    fn open_published(
        &self,
        scope: &ProjectionScope,
        receipt: &ProjectionReceipt,
    ) -> Result<Self::Reader, String> {
        if !self
            .published
            .contains(&(scope.clone(), receipt.file_name.clone()))
        {
            return Err("wrong receipt name".to_owned());
        }
        let mut edges = self
            .pending
            .get(&scope.generation_id)
            .cloned()
            .unwrap_or_default();
        let mut facts = self
            .pending_facts
            .get(&scope.generation_id)
            .cloned()
            .unwrap_or_default();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        facts.sort_by(|left, right| left.claim.id.cmp(&right.claim.id));
        Ok(FakeReader {
            digest: content::digest(&edges, &facts)?,
            edges,
            facts,
            fact_count: *self.fact_counts.get(&scope.generation_id).unwrap_or(&0),
        })
    }
}

pub(super) fn verified_backend_contract<B: ProjectionBackend>(
    backend: &mut B,
    contract: &super::contract::BackendContract<'_>,
) {
    super::contract::run(backend, contract);
}

#[test]
fn backend_contract_keeps_families_separate_and_publishes_only_verified_builds() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 3,
    };
    let (path, database, all) = scoped();
    let denied = database.visible("no-grants").unwrap();
    let mut backend = Fake::default();
    let (edges, facts) = super::contract::ordered_rows(&scope);
    let fixture = super::contract::fixture(&all, &denied, &scope, &edges, &facts);
    verified_backend_contract(&mut backend, &fixture);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(
        backend
            .published
            .iter()
            .any(|(published_scope, _)| published_scope.generation_id == 3)
    );
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
        ..super::contract::edge(9, EdgeFamily::CatalogDependency)
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
    writer
        .verify_and_publish(&build, &Digest::of(b"set"))
        .unwrap();
    drop(writer);
    let latest = backend.verify_unpublished(&scope).unwrap();
    let reader =
        ProjectionReader::open(&backend, &ready(&scope, &latest), &scopes, scope.clone()).unwrap();
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
            .write_batch(
                &all,
                &[super::contract::edge(4, EdgeFamily::CatalogDependency)],
                &[]
            )
            .is_err()
    );
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.pending.get(&4).is_none_or(Vec::is_empty));
    assert!(
        !backend
            .published
            .iter()
            .any(|(published_scope, _)| published_scope.generation_id == 4)
    );
}

#[cfg(test)]
#[path = "writer/extra.rs"]
mod extra;

#[cfg(not(windows))]
#[test]
fn fake_shared_readers_keep_old_generation_after_later_publication() {
    let (path, database, scopes) = scoped();
    super::contract_reads::pinned_generations(&mut Fake::default(), &mut Fake::default(), &scopes);
    drop(database);
    fs::remove_dir_all(path).unwrap();
}
