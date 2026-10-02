//! Resolve-wide source capture and apply revalidation regressions.
use super::{
    super::{AreaInventories, PresetPort},
    inventory::Fixture,
    support::{apply, preview},
};
use crate::files::digest;
use std::fs;

#[test]
fn same_source_two_outputs_counts_once() {
    use crate::limits::Limits;
    let fixture = Fixture::new();
    let manifest = fixture.catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let entry = text.split_once("[[files]]").unwrap().1;
    let second = entry.replace(".github/copilot-instructions.md", "second.md");
    fs::write(manifest, format!("{text}\n[[files]]{second}")).unwrap();
    let bytes = [
        "presets/base.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ]
    .iter()
    .map(|path| fs::metadata(fixture.catalog.join(path)).unwrap().len())
    .sum::<u64>();
    let limits = Limits {
        archive_entries: 3,
        archive_total_bytes: bytes,
        ..Limits::PRODUCTION
    };
    let port = AreaInventories::new(&fixture.catalog, &fixture.areas).with_limits(limits);
    let resolved = port.resolve(&["base".into()]).unwrap();
    assert_eq!(resolved[0].files.len(), 2);
    assert_eq!(resolved[0].source_files.len(), 3);
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert_eq!(
        fs::read(fixture.project.join(".github/copilot-instructions.md")).unwrap(),
        fs::read(fixture.project.join("second.md")).unwrap()
    );
}

#[test]
fn same_source_different_digests_refuses() {
    let fixture = Fixture::new();
    let manifest = fixture.catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let entry = text.split_once("[[files]]").unwrap().1;
    let old = entry
        .lines()
        .find(|line| line.starts_with("sha256 ="))
        .unwrap();
    let second = entry
        .replace(".github/copilot-instructions.md", "second.md")
        .replace(old, &format!("sha256 = \"{}\"", digest(b"different\n")));
    fs::write(manifest, format!("{text}\n[[files]]{second}")).unwrap();
    fixture.refuses(
        &["base".into()],
        "inventory digest mismatch: bootstrap/base/files/instructions.md",
    );
}

#[test]
fn shared_source_across_presets_counts_once() {
    use crate::limits::Limits;
    let mut fixture = Fixture::new();
    fixture.areas.insert("alias".into(), String::new());
    fixture.edit("presets/rust.toml", "rust/starter", "alias/base");
    let bytes = [
        "presets/base.toml",
        "presets/rust.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ]
    .iter()
    .map(|path| fs::metadata(fixture.catalog.join(path)).unwrap().len())
    .sum::<u64>();
    let limits = Limits {
        archive_entries: 4,
        archive_total_bytes: bytes,
        ..Limits::PRODUCTION
    };
    let port = AreaInventories::new(&fixture.catalog, &fixture.areas).with_limits(limits);
    let resolved = port.resolve(&["base".into(), "rust".into()]).unwrap();
    let path = "bootstrap/base/files/instructions.md";
    assert_eq!(resolved[0].source_files.len(), 3);
    assert_eq!(resolved[1].source_files.len(), 3);
    assert_eq!(
        resolved[0].source_files[path],
        resolved[1].source_files[path]
    );
}

#[test]
fn deleted_input_refuses_before_writes() {
    for path in [
        "presets/base.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ] {
        let fixture = Fixture::new();
        let port = AreaInventories::new(&fixture.catalog, &fixture.areas);
        let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
        fs::remove_file(fixture.catalog.join(path)).unwrap();
        let error = apply(&fixture.project, &proposal).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("input changed; run preview again"),
            "{error}"
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    }
}

#[test]
fn aggregate_bounds_cross_presets() {
    use crate::limits::Limits;
    let fixture = Fixture::new();
    let bytes = [
        "presets/base.toml",
        "presets/rust.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
        "languages/rust/bootstrap/starter.toml",
        "languages/rust/bootstrap/starter/files/recipes.json",
    ]
    .iter()
    .map(|path| fs::metadata(fixture.catalog.join(path)).unwrap().len())
    .sum::<u64>();
    let limits = Limits {
        archive_entries: 6,
        archive_total_bytes: bytes,
        ..Limits::PRODUCTION
    };
    let names = ["base".into(), "rust".into()];
    assert!(
        AreaInventories::new(&fixture.catalog, &fixture.areas)
            .with_limits(limits)
            .resolve(&names)
            .is_ok()
    );
    for (limits, message) in [
        (
            Limits {
                archive_entries: 5,
                ..limits
            },
            "source count exceeds limit",
        ),
        (
            Limits {
                archive_total_bytes: bytes - 1,
                ..limits
            },
            "source bytes exceed limit",
        ),
    ] {
        let error = AreaInventories::new(&fixture.catalog, &fixture.areas)
            .with_limits(limits)
            .resolve(&names)
            .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}
