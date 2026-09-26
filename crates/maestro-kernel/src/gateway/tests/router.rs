//! Tests of the router client against a stub router: every call is bound to
//! its card and to the room it names, a card the model's server does not
//! match is refused before any call, and each refusal keeps its meaning.

use super::{
    super::{CardFields, Error, Message, ModelPort, Role, Room, RouterClient, Speaker},
    fixture::{BUILD, TEMPLATE, TEMPLATE_DIGEST, card, card_of, fields},
    stub::{Reply, StubRouter, answer, any_room, free},
};
use crate::artifact::Digest;
use serde_json::{Value, json};
use std::error::Error as _;

/// What llama.cpp's server answers `GET /props` with, in part, for a server
/// of `build` with `template`.
fn props(build: &str, template: &str) -> Reply {
    answer(
        200,
        &json!({
            "default_generation_settings": {"n_ctx": 8192, "params": {"temperature": 0.8}},
            "total_slots": 1,
            "model_path": "models/model.gguf",
            "chat_template": template,
            "chat_template_caps": {},
            "modalities": {"vision": false},
            "build_info": build,
            "is_sleeping": false
        }),
    )
}

/// A stub whose model `entry` reports what the cards here record and answers
/// `path` under it with `reply`, and a client of that stub.
fn serve(entry: &str, path: &str, reply: Reply) -> (StubRouter, RouterClient) {
    let check = format!("/models/{entry}/props");
    let call = format!("/models/{entry}/{path}");
    let stub = StubRouter::serve(vec![
        (check.as_str(), props(BUILD, TEMPLATE)),
        (call.as_str(), reply),
    ]);
    let client = RouterClient::new(stub.base()).unwrap();
    (stub, client)
}

/// A router refusal in the envelope the router answers with.
fn refusal(status: u16, code: &str, message: &str) -> Reply {
    let kind = if status < 500 {
        "invalid_request_error"
    } else {
        "server_error"
    };
    answer(
        status,
        &json!({"error": {"code": code, "message": message, "type": kind}}),
    )
}

/// Two inputs to embed.
fn inputs() -> Vec<String> {
    vec!["The whale sings.".to_owned(), "A ship sails.".to_owned()]
}

/// An answer to embedding [`inputs`] with 3 dimensions, out of order as an
/// index allows.
fn embeddings() -> Reply {
    answer(
        200,
        &json!({
            "model": "embed",
            "object": "list",
            "usage": {"prompt_tokens": 9, "total_tokens": 9},
            "data": [
                {"embedding": [0.5, -0.25, 1.0], "index": 1, "object": "embedding"},
                {"embedding": [0.125, 0.0, -1.0], "index": 0, "object": "embedding"}
            ]
        }),
    )
}

/// An answer to reranking three documents, ordered by score as llama.cpp's
/// server orders it.
fn ranking() -> Reply {
    answer(
        200,
        &json!({
            "model": "rerank",
            "object": "list",
            "usage": {"prompt_tokens": 30, "total_tokens": 30},
            "results": [
                {"index": 2, "relevance_score": 3.5},
                {"index": 0, "relevance_score": 0.75},
                {"index": 1, "relevance_score": -2.0}
            ]
        }),
    )
}

/// A chat completion replying `content`.
fn completion(content: &str) -> Reply {
    answer(
        200,
        &json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "created": 1_758_844_800,
            "model": "answer",
            "choices": [{"index": 0, "finish_reason": "stop",
                "message": {"role": "assistant", "content": content}}],
            "usage": {"completion_tokens": 4, "prompt_tokens": 20, "total_tokens": 24}
        }),
    )
}

/// The error embedding [`inputs`] gives when the router answers `reply`.
async fn embedding_error(reply: Reply) -> Error {
    let (_stub, client) = serve("embed", "v1/embeddings", reply);
    client
        .embed(&card(Role::Embedder), Room::Free, &inputs())
        .await
        .unwrap_err()
}

/// What a card mismatch names: the card, the property, what the card records
/// and what the server reports. Any other outcome fails the test.
fn mismatch(outcome: Result<String, Error>) -> (Digest, &'static str, String, String) {
    match outcome {
        Err(Error::CardMismatch {
            card,
            property,
            recorded,
            reported,
        }) => (card, property, recorded, reported),
        other => panic!("not a card mismatch: {other:?}"),
    }
}

