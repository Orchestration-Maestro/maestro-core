//! `maestro doctor` as its users run it (FR-S1-015): every check, each
//! failure with its next action; the model router it cannot reach named
//! with the address it tried; what it finds but must not touch, listed and
//! left as it was; and the kernel's database never created by it, nor
//! migrated by it or by `maestro status`.

use super::{
    machine::{checked, checked_with, nothing_at},
    support::Home,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{fs, path::Path, process};

const DATABASE_TEMPORARY_WARNING: &str = concat!(
    "may be another name of the live database; never open it; ",
    "inspect after no creation is in progress before removing only this name"
);

/// The checks of the document doctor printed.
fn checks(document: &Value) -> &[Value] {
    document["checks"].as_array().unwrap()
}

/// The check named `name` in `document`.
fn check<'a>(document: &'a Value, name: &str) -> &'a Value {
    checks(document)
        .iter()
        .find(|check| check["name"] == name)
        .unwrap()
}

#[test]
fn every_failed_check_names_its_next_action() {
    let home = Home::bare();
    let ended = checked(&home, &["doctor", "--json"]);
    assert_eq!(ended.code, Some(1), "{ended:?}");
    assert_eq!(ended.stderr, "", "{ended:?}");
    let document = ended.json();
    assert_eq!(document["schema"], "maestro-cli/doctor/1");
    let names: Vec<&str> = checks(&document)
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "config",
            "bindings",
            "database",
            "artifacts",
            "qdrant",
            "router",
            "model_card",
            "model_card",
            "model_card"
        ]
    );
    for check in checks(&document) {
        let next = &check["next_action"];
        if check["passed"] == false {
            assert!(
                next.as_str().is_some_and(|next| !next.is_empty()),
                "a failure names its next action: {check}"
            );
        } else {
            assert_eq!(check["passed"], true, "{check}");
            assert!(next.is_null(), "{check}");
        }
    }
    assert_eq!(check(&document, "config")["detail"], "valid");
    assert_eq!(
        check(&document, "database")["detail"],
        "no kernel database yet"
    );
    assert_eq!(check(&document, "qdrant")["target"], "http://127.0.0.1:0");
    assert!(
        !home.data().join("kernel.sqlite3").exists(),
        "doctor never creates the kernel's database"
    );
}

#[test]
fn the_report_for_people_puts_each_next_action_under_its_failure() {
    let home = Home::bare();
    let text = checked(&home, &["doctor"]);
    assert_eq!(text.code, Some(1), "{text:?}");
    let lines: Vec<&str> = text.stdout.lines().collect();
    let failures = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("FAIL"))
        .inspect(|(index, line)| {
            assert!(
                lines[index + 1].trim_start().starts_with("next: "),
                "{line} is followed by its next action:\n{}",
                text.stdout
            );
        })
        .count();
    assert!(failures >= 6, "{}", text.stdout);
    assert!(
        lines[0].starts_with("ok    config ") && lines[0].ends_with("config.toml: valid"),
        "{}",
        lines[0]
    );
    assert_eq!(
        lines.last().copied(),
        Some(format!("{failures} of 9 checks failed.").as_str())
    );
    assert!(
        !text.stdout.contains("left untouched") && !text.stdout.contains("reach no known scope"),
        "nothing to list: {}",
        text.stdout
    );
}

#[test]
fn an_unreachable_router_is_named_with_the_address_it_tried_and_the_next_action() {
    let home = Home::bare();
    let address = nothing_at();
    let document = checked_with(&home, &address, &["doctor", "--json"]).json();
    let router = check(&document, "router");
    assert_eq!(router["target"], format!("{address}/"));
    assert_eq!(router["passed"], false);
    let next = router["next_action"].as_str().unwrap();
    assert!(
        next.contains("maestro-model-router") && next.contains(&address),
        "{next}"
    );
}

#[test]
fn doctor_lists_what_it_must_not_touch_and_leaves_it_as_it_was() {
    let home = Home::new();
    home.configure(
        "[access]\nread = ['workspace/default', 'workspace/other', \
         'workspace/default/collection/absent']\n",
    );
    home.add_synthetic();
    let data = home.data();
    fs::write(data.join("ledger.sqlite3"), "maestro v1's ledger").unwrap();
    fs::write(data.join("maestro.sock"), "").unwrap();
    fs::create_dir(data.join("material")).unwrap();
    fs::write(data.join("material").join("page.html"), "v1").unwrap();
    let document = checked(&home, &["doctor", "--json"]).json();
    let text = checked(&home, &["doctor"]).stdout;
    let shown = |name: &str| data.join(name).display().to_string();
    for line in [
        format!(
            "Not the kernel's, left untouched: {}, {}, {}",
            shown("ledger.sqlite3"),
            shown("maestro.sock"),
            shown("material")
        ),
        "Grants in config.toml that reach no known scope: \
         workspace/default/collection/absent, workspace/other"
            .to_owned(),
    ] {
        assert!(text.contains(&line), "{line} is missing from:\n{text}");
    }
    assert_eq!(
        document["untouched"],
        json!([
            shown("ledger.sqlite3"),
            shown("maestro.sock"),
            shown("material")
        ])
    );
    assert_eq!(
        document["unreached_grants"],
        json!(["workspace/default/collection/absent", "workspace/other"])
    );
    for name in ["config", "bindings", "database", "artifacts"] {
        assert_eq!(check(&document, name)["passed"], true, "{document}");
    }
    assert_eq!(
        fs::read_to_string(data.join("ledger.sqlite3")).unwrap(),
        "maestro v1's ledger"
    );
    assert_eq!(
        fs::read_to_string(data.join("material").join("page.html")).unwrap(),
        "v1"
    );
    assert!(data.join("maestro.sock").exists());
}

