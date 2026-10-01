//! The config load and its resulting scope snapshot share one writer transaction.

use super::support::{Scratch, scope};
use crate::{
    scope::{Config, ConfigError, ConfigRefreshError, LOCAL},
    store::Error as StoreError,
};
use rusqlite::{Error as SqliteError, ErrorCode};
use std::error::Error as _;
use std::time::Duration;

#[test]
fn loading_config_holds_the_writer_lock_before_reading_the_file() {
    let scratch = Scratch::new();
    let database = scratch.open();
    scratch.configure("[access]\nread = []\n");
    let outside = scratch.outside();
    outside.busy_timeout(Duration::ZERO).unwrap();
    let scopes = database
        .refresh_config_inner(
            &scratch.0,
            Some(Box::new(|| {
                let attempted = outside.execute_batch("BEGIN IMMEDIATE");
                if attempted.is_ok() {
                    outside.execute_batch("ROLLBACK").unwrap();
                }
                assert!(
                    matches!(
                        attempted,
                        Err(SqliteError::SqliteFailure(error, _))
                            if error.code == ErrorCode::DatabaseBusy
                    ),
                    "a second writer entered while the configuration was loading"
                );
                Config::load(&scratch.0)
            })),
        )
        .unwrap();
    assert!(scopes.is_empty());
}

#[test]
fn refreshed_snapshots_exactly_match_reconciled_grants_including_revocations() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (text, expected) in [
        ("[access]\nread = []\n", vec![]),
        (
            "[access]\nread = ['workspace/default']\n",
            vec!["workspace/default"],
        ),
        (
            "[access]\nread = ['workspace/default/collection/docs', \
             'workspace/default/collection/other/source/guide']\n",
            vec![
                "workspace/default/collection/docs",
                "workspace/default/collection/other/source/guide",
            ],
        ),
        ("[access]\nread = []\n", vec![]),
    ] {
        scratch.configure(text);
        let refreshed = database.refresh_config(&scratch.0).unwrap();
        assert_eq!(refreshed, database.visible(LOCAL).unwrap());
        assert_eq!(
            refreshed.granted().cloned().collect::<Vec<_>>(),
            expected.into_iter().map(scope).collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_refused_refresh_rolls_back_and_preserves_the_previous_grants() {
    let scratch = Scratch::new();
    let database = scratch.open();
    scratch.configure("[access]\nread = ['workspace/default']\n");
    let admitted = database.refresh_config(&scratch.0).unwrap();
    scratch
        .outside()
        .execute_batch(
            "CREATE TRIGGER grants_are_kept BEFORE DELETE ON grants \
         BEGIN SELECT RAISE(ABORT, 'revocation refused'); END;",
        )
        .unwrap();
    scratch.configure("[access]\nread = []\n");
    assert!(matches!(
        database.refresh_config(&scratch.0),
        Err(ConfigRefreshError::Store(StoreError::Sqlite(_)))
    ));
    assert_eq!(database.visible(LOCAL).unwrap(), admitted);
    scratch.configure("[unknown]\n");
    assert!(matches!(
        database.refresh_config(&scratch.0),
        Err(ConfigRefreshError::Config(_))
    ));
    assert_eq!(database.visible(LOCAL).unwrap(), admitted);
}

#[test]
fn refresh_errors_show_and_chain_their_cause() {
    let unknown = || ConfigError::UnknownKey("access.write".to_owned());
    let migration = || StoreError::UnknownMigration("0099_future".to_owned());
    let cases: [(ConfigRefreshError, String); 2] = [
        (ConfigRefreshError::Config(unknown()), unknown().to_string()),
        (
            ConfigRefreshError::Store(migration()),
            migration().to_string(),
        ),
    ];
    for (error, cause) in cases {
        assert_eq!(error.to_string(), cause);
        assert_eq!(error.source().map(ToString::to_string), Some(cause));
    }
}