/// A user's message.
fn user(content: &str) -> Message {
    Message {
        speaker: Speaker::User,
        content: content.to_owned(),
    }
}

#[tokio::test]
async fn every_call_is_bound_to_its_card_and_to_its_room() {
    let tokens = answer(200, &json!({"tokens": [0, 35378, 8999, 2]}));
    let stub = StubRouter::serve(vec![
        ("/models/embed/props", props(BUILD, TEMPLATE)),
        ("/models/embed/v1/embeddings", embeddings()),
        ("/models/embed/tokenize", tokens),
        ("/models/rerank/props", props(BUILD, TEMPLATE)),
        ("/models/rerank/v1/rerank", ranking()),
        ("/models/answer/props", props(BUILD, TEMPLATE)),
        ("/models/answer/v1/chat/completions", completion("They do.")),
    ]);
    let client = RouterClient::new(stub.base()).unwrap();
    let embedder = card(Role::Embedder);
    // Tokenization comes first, so that the embedder's check is its own.
    let ids = client.tokenize(&embedder, Room::Free, "Hello world").await;
    assert_eq!(ids.unwrap(), [0, 35378, 8999, 2]);
    let vectors = client.embed(&embedder, Room::Free, &inputs()).await;
    assert_eq!(
        vectors.unwrap(),
        [[0.125, 0.0, -1.0], [0.5, -0.25, 1.0]],
        "by index"
    );
    let documents = ["Ships sail.", "Whales sing.", "Blue whales sing."].map(str::to_owned);
    let reranker = card(Role::Reranker);
    let scores = client
        .rerank(&reranker, Room::Free, "whale song", &documents)
        .await;
    assert_eq!(scores.unwrap(), [0.75, -2.0, 3.5], "by index");
    let instructions = Message {
        speaker: Speaker::System,
        content: "Answer from the evidence.".to_owned(),
    };
    let messages = [instructions, user("Whales sing."), user("Do whales sing?")];
    let reply = client
        .chat(&card(Role::Answerer), Room::Any, &messages)
        .await;
    assert_eq!(reply.unwrap(), "They do.");
    let counted = json!({"content": "Hello world", "add_special": true, "parse_special": true});
    let asked = json!({"messages": [
        {"role": "system", "content": "Answer from the evidence."},
        {"role": "user", "content": "Whales sing."},
        {"role": "user", "content": "Do whales sing?"}
    ]});
    assert_eq!(
        stub.requests(),
        [
            free("GET", "/models/embed/props", Value::Null),
            free("POST", "/models/embed/tokenize", counted),
            free(
                "POST",
                "/models/embed/v1/embeddings",
                json!({"input": inputs()})
            ),
            free("GET", "/models/rerank/props", Value::Null),
            free(
                "POST",
                "/models/rerank/v1/rerank",
                json!({"query": "whale song", "documents": documents})
            ),
            any_room("GET", "/models/answer/props", Value::Null),
            any_room("POST", "/models/answer/v1/chat/completions", asked),
        ]
    );
}

#[tokio::test]
async fn a_card_the_server_does_not_match_is_refused_before_any_call() {
    let bare = answer(200, &json!({"build_info": BUILD}));
    let cases = [
        (
            props("b6501-0c1d2e3f", TEMPLATE),
            "build_info",
            BUILD,
            "b6501-0c1d2e3f",
        ),
        (
            props(BUILD, "{{ messages }}"),
            "chat_template",
            TEMPLATE_DIGEST,
            "f24189f08c85a1eb19a737306c3a13e85462ee33d964f28108d64a2485fa2171",
        ),
        (bare, "chat_template", TEMPLATE_DIGEST, "none"),
    ];
    let question = [user("Do whales sing?")];
    for (props, property, recorded, reported) in cases {
        let stub = StubRouter::serve(vec![("/models/answer/props", props)]);
        let client = RouterClient::new(stub.base()).unwrap();
        let card = card(Role::Answerer);
        let refused = (
            card.digest().clone(),
            property,
            recorded.to_owned(),
            reported.to_owned(),
        );
        // A refused card is never taken as checked: its next call checks again.
        let first = mismatch(client.chat(&card, Room::Any, &question).await);
        let second = mismatch(client.chat(&card, Room::Any, &question).await);
        assert_eq!([first, second], [refused.clone(), refused]);
        let checked = any_room("GET", "/models/answer/props", Value::Null);
        let requests = [checked.clone(), checked];
        assert_eq!(stub.requests(), requests, "no call follows a check");
    }
}

