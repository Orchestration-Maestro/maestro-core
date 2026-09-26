//! The router client through a router tokenizer, against a stub router: from
//! a plain thread and from inside a runtime, whose `block_on` would panic,
//! each request in free room.

use super::{
    super::{RouterTokenizer, TokenizerError},
    stub::StubRouter,
    support::{MODEL_FILE, card, embedder},
};
use maestro_canonicalization::TokenCounter;
use maestro_kernel::gateway::{self, Role, RouterClient};

/// A tokenizer qualified through a router client of `stub`.
fn qualified(stub: &StubRouter) -> RouterTokenizer {
    RouterTokenizer::qualify(RouterClient::new(stub.base()).unwrap(), embedder()).unwrap()
}

#[test]
fn a_router_client_counts_from_a_plain_thread_in_free_room() {
    let stub = StubRouter::serve();
    let tokenizer = qualified(&stub);
    assert_eq!(
        tokenizer.token_ids("Hello world").unwrap(),
        [0, 35378, 8999, 2]
    );
    let requests = stub.requests();
    // The card's check, the 41 fixtures, then the count.
    assert_eq!(requests.len(), 43);
    assert_eq!(requests[0].0, "/models/embed/props");
    assert!(
        requests[1..]
            .iter()
            .all(|(path, _)| path == "/models/embed/tokenize")
    );
    assert!(
        requests
            .iter()
            .all(|(_, room)| room.as_deref() == Some("free"))
    );
}

#[tokio::test]
async fn a_router_client_counts_from_inside_a_runtime() {
    let stub = StubRouter::serve();
    let tokenizer = qualified(&stub);
    tokenizer.verify().unwrap();
    assert_eq!(tokenizer.token_ids("<s></s>").unwrap(), [0, 0, 2, 2]);
}

#[test]
fn a_card_recorded_under_another_router_build_is_refused_by_the_gateway() {
    let stub = StubRouter::serve();
    let client = RouterClient::new(stub.base()).unwrap();
    let rebuilt = card(Role::Embedder, MODEL_FILE, "b6600-9e8d7c6b");
    let refused = RouterTokenizer::qualify(client, rebuilt).unwrap_err();
    let TokenizerError::Port(gateway::Error::CardMismatch {
        property,
        recorded,
        reported,
        ..
    }) = refused
    else {
        panic!("{refused:?}");
    };
    assert_eq!(
        (property, recorded.as_str(), reported.as_str()),
        ("build_info", "b6600-9e8d7c6b", "b6500-3f2c9a1b")
    );
}
