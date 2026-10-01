//! CLI paths must not deliver content admitted by delayed stale reconciliation.

use crate::{
    cli::{ask, output::Output, retrieve},
    failure::Failure,
    kernel::Kernel,
    knowledge::{
        GetRequest, SearchRequest,
        operations::{KnowledgeError, search_with, tests::Scratch},
    },
    settings::KnowledgeSettings,
};
use maestro_kernel::gateway::{RouterClient, Url};
use maestro_knowledge::{
    answer::{AskBudget, AskRequest, DEFAULT_MODEL},
    index::Qdrant,
};
use serde_json::json;
use std::process::ExitCode;

#[test]
fn get_refuses_a_stale_grant_reapplied_after_revocation() {
    let scratch = Scratch::for_stale_grants();
    let request = GetRequest::from_cli(Some("chunk".to_owned()), None, None, None)
        .expect("valid exact retrieval");
    let result = retrieve::get_exact(Output::new(false), &request, || {
        scratch.stale_grant_kernel()
    });
    let Err(Failure::Refused(message)) = result else {
        panic!("stale-grant exact retrieval must refuse, not deliver source text");
    };
    assert_eq!(
        message,
        "permissions changed during the request; no result was delivered"
    );
}

#[test]
fn ask_refuses_a_stale_grant_reapplied_after_revocation() {
    let scratch = Scratch::for_stale_grants();
    assert_ask_refused(|| scratch.stale_grant_kernel());
}

#[test]
fn ask_refuses_a_stale_writer_at_the_final_snapshot() {
    let scratch = Scratch::for_stale_grants();
    assert_ask_refused(|| scratch.kernel_with_pending_writer());
}

fn assert_ask_refused(open: impl FnOnce() -> Result<Kernel, Failure>) {
    let request = AskRequest {
        collection: "collection".to_owned(),
        question: "question".to_owned(),
        model: DEFAULT_MODEL.to_owned(),
        version: None,
        budget: AskBudget::default(),
    };
    let result = ask::run(
        Output::new(false),
        &request,
        false,
        &KnowledgeSettings::default(),
        open,
    );
    assert_eq!(result.expect("safe CLI refusal"), ExitCode::from(2));
}

#[tokio::test]
async fn search_refuses_a_stale_grant_reapplied_after_revocation() {
    let scratch = Scratch::for_stale_grants();
    let request = SearchRequest::parse(json!({
        "collection": "collection", "query": "question", "deadline_ms": 30000,
    }))
    .expect("valid search request");
    let port = RouterClient::new(Url::parse("http://127.0.0.1:1").expect("closed router URL"))
        .expect("router client");
    let qdrant = Qdrant::new("http://127.0.0.1:1").expect("closed projection client");
    // CLI search calls this same operation; it must refuse before handing
    // any evidence to the CLI's formatter.
    let result = search_with(
        scratch.stale_grant_kernel().expect("replay stale opener"),
        &request,
        &KnowledgeSettings::default(),
        &port,
        &qdrant,
    )
    .await;
    assert!(matches!(
        result,
        Err(KnowledgeError::Refused {
            code: "access_changed",
            ..
        })
    ));
}
