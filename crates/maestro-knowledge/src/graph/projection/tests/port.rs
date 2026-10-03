//! Application-ID projection model tests.

use super::super::{EdgeFamily, ProjectionEdge, ProjectionScope};
use maestro_kernel::{artifact::Digest, facts::Predicate};

#[test]
fn projection_edges_carry_application_identity_scope_and_family() {
    let edge = ProjectionEdge {
        id: Digest::of(b"edge"),
        scope: ProjectionScope {
            collection_id: "catalog".to_owned(),
            generation_id: 17,
        },
        family: EdgeFamily::CatalogDependency,
        source: Digest::of(b"source"),
        target: Digest::of(b"target"),
        relation: "depends_on".to_owned(),
    };

    assert_eq!(edge.family, EdgeFamily::CatalogDependency);
    assert_ne!(edge.id, edge.source);
    assert_eq!(edge.scope.generation_id, 17);
    assert_ne!(edge.relation, Predicate::DependsOn.as_str());
}