fn database_state(database: &Path) -> (Vec<String>, i64) {
    let connection = Connection::open(database).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT scope FROM grants WHERE principal = 'local' AND right = 'read' \
             ORDER BY scope",
        )
        .unwrap();
    let scopes = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let migrations = connection
        .query_row("SELECT count(*) FROM migrations", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
    (scopes, migrations)
}

fn assert_configured_database(database: &Path, migration_count: i64) {
    let (scopes, migrations) = database_state(database);
    assert_eq!(scopes, ["workspace/default"]);
    assert_eq!(migrations, migration_count);
}

fn failed_check_count(document: &Value) -> usize {
    checks(document)
        .iter()
        .filter(|check| check["passed"] == false)
        .count()
}

fn assert_database_temporary_warning(document: &Value, temporary: &str, unrelated: &str) {
    assert_eq!(document["database_temporaries"], json!([temporary]));
    assert_eq!(
        document["database_temporary_warning"],
        DATABASE_TEMPORARY_WARNING
    );
    assert_eq!(document["untouched"], json!([unrelated]));
}

fn assert_temporary_database_sidecars_absent(data: &Path, temporary_name: &str) {
    for suffix in ["-wal", "-shm"] {
        assert!(
            !data.join(format!("{temporary_name}{suffix}")).exists(),
            "doctor must not open the temporary database name"
        );
    }
}

#[test]
fn doctor_warns_about_database_temporaries_without_a_failed_check() {
    let home = Home::bare();
    home.configure("[access]\nread = ['workspace/default']\n");
    drop(home.database());
    let data = home.data();
    let database = data.join("kernel.sqlite3");
    let unrelated = data.join("kernel.sqlite3.notes");
    fs::write(&unrelated, "leave this file alone").unwrap();
    let migration_count = database_state(&database).1;
    let baseline = checked(&home, &["doctor", "--json"]);
    let baseline_document = baseline.json();
    assert_eq!(baseline_document["database_temporaries"], json!([]));
    assert_eq!(baseline_document["database_temporary_warning"], Value::Null);
    assert_configured_database(&database, migration_count);

    let temporary_name = format!("kernel.sqlite3.tmp-{}-0", process::id());
    let temporary = data.join(&temporary_name);
    fs::hard_link(&database, &temporary).unwrap();
    let temporary_path = temporary.display().to_string();
    let unrelated_path = unrelated.display().to_string();
    let unrelated_before = fs::read(&unrelated).unwrap();
    let with_temporary = checked(&home, &["doctor", "--json"]);
    let document = with_temporary.json();
    assert_eq!(with_temporary.code, baseline.code);
    assert_eq!(checks(&document).len(), checks(&baseline_document).len());
    assert_eq!(
        failed_check_count(&document),
        failed_check_count(&baseline_document)
    );
    assert_database_temporary_warning(&document, &temporary_path, &unrelated_path);
    assert_configured_database(&database, migration_count);

    let text = checked(&home, &["doctor"]);
    assert_eq!(text.code, baseline.code);
    assert!(
        text.stdout.contains(DATABASE_TEMPORARY_WARNING),
        "{}",
        text.stdout
    );
    assert!(text.stdout.contains(&temporary_path), "{}", text.stdout);
    assert!(text.stdout.contains(&unrelated_path), "{}", text.stdout);
    assert!(temporary.exists());
    assert_eq!(fs::read(&temporary).unwrap(), fs::read(&database).unwrap());
    assert_eq!(fs::read(&unrelated).unwrap(), unrelated_before);
    assert_temporary_database_sidecars_absent(&data, &temporary_name);
}

#[test]
fn a_kernel_that_lacks_a_migration_is_reported_by_doctor_and_status_never_migrated() {
    let home = Home::new();
    home.add_synthetic();
    let file = home.data().join("kernel.sqlite3");
    let recorded = |name: &str| -> i64 {
        Connection::open(&file)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM migrations WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .unwrap()
    };
    // As a kernel a maestro without the last migration opened last.
    let outside = Connection::open(&file).unwrap();
    let last: String = outside
        .query_row("SELECT max(name) FROM migrations", [], |row| row.get(0))
        .unwrap();
    outside
        .execute("DELETE FROM migrations WHERE name = ?1", [&last])
        .unwrap();
    drop(outside);
    let lacking = format!(
        "the kernel database lacks {last}, which this maestro applies when a command opens it"
    );
    let doctor = checked(&home, &["doctor", "--json"]);
    assert_eq!(doctor.code, Some(1), "{doctor:?}");
    let doctor = doctor.json();
    let database = check(&doctor, "database");
    assert_eq!(
        (&database["passed"], &database["detail"]),
        (&json!(false), &json!(lacking)),
        "{doctor}"
    );
    assert!(
        database["next_action"]
            .as_str()
            .is_some_and(|next| next.contains("any `maestro knowledge` command applies it")),
        "{doctor}"
    );
    let status = checked(&home, &["status", "--json"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    let status = status.json();
    let kernel = &status["services"][0];
    assert_eq!(
        (&kernel["name"], &kernel["ready"], &kernel["detail"]),
        (&json!("kernel"), &json!(false), &json!(lacking)),
        "{status}"
    );
    assert_eq!(
        recorded(&last),
        0,
        "neither doctor nor status applied {last}"
    );
}
