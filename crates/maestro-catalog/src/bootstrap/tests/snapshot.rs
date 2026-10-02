//! Preview bytes and decoded declarations come from one checked source snapshot.
use super::{
    super::PresetPort,
    inventory::Fixture,
    support::{apply, preview},
};
use std::fs;

#[test]
fn checked_snapshot_never_reopens_presets_inventories_or_payloads() {
    for path in [
        "presets/base.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ] {
        let fixture = Fixture::new();
        let port = fixture.port().unwrap();
        let expected = port.resolve(&["base".into()]).unwrap();
        let original = fs::read(fixture.catalog.join(path)).unwrap();
        fs::write(fixture.catalog.join(path), b"changed after check").unwrap();
        assert_eq!(port.resolve(&["base".into()]).unwrap(), expected);
        let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
        let error = apply(&fixture.project, &proposal).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("input changed; run preview again"),
            "{error}"
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
        fs::write(fixture.catalog.join(path), original).unwrap();
        let fresh = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap();
        apply(&fixture.project, &fresh).unwrap();
    }
}

#[test]
fn missing_declared_payload_refuses_at_source_check() {
    let fixture = Fixture::new();
    assert!(fixture.port().is_ok());
    fs::remove_file(fixture.catalog.join("bootstrap/base/files/instructions.md")).unwrap();
    let error = fixture.port().unwrap_err();
    assert!(
        error.contains("cannot read inventoried asset")
            && error.contains("bootstrap/base/files/instructions.md"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}
