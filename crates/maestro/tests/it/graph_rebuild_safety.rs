//! Read-only preflight, explicit recovery and human output ordering.
#![cfg(all(feature = "engine", not(windows)))]
use crate::{
    graph_build::exited,
    graph_rebuild_fixture::{Fixture, tree},
};
use maestro_kernel::scope::LOCAL;

#[test]
fn graph_rebuild_nonverified_receiptless_generations_are_read_only() {
    for state in ["building", "failed", "published", "retired"] {
        let fixture = Fixture::new();
        let path = fixture.home.data().join("graph");
        let before = tree(&path);
        let connection =
            rusqlite::Connection::open(fixture.home.data().join("kernel.sqlite3")).unwrap();
        // Synthetic authority states preserve the already attached membership.
        connection
            .execute(
                "UPDATE generations SET state=?1 WHERE id=?2",
                rusqlite::params![state, fixture.generation],
            )
            .unwrap();
        let refused = fixture.run(&["--resolution", fixture.resolution.as_str()]);
        exited(&refused, 2);
        assert!(refused.stderr.contains("verified"), "{refused:?}");
        assert_eq!(tree(&path), before);
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM jobs WHERE kind='knowledge.graph.project'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn graph_rebuild_human_output_starts_with_job() {
    let fixture = Fixture::new();
    let built = fixture.home.run_in(
        &fixture.workspace,
        &[
            "--set",
            "graph.engine=ladybug",
            "knowledge",
            "graph",
            "rebuild",
            "--generation",
            &fixture.generation.to_string(),
            "--resolution",
            fixture.resolution.as_str(),
        ],
    );
    exited(&built, 0);
    assert!(built.stdout.starts_with("job "), "{built:?}");
}

#[test]
fn graph_rebuild_resource_less_interruption_is_preserved_and_resumable() {
    let fixture = Fixture::new();
    let (job, staging) = fixture.interrupted_without_resource();
    let before = tree(&staging);
    let refused = fixture.run(&["--resolution", fixture.resolution.as_str()]);
    exited(&refused, 2);
    assert!(
        refused
            .stderr
            .contains(&format!("repeat with --resume {job}")),
        "{refused:?}"
    );
    assert_eq!(tree(&staging), before);
    let db = fixture.home.database();
    let scopes = db.visible(LOCAL).unwrap();
    assert_eq!(
        db.job(&scopes, job).unwrap().unwrap().state.to_string(),
        "running"
    );
    let resumed = fixture.run(&[
        "--resolution",
        fixture.resolution.as_str(),
        "--resume",
        &job.to_string(),
    ]);
    exited(&resumed, 0);
    assert_eq!(
        db.job(&scopes, job).unwrap().unwrap().state.to_string(),
        "succeeded"
    );
}
