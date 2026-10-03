//! The router client against a misbehaving router: it never follows a
//! redirect, so a call's body cannot leave for another host, and it reads no
//! answer past its call's limit, so memory stays bounded, while a refusal
//! too long to read keeps its status.

use super::{
    super::{
        Error, Message, ModelPort, Role, Room, RouterClient, Speaker,
        body::{
            MAX_CATALOG_BODY_BYTES, MAX_CHAT_BODY_BYTES, MAX_ERROR_BODY_BYTES,
            MAX_PROPS_BODY_BYTES, embeddings_limit, ranking_limit, render_limit, tokens_limit,
        },
    },
    fixture::{BUILD, TEMPLATE, card, chat_request},
    stub::{Reply, StubRouter, answer},
};
use serde_json::json;
use std::{iter, time::Duration};
use tokio::time;

/// The message the chat and render calls send.
const QUESTION: &str = "Use the evidence.";

/// The text the tokenize call sends.
const TEXT: &str = "The whale sings.";

/// A body far past every limit here, which the stub writes in chunks.
const FLOOD: usize = 16 * 1_048_576;

/// The calls of the router client, each reading its own endpoint.
#[derive(Debug, Clone, Copy)]
enum Call {
    /// `GET /v1/models`.
    Catalog,
    /// `GET /models/embed/props`.
    Props,
    /// `POST /models/embed/v1/embeddings` of two inputs.
    Embed,
    /// `POST /models/rerank/v1/rerank` of three documents.
    Rerank,
    /// `POST /models/embed/tokenize` of [`TEXT`].
    Tokenize,
    /// `POST /models/answer/v1/chat/completions`.
    Chat,
    /// `POST /models/answer/apply-template`.
    Render,
}

/// Every call.
const CALLS: [Call; 7] = [
    Call::Catalog,
    Call::Props,
    Call::Embed,
    Call::Rerank,
    Call::Tokenize,
    Call::Chat,
    Call::Render,
];

impl Call {
    /// The path the call reads, and the `/props` path its card's check reads
    /// first, when it has one.
    const fn paths(self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Catalog => ("/v1/models", None),
            Self::Props => ("/models/embed/props", None),
            Self::Embed => ("/models/embed/v1/embeddings", Some("/models/embed/props")),
            Self::Rerank => ("/models/rerank/v1/rerank", Some("/models/rerank/props")),
            Self::Tokenize => ("/models/embed/tokenize", Some("/models/embed/props")),
            Self::Chat => (
                "/models/answer/v1/chat/completions",
                Some("/models/answer/props"),
            ),
            Self::Render => (
                "/models/answer/apply-template",
                Some("/models/answer/props"),
            ),
        }
    }

    /// The most bytes a successful answer to the call may have: the
    /// embedder's card records 3 dimensions.
    const fn limit(self) -> usize {
        match self {
            Self::Catalog => MAX_CATALOG_BODY_BYTES,
            Self::Props => MAX_PROPS_BODY_BYTES,
            Self::Embed => embeddings_limit(2, 3),
            Self::Rerank => ranking_limit(3),
            Self::Tokenize => tokens_limit(TEXT.len()),
            Self::Chat => MAX_CHAT_BODY_BYTES,
            Self::Render => render_limit(QUESTION.len()),
        }
    }

    /// Makes the call against a stub that answers its path with `reply`,
    /// and its `/props` check as the cards here record.
    async fn against(self, reply: Reply) -> Result<(), Error> {
        let (path, check) = self.paths();
        let props = answer(
            200,
            &json!({"build_info": BUILD, "chat_template": TEMPLATE}),
        );
        let mut replies = vec![(path, reply)];
        replies.extend(check.map(|check| (check, props)));
        let stub = StubRouter::serve(replies);
        self.on(&RouterClient::new(stub.base()).unwrap()).await
    }

    /// Makes the call with `client`.
    async fn on(self, client: &RouterClient) -> Result<(), Error> {
        let documents = ["Ships sail.", "Whales sing.", "Rain falls."].map(str::to_owned);
        let call = async {
            match self {
                Self::Catalog => client.catalog().await.map(drop),
                Self::Props => client.prepare(&card(Role::Embedder), Room::Free).await,
                Self::Embed => client
                    .embed(&card(Role::Embedder), Room::Free, &documents[..2])
                    .await
                    .map(drop),
                Self::Rerank => client
                    .rerank(&card(Role::Reranker), Room::Free, "whale", &documents)
                    .await
                    .map(drop),
                Self::Tokenize => client
                    .tokenize(&card(Role::Embedder), Room::Free, TEXT)
                    .await
                    .map(drop),
                Self::Chat | Self::Render => self.answer(client).await,
            }
        };
        time::timeout(Duration::from_secs(30), call)
            .await
            .expect("the call ends")
    }

    /// Makes a call of the answerer's, which sends [`QUESTION`], with
    /// `client`.
    async fn answer(self, client: &RouterClient) -> Result<(), Error> {
        let card = card(Role::Answerer);
        let request = chat_request(&[Message {
            speaker: Speaker::User,
            content: QUESTION.to_owned(),
        }]);
        if matches!(self, Self::Render) {
            client
                .render_chat(&card, Room::Free, &request)
                .await
                .map(drop)
        } else {
            client.chat(&card, Room::Free, &request).await.map(drop)
        }
    }
}

