//! The services' checks: Qdrant answering as the pinned version, the model
//! router listing its catalog, and each role's model card; a failure names
//! the address it tried and the next action, which for Qdrant depends on
//! what setup would still do.

use super::{
    super::services::{DEFAULT_ROUTER, card_checks, qdrant_check, router_check, router_url},
    support::{detail, failure, nothing_at, serve},
};
use crate::cli::{
    failure::Failure,
    setup::{Readiness, Step},
};
use std::ffi::OsStr;

/// What Qdrant 1.19.1 answers `GET /` with.
const ROOT: &str = concat!(
    r#"{"title":"qdrant - vector search engine","version":"1.19.1","#,
    r#""commit":"6ab21cac18ebb6f4ae29102c7f8f5cc11affd5de"}"#
);

/// A readiness never asked for, as when Qdrant answers.
fn unasked() -> Result<Readiness, Failure> {
    panic!("asked what setup would do, though Qdrant answered")
}

#[test]
fn qdrant_passes_when_the_pinned_version_answers() {
    let address = serve(200, ROOT);
    let check = qdrant_check(&address, unasked);
    assert_eq!(check.name, "qdrant");
    assert_eq!(check.target, address);
    assert_eq!(detail(&check), "Qdrant 1.19.1 answers");
}

#[test]
fn another_version_of_qdrant_fails_and_setup_installs_the_pinned_one() {
    let address = serve(
        200,
        r#"{"title":"qdrant - vector search engine","version":"1.18.0"}"#,
    );
    let check = qdrant_check(&address, unasked);
    let (problem, next) = failure(&check);
    assert_eq!(problem, "Qdrant 1.18.0 answers, not the pinned 1.19.1");
    assert!(next.contains("maestro setup --yes"), "{next}");
}

#[test]
fn a_server_that_is_not_qdrant_fails_naming_what_answered() {
    for (status, body) in [
        (200, "<html>a web page</html>"),
        (404, "{}"),
        (500, r#"{"version":"1.19.1"}"#),
    ] {
        let address = serve(status, body);
        let check = qdrant_check(&address, unasked);
        let (problem, next) = failure(&check);
        assert!(problem.contains("not as Qdrant"), "{status}: {problem}");
        assert!(next.contains("port 6333"), "{next}");
    }
}

#[test]
fn qdrant_that_does_not_answer_fails_with_what_setup_would_do_next() {
    let address = nothing_at();
    let cases: [(Result<Readiness, Failure>, &str); 4] = [
        (
            Ok(Readiness::Steps(vec![Step::Install])),
            "maestro setup --yes",
        ),
        (
            Ok(Readiness::Steps(Vec::new())),
            "systemctl --user restart maestro-qdrant.service",
        ),
        (Ok(Readiness::ByHand), "by hand"),
        (
            Err(Failure::failed("systemctl is missing")),
            "maestro setup",
        ),
    ];
    for (readiness, expected) in cases {
        let check = qdrant_check(&address, || readiness);
        let (problem, next) = failure(&check);
        assert!(problem.starts_with("no answer"), "{problem}");
        assert!(
            next.contains(expected),
            "{expected} is missing from: {next}"
        );
    }
}

#[test]
fn the_router_passes_when_it_lists_its_catalog() {
    let address = serve(
        200,
        r#"{"object":"list","data":[{"id":"embed"},{"id":"rerank"},{"id":"qwen38"}]}"#,
    );
    let check = router_check(router_url(Some(OsStr::new(&address))));
    assert_eq!(check.name, "router");
    assert_eq!(check.target, format!("{address}/"));
    assert_eq!(detail(&check), "3 entries in its catalog");
}

#[test]
fn a_router_that_does_not_answer_fails_naming_the_address_it_tried() {
    let address = nothing_at();
    let check = router_check(router_url(Some(OsStr::new(&address))));
    assert_eq!(check.target, format!("{address}/"));
    let (problem, next) = failure(&check);
    assert!(problem.contains("does not answer"), "{problem}");
    assert!(
        next.contains("maestro-model-router")
            && next.contains(&format!("{address}/"))
            && next.contains("MAESTRO_ROUTER_URL"),
        "{next}"
    );
}

#[test]
fn the_router_is_at_its_default_address_unless_the_environment_names_another() {
    assert_eq!(DEFAULT_ROUTER, "http://127.0.0.1:8080");
    assert_eq!(router_url(None).unwrap().as_str(), "http://127.0.0.1:8080/");
    assert_eq!(
        router_url(Some(OsStr::new("http://10.0.0.7:9000")))
            .unwrap()
            .as_str(),
        "http://10.0.0.7:9000/"
    );
    let check = router_check(router_url(Some(OsStr::new("not a url"))));
    assert_eq!(check.target, "not a url");
    let (problem, next) = failure(&check);
    assert!(problem.contains("MAESTRO_ROUTER_URL"), "{problem}");
    assert!(next.contains("http://127.0.0.1:8080"), "{next}");
}

#[test]
fn every_role_fails_until_the_bake_off_records_its_card() {
    let checks = card_checks();
    let targets: Vec<&str> = checks.iter().map(|check| check.target.as_str()).collect();
    assert_eq!(targets, ["embedder", "reranker", "answerer"]);
    for check in &checks {
        assert_eq!(check.name, "model_card");
        let (problem, next) = failure(check);
        assert_eq!(
            problem,
            format!("no model card is recorded for the {}", check.target)
        );
        assert!(next.contains("bake-off") && next.contains("T030"), "{next}");
    }
}
