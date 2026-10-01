//! Delayed startup reconciliation must not deliver revoked content.

use super::{HANG_GUARD, KnowledgeServer, handshake_and_tool_call, serve_one_call, tool_error};
use crate::knowledge::operations::tests::Scratch;
use serde_json::{Value, json};

#[tokio::test]
async fn get_refuses_a_stale_grant_reapplied_after_revocation() {
    assert_stale_grant_refused("knowledge_get", json!({"chunk_id": "chunk"})).await;
}

#[tokio::test]
async fn search_refuses_a_stale_grant_reapplied_after_revocation() {
    assert_stale_grant_refused(
        "knowledge_search",
        json!({"collection": "collection", "query": "question", "deadline_ms": 30000}),
    )
    .await;
}

#[tokio::test]
async fn ask_refuses_a_stale_grant_reapplied_after_revocation() {
    assert_stale_grant_refused(
        "knowledge_ask",
        json!({"collection": "collection", "question": "question"}),
    )
    .await;
}

#[tokio::test]
async fn ask_refuses_a_stale_writer_at_the_final_snapshot() {
    let scratch = Scratch::for_stale_grants();
    let server = KnowledgeServer::with_kernel_opener(
        move || scratch.kernel_with_pending_writer(),
        HANG_GUARD,
    );
    let request = handshake_and_tool_call(
        2,
        "knowledge_ask",
        &json!({"collection": "collection", "question": "question"}),
    );
    let response = serve_one_call(server, request, 2).await;
    assert_eq!(tool_error(&response)["error"]["code"], "access_changed");
    assert!(!response.to_string().contains("exact source text"));
}

async fn assert_stale_grant_refused(tool: &str, arguments: Value) {
    let scratch = Scratch::for_stale_grants();
    let server =
        KnowledgeServer::with_kernel_opener(move || scratch.stale_grant_kernel(), HANG_GUARD);
    let response = serve_one_call(server, handshake_and_tool_call(2, tool, &arguments), 2).await;
    assert_eq!(tool_error(&response)["error"]["code"], "access_changed");
    assert!(!response.to_string().contains("exact source text"));
    assert!(!response.to_string().contains("chunk-set"));
}