/// Asserts that `outcome` refuses an answer over a limit.
fn assert_over_limit(call: Call, outcome: Result<(), Error>) {
    match outcome {
        Err(Error::InvalidAnswer { reason }) if reason.contains("exceeds its limit of") => {}
        other => panic!("{call:?}: not refused over its limit: {other:?}"),
    }
}

/// Asserts that `outcome` is the router's refusal of `status`, whose body
/// was too long to read: its status stays, and its message names the limit
/// alone.
fn assert_refused_over_limit(call: Call, status: u16, outcome: Result<(), Error>) {
    let limit = MAX_ERROR_BODY_BYTES;
    match outcome {
        Err(Error::Refused {
            status: refused,
            code: None,
            message,
        }) if refused == status => assert_eq!(
            message,
            format!("the answer exceeds its limit of {limit} bytes"),
            "{call:?}"
        ),
        other => panic!("{call:?}: not a refusal of {status}: {other:?}"),
    }
}

#[tokio::test]
async fn a_redirect_is_a_typed_error_and_its_target_receives_nothing() {
    for status in [307, 308] {
        for call in CALLS {
            let target = StubRouter::serve(Vec::new());
            let (path, _) = call.paths();
            let location = target.base().join(path).unwrap().to_string();
            let outcome = call.against(Reply::Redirect(status, location)).await;
            match outcome {
                Err(Error::Redirected { status: named }) => assert_eq!(named, status),
                other => panic!("{call:?} after {status}: not refused: {other:?}"),
            }
            assert!(target.requests().is_empty(), "{call:?} after {status}");
        }
    }
}

#[test]
fn a_redirect_names_its_status() {
    let error = Error::Redirected { status: 308 };
    assert_eq!(
        error.to_string(),
        "the router answered with redirect 308, which this gateway never follows"
    );
}

#[tokio::test]
async fn a_declared_length_over_the_limit_is_refused_before_reading() {
    for call in CALLS {
        let outcome = call.against(Reply::Declared(200, call.limit() + 1)).await;
        assert_over_limit(call, outcome);
        let outcome = call
            .against(Reply::Declared(500, MAX_ERROR_BODY_BYTES + 1))
            .await;
        assert_refused_over_limit(call, 500, outcome);
    }
}

