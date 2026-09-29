//! The journal of settings changes: each change recorded with its time and
//! read back for its principal alone.

use super::{CHANGED, SettingChange};
use crate::{
    journal::{Filter, NewEvent},
    scope::{Right, Scope, WORKSPACE},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{fs, path::PathBuf};

/// A scratch data directory, removed when dropped after its database.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// A change of `key` by `principal`, from `old` to `new`.
fn change(principal: &str, key: &str, old: Option<i64>, new: Option<i64>) -> SettingChange {
    SettingChange {
        principal: principal.to_owned(),
        key: key.to_owned(),
        old: old.map(Into::into),
        new: new.map(Into::into),
        layer: "user".to_owned(),
        file: "/config/maestro/preferences.toml".to_owned(),
    }
}

#[test]
fn record_setting_change_journals_the_change_and_setting_changes_read_it_back_in_order() {
    let scratch = Scratch(scratch_directory().unwrap());
    let database = Database::open_in(&scratch.0).unwrap();
    let first = change("local", "search.k", None, Some(20));
    let second = change("local", "search.k", Some(20), None);
    let recorded = database.record_setting_change(&first).unwrap();
    assert_eq!(recorded.change, first);
    database.record_setting_change(&second).unwrap();

    let changes = database.setting_changes("local").unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0], recorded);
    assert_eq!(changes[1].change, second);
    assert!(changes[0].id < changes[1].id);
    assert!(changes[0].time.ends_with('Z'), "{}", changes[0].time);

    let event = &database
        .events(
            &database.visible("nobody").unwrap(),
            &Filter {
                stream: "principal/local/settings",
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert!(event.is_empty(), "the events stay scoped: {event:?}");
}

#[test]
fn setting_changes_never_return_another_principal_s_changes() {
    let scratch = Scratch(scratch_directory().unwrap());
    let database = Database::open_in(&scratch.0).unwrap();
    database
        .record_setting_change(&change("local", "tone", None, Some(1)))
        .unwrap();
    database
        .record_setting_change(&change("other", "tone", None, Some(2)))
        .unwrap();
    let forged = json!(change("other", "tone", None, Some(3)));
    database
        .record(&NewEvent {
            stream: "principal/local/settings",
            r#type: CHANGED,
            subject: "setting/tone",
            scope: WORKSPACE,
            data: &forged,
        })
        .unwrap();

    let local = database.setting_changes("local").unwrap();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].change, change("local", "tone", None, Some(1)));
    let other = database.setting_changes("other").unwrap();
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].change.new, Some(json!(2)));
    assert!(database.setting_changes("nobody").unwrap().is_empty());
}

#[test]
fn setting_changes_refuse_an_event_they_cannot_read() {
    let scratch = Scratch(scratch_directory().unwrap());
    let database = Database::open_in(&scratch.0).unwrap();
    database
        .record(&NewEvent {
            stream: "principal/local/settings",
            r#type: CHANGED,
            subject: "setting/tone",
            scope: WORKSPACE,
            data: &json!({"key": "tone"}),
        })
        .unwrap();
    assert!(database.setting_changes("local").is_err());
}

#[test]
fn a_principal_granted_the_workspace_never_reads_another_s_settings_events() {
    let scratch = Scratch(scratch_directory().unwrap());
    let database = Database::open_in(&scratch.0).unwrap();
    database
        .record_setting_change(&change("local", "tone", None, Some(1)))
        .unwrap();
    let workspace: Scope = WORKSPACE.parse().unwrap();
    database
        .grant("auditor", &workspace, Right::Read, "operator")
        .unwrap();
    let auditor = database.visible("auditor").unwrap();
    for r#type in [None, Some(CHANGED)] {
        let events = database
            .events(
                &auditor,
                &Filter {
                    stream: "principal/local/settings",
                    after: 0,
                    r#type,
                },
            )
            .unwrap();
        assert!(events.is_empty(), "{events:?}");
    }
    let granted = database
        .events(
            &auditor,
            &Filter {
                stream: "principal/auditor",
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(granted.len(), 1, "other events stay readable");
    assert_eq!(database.setting_changes("local").unwrap().len(), 1);
}
