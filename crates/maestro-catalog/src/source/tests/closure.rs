//! Implicit preset edges require both the package kind and its common/core name.

use super::support::{MemoryTree, check_by};
use crate::{
    limits::Limits,
    source::{Catalog, ResourceId, builtin},
};

#[test]
fn preset_implicit_packages_require_kind_and_name() {
    let registry = builtin().unwrap();
    let catalog = check_by(&MemoryTree::valid(), &registry, &Limits::PRODUCTION).unwrap();
    let preset = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "preset")
        .unwrap();
    let package = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "package")
        .unwrap();
    let candidates: Vec<_> = [
        "package:common",
        "package:core",
        "package:team",
        "language:common",
        "standard:security",
    ]
    .into_iter()
    .map(|id| {
        let mut resource = package.clone();
        resource.id = ResourceId::parse(id).unwrap();
        resource
    })
    .collect();
    let mut expected = preset.metadata.requires.clone();
    expected.extend(registry.edges(preset));
    expected.extend(
        ["package:common", "package:core", "standard:security"]
            .map(|id| ResourceId::parse(id).unwrap()),
    );
    expected.sort();
    expected.dedup();
    assert_eq!(
        Catalog::dependency_edges(preset, &registry, candidates.iter()),
        expected
    );
}
