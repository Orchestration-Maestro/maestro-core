//! Serialized authoring locks stay within their replay read bound.
use super::{
    inventory::Fixture,
    support::{apply, preview},
};
use crate::{files::tests::support::remove, limits::Limits};
use std::fs;

#[test]
fn review_large_revision_cannot_publish_an_unreadable_lock() {
    let fixture = Fixture::new();
    let version = "x".repeat(600_000);
    fixture.edit(
        "core/agents/maestro.maestro.toml",
        "maturity = \"reviewed\"",
        &format!("version = \"{version}\"\nmaturity = \"reviewed\""),
    );
    let port = fixture.port().unwrap();
    let error = preview(&fixture.project, &port, &["base".into()]).unwrap_err();
    assert!(
        error.contains(".maestro/authoring.lock.json")
            && error.contains("1048576")
            && error.contains("limit"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}

#[test]
fn lock_at_the_reader_limit_is_accepted_and_replays() {
    let fixture = Fixture::new();
    let sidecar = "core/agents/maestro.maestro.toml";
    fixture.edit(
        sidecar,
        "maturity = \"reviewed\"",
        "version = \"x\"\nmaturity = \"reviewed\"",
    );
    let names = ["base".into()];
    let initial = preview(&fixture.project, &fixture.port().unwrap(), &names).unwrap();
    apply(&fixture.project, &initial).unwrap();
    let lock_path = fixture.project.join(".maestro/authoring.lock.json");
    let mut size = fs::metadata(&lock_path).unwrap().len();
    remove(&fixture.project, initial.plan.id()).unwrap();
    let limit = Limits::PRODUCTION.source_file_bytes;
    // The agent revision occurs twice; the single package revision fixes parity.
    if !(limit - size).is_multiple_of(2) {
        fixture.edit(
            "package.toml",
            "version = \"1.2.3\"",
            "version = \"1.2.34\"",
        );
        size += 1;
    }
    let revision_len = usize::try_from(1 + (limit - size) / 2).unwrap();
    fixture.edit(
        sidecar,
        "version = \"x\"",
        &format!("version = \"{}\"", "x".repeat(revision_len)),
    );
    let port = fixture.port().unwrap();
    let proposal = preview(&fixture.project, &port, &names).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert_eq!(fs::metadata(&lock_path).unwrap().len(), limit);
    let replay = preview(&fixture.project, &port, &names).unwrap();
    assert!(replay.plan.is_applied());
    apply(&fixture.project, &replay).unwrap();
}
