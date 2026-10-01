//! Ownership admission for data-only Area registrations without a package hook.

use super::{
    area_packages::package_source,
    registry::glossary,
    support::{MemoryTree, check_by},
};
use crate::{
    limits::Limits,
    source::{Field, FieldType, Layout, Registry},
};

#[test]
fn registered_area_requires_ownership_record() {
    let mut descriptor = glossary();
    descriptor.directory.clear();
    descriptor.layout = Layout::Area {
        file: "package.toml".to_owned(),
    };
    descriptor.fields.extend([
        Field::optional("owners", FieldType::TextList),
        Field::optional("maintainers", FieldType::TextList),
    ]);
    let mut registry = Registry::default();
    registry.register(descriptor.clone()).unwrap();
    let root = package_source("package", "common");
    let source = format!(
        "term = \"evidence\"\n[metadata]{}",
        root.split_once("[metadata]").unwrap().1
    );
    let check = |declaration: &str| {
        check_by(
            &MemoryTree::default().with(
                "package.toml",
                &source.replace("[metadata]", &format!("{declaration}\n[metadata]")),
            ),
            &registry,
            &Limits::PRODUCTION,
        )
    };
    let catalog = check("owners = [\"reader\"]").unwrap();
    assert_eq!(
        catalog.ownership(&catalog.resources[0]).unwrap().owners,
        ["reader"]
    );
    for declaration in [
        "",
        "owners = []",
        "owners = [\"bad name\"]",
        "owners = [\"reader\", \"@READER\"]",
        "owners = [\"reader\"]\nmaintainers = [\"bad name\"]",
        "owners = [\"reader\"]\nmaintainers = [\"writer\", \"@WRITER\"]",
    ] {
        let result = check(declaration);
        assert!(
            result.is_err(),
            "area ownership must refuse {declaration:?}: {result:?}"
        );
    }
    let catalog = check("owners = [\"reader\"]\nmaintainers = [\"writer\"]").unwrap();
    assert_eq!(
        catalog
            .ownership(&catalog.resources[0])
            .unwrap()
            .maintainers,
        ["writer"]
    );
    for field in ["owners", "maintainers"] {
        let mut wrong_shape = descriptor.clone();
        wrong_shape
            .fields
            .iter_mut()
            .find(|entry| entry.key == field)
            .unwrap()
            .kind = FieldType::Text;
        let mut registry = Registry::default();
        registry.register(wrong_shape).unwrap();
        let declaration = if field == "owners" {
            "owners = \"reader\""
        } else {
            "owners = [\"reader\"]\nmaintainers = \"writer\""
        };
        let tree = MemoryTree::default().with(
            "package.toml",
            &source.replace("[metadata]", &format!("{declaration}\n[metadata]")),
        );
        let refusal = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap_err();
        assert!(
            refusal
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.key == field
                    && diagnostic.message.contains("list of strings"))
        );
    }
}
