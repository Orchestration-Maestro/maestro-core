//! Registration input, grant, and router-failure contract tests.

use super::{
    model_cli::{card_file, register_command},
    support::Home,
};
use serde_json::json;
use std::{fs, net::TcpListener};

#[test]
fn unknown_model_role_is_refused() {
    let home = Home::new();
    home.add_synthetic();
    let result = home.run(&[
        "model",
        "list",
        "--collection",
        "synthetic",
        "--role",
        "unknown",
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("unknown model role"), "{result:?}");
}

#[test]
fn model_register_refuses_a_collection_outside_the_principals_grants() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, false);
    home.configure("[access]\nread = ['workspace/other']\n");

    let refused = register_command(&home, &files, false);

    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("synthetic"), "{refused:?}");
}

#[test]
fn model_register_rejects_a_wrong_schema() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, false);
    let mut card: serde_json::Value =
        serde_json::from_slice(&fs::read(&files.card).unwrap()).unwrap();
    card["schema"] = json!("maestro-model-card/99");
    fs::write(&files.card, serde_json::to_vec(&card).unwrap()).unwrap();

    let refused = register_command(&home, &files, false);

    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(
        refused.stderr.contains("maestro-model-card/2"),
        "{refused:?}"
    );
}

#[test]
fn model_register_refuses_unknown_card_fields() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, false);
    let mut card: serde_json::Value =
        serde_json::from_slice(&fs::read(&files.card).unwrap()).unwrap();
    card["surprise"] = json!(true);
    fs::write(&files.card, serde_json::to_vec(&card).unwrap()).unwrap();

    let refused = register_command(&home, &files, false);

    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("unknown field"), "{refused:?}");
}

#[test]
fn router_failure_is_recorded_and_exits_one() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, true);
    let registered = register_command(&home, &files, true);
    let digest = registered.json()["digest"].as_str().unwrap().to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let mut command = home.command(&[
        "--json",
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        &digest,
    ]);
    command.env("MAESTRO_ROUTER_URL", format!("http://{address}"));
    let checked = super::support::Running::of(command).finish();

    assert_eq!(checked.code, Some(1), "{checked:?}");
    assert_eq!(checked.json()["disposition"], "failed");
    assert_eq!(checked.json()["eligible"], false);
    assert_eq!(checked.json()["report_digest"].as_str().unwrap().len(), 64);
    let listed = home.run(&[
        "--json",
        "model",
        "list",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
    ]);
    assert_eq!(listed.json()["evaluations"][0]["disposition"], "failed");
}