#[tokio::test]
async fn a_declared_length_at_the_limit_is_read() {
    // The stub sends no body, so reading one meets the connection's end.
    for call in CALLS {
        for reply in [
            Reply::Declared(200, call.limit()),
            Reply::Declared(500, MAX_ERROR_BODY_BYTES),
        ] {
            let outcome = call.against(reply).await;
            assert!(
                matches!(outcome, Err(Error::Transport(_))),
                "{call:?}: {outcome:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_chunked_body_stops_at_the_limit() {
    for call in CALLS {
        assert_over_limit(call, call.against(Reply::Chunked(200, FLOOD)).await);
        let outcome = call.against(Reply::Chunked(500, FLOOD)).await;
        assert_refused_over_limit(call, 500, outcome);
    }
}

#[tokio::test]
async fn a_chunked_body_one_byte_over_the_limit_is_refused() {
    for call in CALLS {
        let outcome = call.against(Reply::Chunked(200, call.limit() + 1)).await;
        assert_over_limit(call, outcome);
        let outcome = call
            .against(Reply::Chunked(503, MAX_ERROR_BODY_BYTES + 1))
            .await;
        assert_refused_over_limit(call, 503, outcome);
    }
}

#[tokio::test]
async fn a_body_over_the_limit_is_refused_before_its_stream_ends() {
    // The stub never ends these bodies: only a refusal at the byte over the
    // limit ends the call before the deadline.
    let deadline = Duration::from_secs(5);
    for call in CALLS {
        let held = call.against(Reply::Held(200, call.limit() + 1));
        let outcome = time::timeout(deadline, held).await;
        assert_over_limit(call, outcome.expect("refused before the stream ends"));
        let held = call.against(Reply::Held(503, MAX_ERROR_BODY_BYTES + 1));
        let outcome = time::timeout(deadline, held).await;
        let outcome = outcome.expect("refused before the stream ends");
        assert_refused_over_limit(call, 503, outcome);
    }
}

#[tokio::test]
async fn the_densest_measured_tokenization_fits_its_limit() {
    // BGE-M3 normalizes U+FDFA to a phrase of 4 tokens: pinned llama.cpp
    // 77f132c tokenized 10,000 of them, 30,000 bytes, into 40,002 tokens and
    // a compact answer of 210,016 bytes. These IDs give the same lengths.
    let text = "\u{FDFA}".repeat(10_000);
    let words = ["1000", "2000", "3000", "40000"];
    let ids: Vec<&str> = iter::once("0")
        .chain(iter::repeat_n(words, 10_000).flatten())
        .chain(iter::once("2"))
        .collect();
    let body = format!("{{\"tokens\":[{}]}}", ids.join(","));
    assert_eq!(
        (text.len(), ids.len(), body.len()),
        (30_000, 40_002, 210_016)
    );
    let props = json!({"build_info": BUILD, "chat_template": TEMPLATE});
    let stub = StubRouter::serve(vec![
        ("/models/embed/props", answer(200, &props)),
        ("/models/embed/tokenize", Reply::Answer(200, body)),
    ]);
    let client = RouterClient::new(stub.base()).unwrap();
    let tokens = client
        .tokenize(&card(Role::Embedder), Room::Free, &text)
        .await
        .unwrap();
    assert_eq!(tokens.len(), 40_002);
}

#[tokio::test]
async fn a_chunked_refusal_at_the_limit_is_read() {
    let outcome = Call::Tokenize
        .against(Reply::Chunked(503, MAX_ERROR_BODY_BYTES))
        .await;
    match outcome {
        Err(Error::Refused {
            status, message, ..
        }) => {
            assert_eq!(status, 503);
            assert_eq!(message.len(), MAX_ERROR_BODY_BYTES);
        }
        other => panic!("not a refusal: {other:?}"),
    }
}

#[test]
fn each_limit_follows_its_arithmetic() {
    assert_eq!(MAX_CHAT_BODY_BYTES, 262_144);
    assert_eq!(MAX_PROPS_BODY_BYTES, 1_048_576);
    assert_eq!(MAX_CATALOG_BODY_BYTES, 1_048_576);
    assert_eq!(MAX_ERROR_BODY_BYTES, 65_536);
    // (16 bytes + 16 added tokens) × 11 bytes + 64 KiB.
    assert_eq!(tokens_limit(16), 32 * 11 + 65_536);
    assert_eq!(tokens_limit(0), 16 * 11 + 65_536);
    assert_eq!(tokens_limit(30_000), 395_712);
    // 2 inputs × (1024 dimensions × 32 bytes + 256 bytes) + 64 KiB.
    assert_eq!(embeddings_limit(2, 1024), 2 * (1024 * 32 + 256) + 65_536);
    assert_eq!(embeddings_limit(1, 3), 3 * 32 + 256 + 65_536);
    // 3 documents × 256 bytes + 64 KiB.
    assert_eq!(ranking_limit(3), 3 * 256 + 65_536);
    assert_eq!(ranking_limit(0), 65_536);
    assert_eq!(tokens_limit(usize::MAX), usize::MAX);
    assert_eq!(embeddings_limit(usize::MAX, 2), usize::MAX);
    assert_eq!(ranking_limit(usize::MAX), usize::MAX);
}

#[test]
fn the_render_limit_follows_its_arithmetic() {
    // 17 message bytes × 6 escaped bytes + 1 MiB of template framing.
    assert_eq!(render_limit(17), 17 * 6 + 1_048_576);
    assert_eq!(render_limit(0), 1_048_576);
    assert_eq!(render_limit(usize::MAX), usize::MAX);
}