#[tokio::test]
async fn a_card_is_checked_once_per_card_and_gateway() {
    let (stub, first) = serve("embed", "v1/embeddings", embeddings());
    let card = card(Role::Embedder);
    let other = card_of(&CardFields {
        suite_results: Vec::new(),
        ..fields(Role::Embedder)
    });
    for card in [&card, &card, &other] {
        first.embed(card, Room::Free, &inputs()).await.unwrap();
    }
    let second = RouterClient::new(stub.base()).unwrap();
    second.embed(&card, Room::Free, &inputs()).await.unwrap();
    let paths: Vec<String> = stub
        .requests()
        .into_iter()
        .map(|request| request.path)
        .collect();
    let (check, call) = ("/models/embed/props", "/models/embed/v1/embeddings");
    assert_eq!(paths, [check, call, call, check, call, check, call]);
}

#[tokio::test]
async fn insufficient_room_makes_the_model_unavailable_with_the_routers_reason() {
    let reason = "'embed' was asked for with 'X-Model-Router-Room: free', and there is \
                  no free room for it: loading it would unload gemma3; nothing was unloaded";
    let full = refusal(503, "insufficient_room", reason);
    // The check loads the model, so the router may refuse it as well as the
    // call. A refused check is made again at the next call; a passed one is
    // not, even when its call is refused.
    let at_check = StubRouter::serve(vec![("/models/embed/props", full.clone())]);
    let at_call = StubRouter::serve(vec![
        ("/models/embed/props", props(BUILD, TEMPLATE)),
        ("/models/embed/v1/embeddings", full),
    ]);
    let check = free("GET", "/models/embed/props", Value::Null);
    let call = free(
        "POST",
        "/models/embed/v1/embeddings",
        json!({"input": inputs()}),
    );
    let cases = [
        (at_check, vec![check.clone(), check.clone()]),
        (at_call, vec![check, call.clone(), call]),
    ];
    for (stub, requests) in cases {
        let client = RouterClient::new(stub.base()).unwrap();
        let card = card(Role::Embedder);
        for _ in 0..2 {
            let error = client
                .embed(&card, Room::Free, &inputs())
                .await
                .unwrap_err();
            assert!(
                matches!(&error, Error::Unavailable { reason: given } if given == reason),
                "{error}"
            );
        }
        assert_eq!(stub.requests(), requests);
    }
}

#[tokio::test]
async fn an_unknown_room_is_a_typed_error() {
    let message = "'X-Model-Router-Room' may only be 'free', not 'spare'";
    let error = embedding_error(refusal(400, "unknown_room", message)).await;
    assert!(
        matches!(&error, Error::UnknownRoom { message: given } if given == message),
        "{error}"
    );
}

#[tokio::test]
async fn other_refusals_keep_their_status_code_and_message() {
    let missing = "no model called 'embed'; this catalog carries: gemma3";
    let held = "the room is held by a request that reached it first";
    let too_long = "input is too large to process. increase the physical batch size";
    let cases = [
        (
            refusal(404, "model_not_found", missing),
            404,
            Some("model_not_found"),
            missing,
        ),
        (
            refusal(503, "room_contended", held),
            503,
            Some("room_contended"),
            held,
        ),
        (
            refusal(400, "insufficient_room", held),
            400,
            Some("insufficient_room"),
            held,
        ),
        (
            refusal(503, "unknown_room", held),
            503,
            Some("unknown_room"),
            held,
        ),
        (
            answer(400, &json!({"error": {"code": 400, "message": too_long}})),
            400,
            None,
            too_long,
        ),
        (
            Reply::Answer(502, "Bad Gateway".to_owned()),
            502,
            None,
            "Bad Gateway",
        ),
    ];
    for (reply, status, code, message) in cases {
        let error = embedding_error(reply).await;
        let Error::Refused {
            status: found,
            code: named,
            message: said,
        } = &error
        else {
            panic!("not a plain refusal: {error}");
        };
        assert_eq!(
            (*found, named.as_deref(), said.as_str()),
            (status, code, message)
        );
    }
}

