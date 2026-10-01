//! Golden payload bytes and journal-only authority/revocation contracts.
use super::{ANSWERED, Answer, Confirmation, WorkspaceAnswer, records::STREAM};
use crate::{
    journal::{Filter, NewEvent},
    scope::{Config, LOCAL, WORKSPACE},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use std::{fs, path::PathBuf};

#[test]
fn workspace_payload_golden_bytes_do_not_depend_on_json_backend() {
    let change = WorkspaceAnswer {
        path: PathBuf::from("/synthetic/workspace"),
        answer: Answer::Approved {
            confirmation: Confirmation::ConfirmPath,
        },
    };
    assert_eq!(
        serde_json::to_vec(&change).unwrap(),
        concat!(
            r#"{"path":"/synthetic/workspace","answer":{"kind":"approved","#,
            r#""confirmation":"confirm_path"}}"#,
        )
        .as_bytes()
    );
    let decoded: WorkspaceAnswer =
        serde_json::from_slice(&serde_json::to_vec(&change).unwrap()).unwrap();
    assert_eq!(decoded, change);
    let preferences = WorkspaceAnswer {
        path: PathBuf::from("/synthetic/workspace"),
        answer: Answer::Preferences {
            digest: "sha256:synthetic".to_owned(),
            confirmation: Confirmation::Terminal,
            completed: true,
        },
    };
    assert_eq!(
        serde_json::to_vec(&preferences).unwrap(),
        concat!(
            r#"{"path":"/synthetic/workspace","answer":{"kind":"preferences","#,
            r#""digest":"sha256:synthetic","confirmation":"terminal","completed":true}}"#,
        )
        .as_bytes()
    );
}

#[test]
fn workspace_journal_replays_revocation_and_never_exposes_authority() {
    let scratch = scratch_directory().unwrap();
    let database = Database::open_in(&scratch.join("data")).unwrap();
    assert!(
        database
            .record_workspace_answer(&WorkspaceAnswer {
                path: PathBuf::from("relative"),
                answer: Answer::Approved {
                    confirmation: Confirmation::ConfirmPath
                },
            })
            .is_err()
    );
    let path = scratch.canonicalize().unwrap().join("project");
    let change = |answer| WorkspaceAnswer {
        path: path.clone(),
        answer,
    };
    let approved = database
        .record_workspace_answer(&change(Answer::Approved {
            confirmation: Confirmation::Terminal,
        }))
        .unwrap();
    assert_eq!(database.trusted_workspaces().unwrap(), vec![approved]);
    database
        .record_workspace_answer(&change(Answer::Removed))
        .unwrap();
    assert!(database.trusted_workspaces().unwrap().is_empty());
    database
        .record_workspace_answer(&change(Answer::Preferences {
            digest: "sha256:synthetic".into(),
            confirmation: Confirmation::ConfirmPath,
            completed: false,
        }))
        .unwrap();
    assert!(database.trusted_workspaces().unwrap().is_empty());
    database
        .apply_config(
            &"[access]\nread = ['workspace/default']\n"
                .parse::<Config>()
                .unwrap(),
        )
        .unwrap();
    let scopes = database.visible(LOCAL).unwrap();
    let filter = Filter {
        stream: STREAM,
        after: 0,
        r#type: None,
    };
    assert!(database.events(&scopes, &filter).unwrap().is_empty());
    let data = serde_json::json!({
        "path": path, "answer": {"kind":"approved", "confirmation":"confirm_path"}
    });
    assert!(
        database
            .record(&NewEvent {
                stream: STREAM,
                r#type: ANSWERED,
                subject: "forged",
                scope: WORKSPACE,
                data: &data
            })
            .is_err()
    );
    drop(database);
    assert!(
        Database::open_in(&scratch.join("data"))
            .unwrap()
            .trusted_workspaces()
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn workspace_read_only_open_creates_nothing_and_cannot_record() {
    let scratch = scratch_directory().unwrap();
    assert!(Database::open_read_only_in(&scratch).is_err());
    assert_eq!(fs::read_dir(&scratch).unwrap().count(), 0);
    drop(Database::open_in(&scratch).unwrap());
    let database = Database::open_read_only_in(&scratch).unwrap();
    assert!(database.trusted_workspaces().unwrap().is_empty());
    assert!(
        database
            .record_workspace_answer(&WorkspaceAnswer {
                path: scratch.canonicalize().unwrap(),
                answer: Answer::Declined,
            })
            .is_err()
    );
    drop(database);
    fs::remove_dir_all(scratch).unwrap();
}
