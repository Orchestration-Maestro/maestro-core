//! Standalone 0031 qualification; runtime registration waits for build-bound ports.
use super::projection_schema_support::Fixture;

#[test]
fn projection_build_schema_installs_every_authority_guard() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    for name in [
        "graph_projection_builds_admit",
        "graph_projection_builds_never_replaced",
        "graph_projection_builds_never_changed",
        "graph_projection_builds_never_deleted",
        "graph_projection_receipts_match_build",
        "graph_projection_receipts_never_replaced",
        "graph_projection_receipts_never_changed",
        "graph_projection_receipts_never_deleted",
        "graph_projection_active_admit",
        "graph_projection_active_advance",
        "graph_projection_active_never_deleted",
        "graph_projection_active_never_replaced",
    ] {
        let present: bool = fixture
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'trigger' AND name = ?1)",
                [name],
                |row| row.get(0),
            )
            .unwrap();
        assert!(present, "missing authority guard: {name}");
    }
}

#[test]
fn projection_build_schema_retains_two_receipts_and_atomically_selects_the_successor() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let old = fixture.generation();
    let build = fixture.reserve(Some(old));
    assert!(build > old);
    fixture.insert_receipt(build, "replacement.db").unwrap();
    fixture.advance(build).unwrap();
    assert_eq!(fixture.head(), build);
    assert_eq!(fixture.count("graph_projection_receipts"), 2);
    assert_eq!(fixture.count("graph_projection_builds"), 2);
    fixture.foreign_keys_clean();
}
