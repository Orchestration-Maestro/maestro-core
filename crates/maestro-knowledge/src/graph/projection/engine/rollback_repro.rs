//! Frozen native checkpoint/rollback matrix for `LadybugDB` PR 1049.
use super::{
    open::open,
    tests::{Fixture, config},
};
use lbug::{Connection, Value};

#[test]
fn native_hash_index_rollback_repro_matrix() {
    let mut failed = Vec::new();
    for (name, checkpoint_schema, committed, key, cycles, rollback) in cases() {
        let fixture = Fixture::new();
        let expected = i64::from(committed);
        {
            let db = fixture.writer();
            let conn = Connection::new(&db).unwrap();
            conn.query("CREATE NODE TABLE Test(id STRING, PRIMARY KEY(id))")
                .unwrap();
            if checkpoint_schema {
                conn.query("CHECKPOINT").unwrap();
            }
            let mut insert = conn.prepare("CREATE (:Test {id: $id})").unwrap();
            if committed {
                conn.execute(&mut insert, vec![("id", Value::String("committed".into()))])
                    .unwrap();
            }
            apply_cycles(&conn, &key, cycles, rollback);
            assert_eq!(
                conn.query("MATCH (e:Test) RETURN count(e)")
                    .unwrap()
                    .next()
                    .unwrap(),
                [Value::Int64(expected)]
            );
            conn.query("CHECKPOINT").unwrap();
        }
        match reopened_count(&fixture) {
            Ok(row) => {
                eprintln!("MATRIX {name}: REOPEN_OK count={row:?}");
                assert_eq!(row, [Value::Int64(expected)]);
            }
            Err(error) => {
                eprintln!("MATRIX {name}: REOPEN_FAILED {error}");
                failed.push(name);
            }
        }
    }
    assert!(failed.is_empty(), "native matrix failures: {failed:?}");
}

/// Exact rollback/commit cycles, extracted without changing the reproduction.
fn apply_cycles(connection: &Connection<'_>, key: &str, cycles: usize, rollback: bool) {
    let mut insert = connection.prepare("CREATE (:Test {id: $id})").unwrap();
    for index in 0..cycles {
        connection.query("BEGIN TRANSACTION").unwrap();
        connection
            .execute(
                &mut insert,
                vec![("id", Value::String(format!("{key}{index}")))],
            )
            .unwrap();
        connection
            .query(if rollback { "ROLLBACK" } else { "COMMIT" })
            .unwrap();
        if index > 0 {
            connection.query("BEGIN TRANSACTION").unwrap();
            connection
                .execute(
                    &mut insert,
                    vec![("id", Value::String(format!("keep{index}")))],
                )
                .unwrap();
            connection.query("COMMIT").unwrap();
            connection
                .query("MATCH (e:Test) WHERE e.id STARTS WITH 'keep' DELETE e")
                .unwrap();
        }
    }
}

/// Own and drop the actual read-only handle; never reuse a writer database.
fn reopened_count(fixture: &Fixture) -> Result<Vec<Value>, String> {
    let database = open(&fixture.root, "rows.lbdb", config().read_only(true))
        .map_err(|error| error.to_string())?;
    let connection = Connection::new(&database).unwrap();
    Ok(connection
        .query("MATCH (e:Test) RETURN count(e)")
        .unwrap()
        .next()
        .unwrap())
}

/// Name, schema checkpoint, committed seed, key, cycles and transaction outcome.
type ReproCase = (&'static str, bool, bool, String, usize, bool);

/// The original eight cases, including all four pinned-fork failures unchanged.
fn cases() -> [ReproCase; 8] {
    [
        (
            "empty_short_control",
            false,
            false,
            "short".to_owned(),
            0,
            false,
        ),
        (
            "fresh_short_rollback",
            false,
            false,
            "short".to_owned(),
            1,
            true,
        ),
        (
            "fresh_long_rollback",
            false,
            false,
            "x".repeat(4096),
            1,
            true,
        ),
        (
            "a_schema_checkpoint_fresh",
            true,
            false,
            "short".to_owned(),
            1,
            true,
        ),
        (
            "b_schema_checkpoint_committed",
            true,
            true,
            "short".to_owned(),
            1,
            true,
        ),
        (
            "c_schema_checkpoint_overflow",
            true,
            false,
            "x".repeat(4096),
            1,
            true,
        ),
        (
            "d_schema_checkpoint_cycles",
            true,
            true,
            "x".repeat(4096),
            4,
            true,
        ),
        (
            "committed_rollback_control",
            false,
            true,
            "short".to_owned(),
            1,
            true,
        ),
    ]
}
