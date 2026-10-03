//! Inventory claims remain exact and owner-local in the checked source tree.
use super::inventory::Fixture;
use crate::{
    limits::Limits,
    source::{Directory, Known, Refusal, builtin, check, frozen_rows},
};
use std::fs;

fn checked(fixture: &Fixture) -> Result<(), Refusal> {
    let registry = builtin().unwrap();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let rows = frozen_rows();
    check(
        &Directory::new(&fixture.catalog),
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .map(|_| ())
}

#[test]
fn listed_inventory_assets_accept_unlisted_assets_refuse() {
    let fixture = Fixture::new();
    checked(&fixture).unwrap();
    fs::write(
        fixture.catalog.join("bootstrap/base/files/unlisted.md"),
        b"unclaimed",
    )
    .unwrap();
    let error = checked(&fixture).unwrap_err().to_string();
    assert!(
        error.contains("bootstrap/base/files/unlisted.md") && error.contains("not a registered"),
        "{error}"
    );
    fs::remove_file(fixture.catalog.join("bootstrap/base/files/unlisted.md")).unwrap();
    checked(&fixture).unwrap();
}

#[test]
fn inventory_outside_registered_area_refuses() {
    let fixture = Fixture::new();
    checked(&fixture).unwrap();
    let target = fixture.catalog.join("outside/bootstrap");
    fs::create_dir_all(&target).unwrap();
    fs::copy(
        fixture.catalog.join("bootstrap/base.toml"),
        target.join("base.toml"),
    )
    .unwrap();
    let error = checked(&fixture).unwrap_err().to_string();
    assert!(
        error.contains("outside/bootstrap/base.toml") && error.contains("not a registered"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}

#[test]
fn inventory_asset_escape_and_identical_mapping_refuse() {
    for source in ["../outside", "instructions.md"] {
        let fixture = Fixture::new();
        checked(&fixture).unwrap();
        let path = fixture.catalog.join("bootstrap/base.toml");
        let text = fs::read_to_string(&path).unwrap();
        if source == "instructions.md" {
            let entry = text
                .split_once("[[files]]")
                .unwrap()
                .1
                .split_once("[metadata]")
                .unwrap()
                .0;
            fs::write(
                &path,
                text.replace("[metadata]", &format!("[[files]]{entry}\n[metadata]")),
            )
            .unwrap();
        } else {
            fixture.edit("bootstrap/base.toml", "instructions.md", source);
        }
        let error = checked(&fixture).unwrap_err().to_string();
        assert!(
            error.contains("unsafe preset file path")
                || error.contains("duplicate inventory source"),
            "{error}"
        );
    }
}

#[test]
fn checked_constructor_refuses_stale_codeowners() {
    let fixture = Fixture::new();
    let registry = builtin().unwrap();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let rows = frozen_rows();
    let catalog = check(
        &Directory::new(&fixture.catalog),
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap();
    fs::create_dir_all(fixture.catalog.join(".github")).unwrap();
    let path = fixture.catalog.join(".github/CODEOWNERS");
    fs::write(&path, catalog.codeowners().unwrap()).unwrap();
    assert!(fixture.port().is_ok());
    fs::write(path, b"# stale generated rules\n").unwrap();
    let error = fixture.port().unwrap_err();
    assert!(
        error.contains(".github/CODEOWNERS") && error.contains("rule drift"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}
