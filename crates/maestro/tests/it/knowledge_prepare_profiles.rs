//! `knowledge prepare --chunk-profile` builds the chunk set of the chunking
//! profile it names, beside the default profile's.

use super::{
    knowledge_publish::{StubRouter, card},
    support::{Home, Running, stream},
};
use maestro_kernel::gateway::Role;
use serde_json::Value;
use std::net::TcpListener;

/// Runs `knowledge prepare` of `synthetic` with the embedder card of `home`
/// against `router`, with `profile` arguments, and returns its exit code, its
/// JSON document and its diagnostics.
fn prepare(home: &Home, router: &StubRouter, profile: &[&str]) -> (Option<i32>, Value, String) {
    let card = card(home, Role::Embedder);
    let mut arguments = vec![
        "knowledge",
        "prepare",
        "--collection",
        "synthetic",
        "--card",
        card.as_str(),
        "--json",
    ];
    arguments.extend(profile);
    let mut command = home.command(&arguments);
    command.env("MAESTRO_ROUTER_URL", router.url());
    let result = Running::of(command).finish();
    let document = serde_json::from_str(&result.stdout).unwrap_or(Value::Null);
    (result.code, document, result.stderr)
}

#[test]
fn a_new_collection_defaults_to_ideas_and_keeps_explicit_structural_sets_available() {
    let home = Home::new();
    home.add_synthetic();
    let router = StubRouter::serve();
    let (code, default, _) = prepare(&home, &router, &[]);
    assert_eq!(code, Some(0), "{default}");
    assert_eq!(
        default["outcome"]["chunk_profile"],
        "mapped-structural-chunks/3"
    );
    let named = ["--chunk-profile", "mapped-structural-chunks/2"];
    let (code, structural, _) = prepare(&home, &router, &named);
    assert_eq!(code, Some(0), "{structural}");
    assert_eq!(
        structural["outcome"]["chunk_profile"],
        "mapped-structural-chunks/2"
    );
    assert_ne!(
        structural["outcome"]["chunk_set"],
        default["outcome"]["chunk_set"]
    );
    let (code, again, _) = prepare(&home, &router, &[]);
    assert_eq!(code, Some(0), "{again}");
    assert_eq!(
        again["outcome"]["chunk_set"],
        default["outcome"]["chunk_set"]
    );
}

#[test]
fn an_unknown_chunking_profile_is_refused_before_a_job_naming_the_known_ones() {
    let home = Home::new();
    home.add_synthetic();
    let router = StubRouter::serve();
    let (code, document, stderr) = prepare(&home, &router, &["--chunk-profile", "fixed/1"]);
    assert_eq!(code, Some(2), "{stderr}");
    assert_eq!(document, Value::Null, "no job was submitted");
    assert!(
        stderr.contains(
            "no chunking profile is named fixed/1; the profiles are \
             mapped-structural-chunks/2, mapped-structural-chunks/3"
        ),
        "{stderr}"
    );
}

/// Runs `knowledge publish` of `synthetic` with the embedder card of `home`,
/// with `selection` arguments, against `router` and a Qdrant that refuses
/// connections, and returns its exit code and the chunk set its job was
/// submitted for, if one was.
fn publish(home: &Home, router: &StubRouter, selection: &[&str]) -> (Option<i32>, Option<String>) {
    let card = card(home, Role::Embedder);
    let mut arguments = vec![
        "knowledge",
        "publish",
        "--collection",
        "synthetic",
        "--card",
        card.as_str(),
        "--json",
    ];
    arguments.extend(selection);
    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut command = home.command(&arguments);
    command.env("MAESTRO_ROUTER_URL", router.url());
    command.env("MAESTRO_QDRANT_URL", format!("http://{closed}"));
    let result = Running::of(command).finish();
    let chunk_set = serde_json::from_str::<Value>(&result.stdout)
        .ok()
        .and_then(|document| {
            let job = document["job"].as_str()?.to_owned();
            let events = stream(&home.database(), &format!("job/{job}"));
            Some(
                events.first()?.data["inputs"]["chunk_set"]
                    .as_str()?
                    .to_owned(),
            )
        });
    (result.code, chunk_set)
}

#[test]
fn publish_without_a_chunk_set_takes_the_latest_complete_set_of_the_chunking_profile() {
    let home = Home::new();
    home.add_synthetic();
    let router = StubRouter::serve();
    let (_, ideas, _) = prepare(&home, &router, &[]);
    let named = ["--chunk-profile", "mapped-structural-chunks/2"];
    let (_, structural, _) = prepare(&home, &router, &named);
    let [ideas, structural] = [ideas, structural].map(|document| {
        document["outcome"]["chunk_set"]
            .as_str()
            .unwrap()
            .to_owned()
    });
    assert_eq!(publish(&home, &router, &[]).1, Some(ideas));
    assert_eq!(publish(&home, &router, &named).1, Some(structural));
    let both = [
        "--chunk-set",
        "chunk-set-any",
        "--chunk-profile",
        "mapped-structural-chunks/3",
    ];
    assert_eq!(publish(&home, &router, &both), (Some(2), None));
    let unknown = ["--chunk-profile", "fixed/1"];
    assert_eq!(publish(&home, &router, &unknown), (Some(2), None));
}
