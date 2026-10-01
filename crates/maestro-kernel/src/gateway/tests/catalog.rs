//! The router's catalog, as `maestro doctor` reads it: `GET /v1/models`,
//! which starts no model, asked in no room.

use super::{
    super::{Error, RouterClient, RouterEntry},
    stub::{Reply, StubRouter, answer, any_room},
};
use serde_json::{Value, json};

/// A client of a stub that answers `/v1/models` with `reply`.
fn serve(reply: Reply) -> (StubRouter, RouterClient) {
    let stub = StubRouter::serve(vec![("/v1/models", reply)]);
    let client = RouterClient::new(stub.base()).unwrap();
    (stub, client)
}

#[tokio::test]
async fn the_catalog_lists_the_routers_entries_and_starts_nothing() {
    let (stub, client) = serve(answer(
        200,
        &json!({"object": "list", "data": [
            {"id": "embed", "object": "model", "owned_by": "model-router"},
            {"id": "rerank", "object": "model", "owned_by": "model-router"}
        ]}),
    ));
    assert_eq!(
        client.catalog().await.unwrap(),
        [
            RouterEntry::parse("embed").unwrap(),
            RouterEntry::parse("rerank").unwrap()
        ]
    );
    assert_eq!(
        stub.requests(),
        [any_room("GET", "/v1/models", Value::Null)],
        "one listing, which asks for no room"
    );
}

#[tokio::test]
async fn a_catalog_the_router_refuses_keeps_its_refusal() {
    let (_stub, client) = serve(answer(
        503,
        &json!({"error": {"code": "shutting_down", "message": "the router is stopping",
            "type": "server_error"}}),
    ));
    let refusal = client.catalog().await.unwrap_err();
    assert!(
        matches!(
            &refusal,
            Error::Refused { status: 503, code: Some(code), message }
                if code == "shutting_down" && message == "the router is stopping"
        ),
        "{refusal:?}"
    );
}

#[tokio::test]
async fn a_catalog_entry_that_is_no_entry_name_is_an_invalid_answer() {
    let (_stub, client) = serve(answer(200, &json!({"data": [{"id": "../embed"}]})));
    let refusal = client.catalog().await.unwrap_err();
    assert!(
        matches!(&refusal, Error::InvalidAnswer { reason } if reason.contains("../embed")),
        "{refusal:?}"
    );
    let (_stub, client) = serve(answer(200, &json!({"models": []})));
    assert!(
        matches!(client.catalog().await, Err(Error::InvalidAnswer { .. })),
        "a listing without its data"
    );
}

#[tokio::test]
async fn a_router_that_hangs_up_is_a_transport_error() {
    let (_stub, client) = serve(Reply::HangUp);
    assert!(matches!(client.catalog().await, Err(Error::Transport(_))));
}
