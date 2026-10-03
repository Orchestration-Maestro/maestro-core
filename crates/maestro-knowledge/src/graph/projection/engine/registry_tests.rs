//! Registry ownership and physical root normalization through public factories.
use super::{
    public_fixture::{Fixture, settings},
    public_tests::publish,
    registry::owner_count,
};
use crate::graph::projection::{ProjectionEngine, ProjectionFactory, ProjectionHandle, content};
use maestro_filesystem::SystemFileLock;
use std::path::Path;

/// Open a supported factory spelling of the same fixture's owned root.
fn reader(fixture: &Fixture, path: &Path) -> ProjectionHandle {
    ProjectionFactory::new(path, ProjectionEngine::Ladybug, settings(), &SystemFileLock)
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap()
}

#[test]
fn lifecycle_registry_shares_root_spellings_and_separates_other_roots() {
    let fixture = Fixture::new();
    publish(&fixture);
    let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
    let path = &fixture.native.path;
    let alias = path
        .parent()
        .unwrap()
        .join(".")
        .join(path.file_name().unwrap());
    assert_ne!(path.as_os_str(), alias.as_os_str());
    let first = reader(&fixture, path);
    let second = reader(&fixture, &alias);
    assert_eq!(owner_count(path, &name), 3);
    assert_eq!(owner_count(&alias, &name), 3);
    #[cfg(unix)]
    {
        use crate::graph::projection::engine::open::tests::Scratch;
        use std::os::unix::fs::symlink;

        let linked_parent = Scratch::new();
        symlink(path.parent().unwrap(), linked_parent.0.join("parent")).unwrap();
        let linked = linked_parent
            .0
            .join("parent")
            .join(path.file_name().unwrap());
        let third = reader(&fixture, &linked);
        assert_eq!(owner_count(&linked, &name), 4);
        assert_eq!(owner_count(path, &name), 4);
        drop(third);
    }
    assert_eq!(owner_count(path, &name), 3);

    let other = Fixture::new();
    publish(&other);
    let other_name = content::basename(&other.build.scope, &other.build.claim_set_id).unwrap();
    let separate = reader(&other, &other.native.path);
    assert_eq!(owner_count(&other.native.path, &other_name), 2);
    assert_eq!(owner_count(path, &name), 3);
    drop(separate);
    assert_eq!(owner_count(&other.native.path, &other_name), 0);
    drop(first);
    assert_eq!(owner_count(&alias, &name), 2);
    drop(second);
    assert_eq!(owner_count(path, &name), 0);
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn lifecycle_registry_shares_case_swapped_owned_leaf() {
    let fixture = Fixture::new();
    publish(&fixture);
    let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
    let path = &fixture.native.path;
    let leaf = path.file_name().unwrap().to_str().unwrap();
    let alias = path.with_file_name(leaf.to_uppercase());
    assert_ne!(path.as_os_str(), alias.as_os_str());
    let first = reader(&fixture, path);
    let second = reader(&fixture, &alias);
    assert_eq!(owner_count(path, &name), 3);
    assert_eq!(owner_count(&alias, &name), 3);
    drop(first);
    drop(second);
    assert_eq!(owner_count(&alias, &name), 0);
}
