//! Grants: a principal sees only what it was granted and what lies below it,
//! read anew for each request, whichever handle of the database changed it,
//! and each grant and revocation is journaled with its actor, in the write
//! that makes it.

use super::support::{CTM, Scratch, audit, read, record_in, scope};
use crate::{scope::Right, store};
use rusqlite::{ErrorCode, types::Type};
use serde_json::json;
use std::error;

#[test]
fn a_principal_sees_only_its_granted_scopes_and_their_descendants() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap();
    let garden = scope("workspace/default/collection/garden");
    database
        .grant("agent", &garden, Right::Read, "test")
        .unwrap();
    let visible = database.visible("local").unwrap();
    assert!(!visible.is_empty());
    assert!(visible.covers(&scope(CTM)));
    assert!(visible.covers(&scope(&format!("{CTM}/source/docs-core"))));
    for unseen in [
        "workspace/default",
        "workspace/default/collection/garden",
        "workspace/default/collection/ct",
        "workspace/default/collection/ctm-archive",
        "workspace/other/collection/ctm",
    ] {
        assert!(!visible.covers(&scope(unseen)), "{unseen}");
    }
    assert!(database.visible("agent").unwrap().covers(&garden));
}

#[test]
fn an_unknown_principal_or_scope_is_no_access() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap();
    let nobody = database.visible("nobody").unwrap();
    assert!(nobody.is_empty());
    for path in ["workspace/default", CTM] {
        assert!(!nobody.covers(&scope(path)), "{path}");
    }
    let unknown = "workspace/default/collection/unknown";
    assert!(!database.visible("local").unwrap().covers(&scope(unknown)));
    record_in(&database, "collection/unknown", unknown);
    let local = database.visible("local").unwrap();
    assert_eq!(read(&database, &local, "collection/unknown"), []);
}

#[test]
fn a_grant_and_its_revocation_are_journaled_once_with_their_actor() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let ctm = scope(CTM);
    let added = database
        .grant("local", &ctm, Right::Read, "config.toml")
        .unwrap()
        .unwrap();
    assert_eq!(added.r#type, "maestro.kernel.grant.added.v1");
    assert_eq!(added.stream, "principal/local");
    assert_eq!(added.subject, "principal/local");
    assert_eq!(added.scope, CTM);
    assert_eq!(
        added.data,
        json!({"principal": "local", "right": "read", "actor": "config.toml"})
    );
    let again = database
        .grant("local", &ctm, Right::Read, "someone")
        .unwrap();
    assert_eq!(again, None, "granting again records nothing");
    let revoked = database
        .revoke("local", &ctm, Right::Read, "operator")
        .unwrap()
        .unwrap();
    assert_eq!(revoked.r#type, "maestro.kernel.grant.revoked.v1");
    assert_eq!(revoked.stream, "principal/local");
    assert_eq!(revoked.subject, "principal/local");
    assert_eq!(revoked.scope, CTM);
    assert_eq!(
        revoked.data,
        json!({"principal": "local", "right": "read", "actor": "operator"})
    );
    let twice = database
        .revoke("local", &ctm, Right::Read, "operator")
        .unwrap();
    assert_eq!(twice, None, "revoking what is not granted records nothing");
    let auditor = audit(&database);
    assert_eq!(
        read(&database, &auditor, "principal/local"),
        [added, revoked]
    );
}

#[test]
fn a_revocation_through_another_handle_applies_to_the_next_read() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let event = record_in(&database, "collection/ctm", CTM);
    database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap();
    let before = database.visible("local").unwrap();
    assert_eq!(read(&database, &before, "collection/ctm"), [event]);
    let another = scratch.open();
    another
        .revoke("local", &scope(CTM), Right::Read, "operator")
        .unwrap();
    let after = database.visible("local").unwrap();
    assert!(!after.covers(&scope(CTM)));
    assert_eq!(read(&database, &after, "collection/ctm"), []);
}

#[test]
fn a_grant_whose_event_is_refused_is_not_granted() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "CREATE TRIGGER grant_events_are_refused BEFORE INSERT ON events
             WHEN NEW.type = 'maestro.kernel.grant.added.v1'
             BEGIN SELECT RAISE(ABORT, 'the test refuses the event of a grant'); END;",
        )
        .unwrap();
    let refusal = database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap_err();
    let reason = error::Error::source(&refusal).map(ToString::to_string);
    assert_eq!(
        reason.as_deref(),
        Some("the test refuses the event of a grant")
    );
    assert!(!database.visible("local").unwrap().covers(&scope(CTM)));
    let rows: (i64, i64) = outside
        .query_row(
            "SELECT (SELECT count(*) FROM grants), (SELECT count(*) FROM events)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(rows, (0, 0), "neither the grant nor its event is recorded");
}

#[test]
fn the_grants_table_holds_one_row_per_principal_scope_and_known_right() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap();
    let outside = scratch.outside();
    let row = outside
        .query_row(
            "SELECT principal, scope, right, granted_by, length(granted_at) FROM grants",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .unwrap();
    let expected = ("local".to_owned(), CTM.to_owned(), "read".to_owned());
    assert_eq!((row.0, row.1, row.2), expected);
    assert_eq!((row.3.as_str(), row.4), ("test", 24));
    for (sql, code) in [
        (
            "INSERT INTO grants (principal, scope, right, granted_by)
             VALUES ('local', 'workspace/default', 'write', 'test')",
            "CHECK constraint failed",
        ),
        (
            "INSERT INTO grants (principal, scope, right, granted_by)
             VALUES ('local', 'workspace/default/collection/ctm', 'read', 'other')",
            "UNIQUE constraint failed",
        ),
    ] {
        let refusal = outside.execute(sql, []).unwrap_err();
        assert_eq!(
            refusal.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation),
            "{sql}"
        );
        assert!(refusal.to_string().contains(code), "{sql}: {refusal}");
    }
}

#[test]
fn a_stored_grant_the_kernel_cannot_read_is_an_error_never_a_guess() {
    let scratch = Scratch::new();
    let database = scratch.open();
    scratch
        .outside()
        .execute(
            "INSERT INTO grants (principal, scope, right, granted_by)
             VALUES ('local', 'workspace/Default', 'read', 'test')",
            [],
        )
        .unwrap();
    let error = database.visible("local").unwrap_err();
    assert!(
        matches!(
            &error,
            store::Error::Sqlite(rusqlite::Error::FromSqlConversionFailure(0, Type::Text, _))
        ),
        "{error:?}"
    );
}