#[tokio::test]
async fn a_card_for_another_role_is_refused_before_any_request() {
    let stub = StubRouter::serve(Vec::new());
    let client = RouterClient::new(stub.base()).unwrap();
    let (embedder, reranker) = (card(Role::Embedder), card(Role::Reranker));
    let question = [user("Do whales sing?")];
    let refusals = [
        (
            client
                .embed(&reranker, Room::Free, &inputs())
                .await
                .unwrap_err(),
            &reranker,
            Role::Embedder,
        ),
        (
            client
                .rerank(&embedder, Room::Free, "whale", &inputs())
                .await
                .unwrap_err(),
            &embedder,
            Role::Reranker,
        ),
        (
            client
                .chat(&reranker, Room::Any, &question)
                .await
                .unwrap_err(),
            &reranker,
            Role::Answerer,
        ),
    ];
    for (error, card, needed) in refusals {
        let role = card.fields().role;
        assert!(
            matches!(&error, Error::WrongRole { card: refused, role: found, needed: wanted }
                if refused == card.digest() && *found == role && *wanted == needed),
            "{error}"
        );
    }
    assert_eq!(stub.requests(), [], "no request is made for the wrong card");
}

#[tokio::test]
async fn no_input_and_no_document_need_no_request() {
    let stub = StubRouter::serve(Vec::new());
    let client = RouterClient::new(stub.base()).unwrap();
    let vectors = client.embed(&card(Role::Embedder), Room::Free, &[]).await;
    assert!(vectors.unwrap().is_empty());
    let reranker = card(Role::Reranker);
    let scores = client.rerank(&reranker, Room::Free, "whale", &[]).await;
    assert!(scores.unwrap().is_empty());
    assert_eq!(stub.requests(), []);
}

#[tokio::test]
async fn an_answer_of_another_shape_is_refused() {
    let vector = |index: usize, values: &[f32]| json!({"embedding": values, "index": index});
    let shapes = [
        json!({"data": [vector(0, &[1.0, 2.0]), vector(1, &[1.0, 2.0])]}),
        json!({"data": [vector(0, &[1.0, 2.0, 3.0])]}),
        json!({"data": [vector(0, &[1.0, 2.0, 3.0]), vector(0, &[1.0, 2.0, 3.0])]}),
        json!({"data": [vector(0, &[1.0, 2.0, 3.0]), vector(2, &[1.0, 2.0, 3.0])]}),
        json!({"vectors": []}),
    ];
    for shape in shapes {
        let error = embedding_error(answer(200, &shape)).await;
        assert!(
            matches!(error, Error::InvalidAnswer { .. }),
            "{shape}: {error}"
        );
    }
    let error = embedding_error(Reply::Answer(200, "not json".to_owned())).await;
    assert!(matches!(error, Error::InvalidAnswer { .. }), "{error}");
}

#[tokio::test]
async fn a_chat_without_a_reply_is_refused() {
    let empty = json!({"choices": []});
    let silent = json!({"choices": [{"message": {"role": "assistant", "content": null}}]});
    for completion in [empty, silent] {
        let reply = answer(200, &completion);
        let (_stub, client) = serve("answer", "v1/chat/completions", reply);
        let card = card(Role::Answerer);
        let error = client
            .chat(&card, Room::Any, &[user("Do whales sing?")])
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::InvalidAnswer { .. }),
            "{completion}: {error}"
        );
    }
}

#[tokio::test]
async fn a_ranking_that_misses_a_document_is_refused() {
    let partial = json!({"results": [{"index": 1, "relevance_score": 0.5}]});
    let (_stub, client) = serve("rerank", "v1/rerank", answer(200, &partial));
    let documents = ["Ships sail.", "Whales sing."].map(str::to_owned);
    let error = client
        .rerank(&card(Role::Reranker), Room::Free, "whale", &documents)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidAnswer { .. }), "{error}");
}

#[tokio::test]
async fn a_router_that_hangs_up_is_a_transport_error() {
    let stub = StubRouter::serve(vec![("/models/embed/props", Reply::HangUp)]);
    let client = RouterClient::new(stub.base()).unwrap();
    let error = client
        .embed(&card(Role::Embedder), Room::Free, &inputs())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Transport(_)), "{error}");
    assert!(error.to_string().contains("router"), "{error}");
    assert!(error.source().is_some());
}
