//! G25: create, insert, query, close and reopen an on-disk `LadybugDB`, and
//! record what a second open of the same files does, in this process and in
//! another one.

use std::env::{current_exe, temp_dir, var_os};
use std::fs::{create_dir_all, remove_dir_all};
use std::io;
use std::path::PathBuf;
use std::process::{Command, id};

use lbug::{Connection, Database, Error, SystemConfig, Value};

/// The variable that turns `child_opens_the_path` into the other process.
const CHILD_PATH: &str = "LBUG_SPIKE_CHILD_PATH";

/// A fresh scratch path under the system temporary directory.
fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = temp_dir().join(format!("lbug-spike-{name}-{}", id()));
    let _ignored = remove_dir_all(&dir);
    create_dir_all(&dir)?;
    Ok(dir.join("graph.lbug"))
}

/// The names of the people `conn` knows about, in name order.
fn names(conn: &Connection<'_>) -> Result<Vec<String>, Error> {
    let rows = conn.query("MATCH (p:Person) RETURN p.name ORDER BY p.name;")?;
    Ok(rows
        .filter_map(|row| row.first().map(ToString::to_string))
        .collect())
}

#[test]
fn create_insert_query_reopen() {
    let path = scratch("reopen").expect("scratch");
    {
        let db = Database::new(&path, SystemConfig::default()).expect("create database");
        let conn = Connection::new(&db).expect("connect");
        conn.query("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name));")
            .expect("create node table");
        conn.query("CREATE REL TABLE Knows(FROM Person TO Person, since INT64);")
            .expect("create rel table");
        let mut insert = conn
            .prepare("CREATE (:Person {name: $name});")
            .expect("prepare insert");
        for name in ["Alice", "Bob", "Carol"] {
            conn.execute(&mut insert, vec![("name", Value::String(name.into()))])
                .expect("insert person");
        }
        conn.query(
            "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) \
             CREATE (a)-[:Knows {since: 2024}]->(b);",
        )
        .expect("insert edge");
        assert_eq!(names(&conn).expect("names"), ["Alice", "Bob", "Carol"]);
    }
    let db = Database::new(&path, SystemConfig::default().read_only(true)).expect("reopen");
    let conn = Connection::new(&db).expect("reconnect");
    assert_eq!(names(&conn).expect("names"), ["Alice", "Bob", "Carol"]);
    let hops: Vec<String> = conn
        .query("MATCH (:Person {name: 'Alice'})-[k:Knows]->(b:Person) RETURN b.name, k.since;")
        .expect("one hop")
        .map(|row| format!("{}@{}", row[0], row[1]))
        .collect();
    assert_eq!(hops, ["Bob@2024"]);
    let refused = conn.query("CREATE (:Person {name: 'Dave'});");
    assert!(refused.is_err(), "a read-only database refuses writes");
}

#[test]
fn second_writer_in_one_process_is_reported() {
    let path = scratch("second").expect("scratch");
    let first = Database::new(&path, SystemConfig::default()).expect("first writer");
    let second = Database::new(&path, SystemConfig::default());
    let reader = Database::new(&path, SystemConfig::default().read_only(true));
    println!(
        "second writer: {:?}; read-only beside writer: {:?}",
        second
            .as_ref()
            .map(|_| "opened")
            .map_err(ToString::to_string),
        reader
            .as_ref()
            .map(|_| "opened")
            .map_err(ToString::to_string),
    );
    drop(first);
}

/// Run by `writer_in_another_process_is_reported` in a second process: it
/// opens the path it is given as a writer, then read-only, and reports both.
#[test]
fn child_opens_the_path() {
    let Some(path) = var_os(CHILD_PATH) else {
        return;
    };
    let writer = Database::new(&path, SystemConfig::default()).map(|_| "opened");
    let reader = Database::new(&path, SystemConfig::default().read_only(true)).map(|_| "opened");
    println!(
        "CHILD writer: {:?}; read-only: {:?}",
        writer.map_err(|error| error.to_string()),
        reader.map_err(|error| error.to_string()),
    );
}

#[test]
fn writer_in_another_process_is_reported() {
    let path = scratch("process").expect("scratch");
    let first = Database::new(&path, SystemConfig::default()).expect("parent writer");
    let output = Command::new(current_exe().expect("test binary"))
        .args([
            "engine_qualification::child_opens_the_path",
            "--exact",
            "--nocapture",
        ])
        .env(CHILD_PATH, &path)
        .output()
        .expect("run the child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report = stdout
        .lines()
        .find(|line| line.starts_with("CHILD"))
        .expect("the child reports");
    println!("while the parent writes: {report}");
    drop(first);
}
