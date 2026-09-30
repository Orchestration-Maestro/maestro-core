//! A public consumer uses only application IDs, pinned scopes, and edge families.

use maestro_kernel::{
    artifact::Digest,
    scope::{Right, Scope, ScopeSet, collection_path},
    store::Database,
};

use maestro_knowledge::graph::projection::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
use std::{error::Error, fs, path::Path, sync::Mutex};

struct FakePort(Mutex<Vec<ProjectionEdge>>);

impl TypedEdgeProjection for FakePort {
    fn write_edges(
        &mut self,
        scopes: &ScopeSet,
        edges: &[ProjectionEdge],
    ) -> Result<(), ProjectionError> {
        for edge in edges {
            if !scopes.covers(&scope_for(&edge.scope.collection_id)?) {
                return Err(ProjectionError::Unauthorized);
            }
        }
        self.0
            .get_mut()
            .map_err(|_| ProjectionError::Backend("fake lock poisoned".to_owned()))?
            .extend_from_slice(edges);
        Ok(())
    }

    fn neighbors(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, ProjectionError> {
        if !scopes.covers(&scope_for(&pin.collection_id)?) {
            return Err(ProjectionError::Unauthorized);
        }
        Ok(self
            .0
            .lock()
            .map_err(|_| ProjectionError::Backend("fake lock poisoned".to_owned()))?
            .iter()
            .filter(|edge| {
                edge.scope == *pin
                    && edge.family == family
                    && (edge.source == *entity || edge.target == *entity)
            })
            .cloned()
            .collect())
    }

    fn entity_facts(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        _subject: &Digest,
    ) -> Result<Vec<EntityFact>, ProjectionError> {
        if !scopes.covers(&scope_for(&pin.collection_id)?) {
            return Err(ProjectionError::Unauthorized);
        }
        Ok(Vec::new())
    }
}

fn scope_for(collection_id: &str) -> Result<Scope, ProjectionError> {
    collection_path(collection_id)
        .parse()
        .map_err(|_| ProjectionError::Invalid("invalid scope".to_owned()))
}

fn scopes(root: &Path) -> Result<(ScopeSet, ScopeSet), Box<dyn Error>> {
    let database = Database::open_in(root)?;
    database.grant(
        "projection-reader",
        &"workspace/default/collection/demo".parse::<Scope>()?,
        Right::Read,
        "test",
    )?;
    Ok((
        database.visible("projection-reader")?,
        database.visible("no-grants")?,
    ))
}

#[test]
fn public_port_pins_reads_to_application_ids_and_one_edge_family() -> Result<(), Box<dyn Error>> {
    let path = maestro_test_scratch::scratch_directory()?;
    let (scopes, denied) = scopes(&path)?;
    let pin = ProjectionScope {
        collection_id: "demo".to_owned(),
        generation_id: 8,
    };
    let source = Digest::of(b"source");
    let target = Digest::of(b"target");
    let records =
        [EdgeFamily::KnowledgeClaim, EdgeFamily::CatalogDependency].map(|family| ProjectionEdge {
            id: Digest::of(format!("{family:?}").as_bytes()),
            scope: pin.clone(),
            family,
            source: source.clone(),
            target: target.clone(),
            relation: "DEPENDS_ON".to_owned(),
        });
    let mut port = FakePort(Mutex::new(Vec::new()));
    port.write_edges(&scopes, &records)?;
    assert_eq!(
        port.neighbors(&scopes, &pin, EdgeFamily::KnowledgeClaim, &source)?,
        [records[0].clone()]
    );
    assert_eq!(
        port.neighbors(&scopes, &pin, EdgeFamily::CatalogDependency, &source)?,
        [records[1].clone()]
    );
    assert!(
        port.neighbors(&denied, &pin, EdgeFamily::KnowledgeClaim, &source)
            .is_err()
    );
    drop(port);
    fs::remove_dir_all(path)?;
    Ok(())
}
