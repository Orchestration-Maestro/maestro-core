//! Selection reads only the evaluations of the exact card it selects.

use super::{
    model_cli::{CardFiles, StubRouter, card_file, register_command},
    support::{Home, Running},
};
use serde_json::json;
use std::fs;

#[test]
fn a_later_evaluation_of_another_card_of_the_role_leaves_selection_to_the_cards_own() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, true);
    let selected_card = registered_digest(&home, &files);
    check(&home, &selected_card, [0.9, 0.1]);
    let mut card: serde_json::Value =
        serde_json::from_slice(&fs::read(&files.card).unwrap()).unwrap();
    card["identity"]["weights"]["upstream_revision"] = json!("fedcba9876543210");
    fs::write(&files.card, serde_json::to_vec(&card).unwrap()).unwrap();
    let other_card = registered_digest(&home, &files);
    assert_ne!(other_card, selected_card);
    check(&home, &other_card, [0.1, 0.2]);

    let selected = home.run(&[
        "--json",
        "model",
        "select",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
        "--digest",
        &selected_card,
    ]);

    assert_eq!(selected.code, Some(0), "{selected:?}");
    assert_eq!(selected.json()["digest"], selected_card.as_str());
}

/// Registers the card in `files` and returns its digest.
fn registered_digest(home: &Home, files: &CardFiles) -> String {
    let registered = register_command(home, files, true);
    assert_eq!(registered.code, Some(0), "{registered:?}");
    registered.json()["digest"].as_str().unwrap().to_owned()
}

/// Records a real health-gate evaluation of `digest` from a router scoring the
/// positive and negative pair with `scores`.
fn check(home: &Home, digest: &str, scores: [f64; 2]) {
    let router = StubRouter::serve(scores);
    let mut command = home.command(&[
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        digest,
    ]);
    command.env("MAESTRO_ROUTER_URL", &router.url);
    let checked = Running::of(command).finish();
    assert_eq!(checked.code, Some(0), "{checked:?}");
}
