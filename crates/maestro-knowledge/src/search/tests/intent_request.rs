//! The expansion port: the one chat request the `HyDE` adapter sends, the
//! cards it refuses, and search's own deadline and guard around any adapter.

use super::rerank::card;
use crate::{
    answer::tests::reasoning_answerer,
    query::{Understood, understand},
    search::{
        hyde::{HydeCardError, HydeExpander},
        intent::{Expansion, ExpansionFailure, ExpansionFuture, QueryExpander},
        intent_routes::expand,
    },
};
use maestro_kernel::gateway::{
    ChatRequest, Error, ModelCard, ModelPort, Role, Room, Speaker, card_v2::ControlValue,
};
use std::{
    collections::BTreeMap,
    future::{self, Future},
    sync::Mutex,
    time::Duration,
};
use tokio::time::Instant;

/// Records each chat request and answers with one fixed reply.
#[derive(Default)]
struct RecordingPort {
    requests: Mutex<Vec<ChatRequest>>,
}

impl ModelPort for RecordingPort {
    fn embed(
        &self,
        _card: &ModelCard,
        _room: Room,
        _inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        future::ready(Ok(Vec::new()))
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        _query: &str,
        _documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        future::ready(Ok(Vec::new()))
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        _text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        future::ready(Ok(Vec::new()))
    }

    fn chat(
        &self,
        _card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        assert_eq!(room, Room::Free);
        self.requests
            .lock()
            .expect("request lock")
            .push(request.clone());
        future::ready(Ok(
            r#"{"passage":"scheduler job definition","keywords":"scheduler"}"#.to_owned(),
        ))
    }
}

#[tokio::test]
async fn an_expansion_sends_the_question_alone_with_the_cards_controls() {
    let port = RecordingPort::default();
    let card = reasoning_answerer(false);
    let question = understand("  Scheduler   job ");
    let expander = HydeExpander::new(&port, &card).expect("a card that does not think");
    let expansion = expand(
        Some(&expander),
        &question,
        4000,
        Instant::now() + Duration::from_secs(5),
    )
    .await
    .expect("a valid expansion");
    assert_eq!(expansion.passage, "scheduler job definition");
    let requests = port.requests.lock().expect("request lock");
    let [request] = requests.as_slice() else {
        panic!("one chat request, not {}", requests.len());
    };
    assert_eq!(request.max_output_tokens, 256);
    assert_eq!(
        request
            .messages
            .iter()
            .map(|message| message.speaker)
            .collect::<Vec<_>>(),
        [Speaker::System, Speaker::User]
    );
    assert_eq!(request.messages[1].content, question.normalized);
    assert_eq!(
        request.chat_template_kwargs,
        BTreeMap::from([("enable_thinking".to_owned(), ControlValue::Boolean(false))])
    );
}

#[test]
fn a_card_that_thinks_or_does_not_answer_cannot_expand() {
    let port = RecordingPort::default();
    let thinking = reasoning_answerer(true);
    assert_eq!(
        HydeExpander::new(&port, &thinking).map(drop),
        Err(HydeCardError::Thinks)
    );
    assert!(HydeCardError::Thinks.to_string().contains("thinks"));
    let reranker = card(Role::Reranker, 8192);
    assert_eq!(
        HydeExpander::new(&port, &reranker).map(drop),
        Err(HydeCardError::NotAnswerer)
    );
}

/// An expander with no model behind it: search takes any adapter.
struct Scripted {
    expansion: Option<Expansion>,
}

impl QueryExpander for Scripted {
    fn expand<'a>(&'a self, _question: &'a Understood) -> ExpansionFuture<'a> {
        let expansion = self.expansion.clone();
        Box::pin(async move {
            match expansion {
                Some(expansion) => Ok(expansion),
                None => future::pending().await,
            }
        })
    }
}

#[tokio::test]
async fn search_bounds_and_guards_any_expander() {
    let question = understand("rerun a job without confirmation");
    let later = Instant::now() + Duration::from_secs(5);
    assert_eq!(
        expand(None, &question, 100, later).await,
        Err(ExpansionFailure::ModelUnavailable)
    );
    let hangs = Scripted { expansion: None };
    let started = Instant::now();
    assert_eq!(
        expand(
            Some(&hangs),
            &question,
            5000,
            started + Duration::from_millis(50)
        )
        .await,
        Err(ExpansionFailure::DeadlineExceeded)
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(
        expand(Some(&hangs), &question, 20, later).await,
        Err(ExpansionFailure::DeadlineExceeded)
    );
    let drops = Scripted {
        expansion: Some(Expansion {
            passage: "Rerun a job with a confirmation.".to_owned(),
            keywords: "rerun".to_owned(),
        }),
    };
    assert_eq!(
        expand(Some(&drops), &question, 100, later).await,
        Err(ExpansionFailure::ProtectedMissing)
    );
    let keeps = Scripted {
        expansion: Some(Expansion {
            passage: "Rerun a job without confirmation.".to_owned(),
            keywords: "rerun".to_owned(),
        }),
    };
    assert!(expand(Some(&keeps), &question, 100, later).await.is_ok());
}
