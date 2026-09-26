//! A model port that answers each parity fixture with the native counter's
//! IDs, the native goldens, and any other text as the gateway's fake does,
//! one token per word, unless a test changes what a text is answered with.
//! It records every call.

use super::super::parity::fixtures;
use maestro_kernel::gateway::{Error, FakeModels, Message, ModelCard, ModelPort, Room};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// What the port answers a text with, in place of its golden or the fake's
/// tokens.
#[derive(Debug, Clone)]
pub(super) enum Answer {
    /// These IDs.
    Ids(Vec<u32>),
    /// The router's refusal of a model that does not fit in free room.
    Unavailable,
    /// A panic, which stops the thread that calls the port.
    Panic,
}

/// The port. Its clones share their answers and calls, so a test keeps one
/// while a tokenizer holds another.
#[derive(Debug, Clone)]
pub(super) struct Goldens {
    /// The native counter's IDs, by input.
    native: Arc<HashMap<String, Vec<u32>>>,
    /// The answers a test changed, by text.
    changed: Arc<Mutex<HashMap<String, Answer>>>,
    /// Every call, in order: the room it named and its text.
    calls: Arc<Mutex<Vec<(Room, String)>>>,
}

impl Goldens {
    /// The port, answering every fixture with its golden.
    pub(super) fn new() -> Self {
        let native = fixtures()
            .unwrap()
            .into_iter()
            .map(|fixture| (fixture.input, fixture.ids))
            .collect();
        Self {
            native: Arc::new(native),
            changed: Arc::default(),
            calls: Arc::default(),
        }
    }

    /// From now on, `text` is answered with `answer`.
    pub(super) fn answer(&self, text: &str, answer: Answer) {
        self.changed.lock().unwrap().insert(text.to_owned(), answer);
    }

    /// The texts of the calls so far, in order.
    pub(super) fn texts(&self) -> Vec<String> {
        let calls = self.calls.lock().unwrap();
        calls.iter().map(|(_, text)| text.clone()).collect()
    }

    /// The rooms the calls so far named, in order.
    pub(super) fn rooms(&self) -> Vec<Room> {
        let calls = self.calls.lock().unwrap();
        calls.iter().map(|(room, _)| *room).collect()
    }
}

impl ModelPort for Goldens {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        FakeModels.embed(card, room, inputs).await
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        FakeModels.rerank(card, room, query, documents).await
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        self.calls.lock().unwrap().push((room, text.to_owned()));
        let changed = self.changed.lock().unwrap().get(text).cloned();
        match (changed, self.native.get(text)) {
            (Some(Answer::Ids(ids)), _) => Ok(ids),
            (Some(Answer::Unavailable), _) => Err(Error::Unavailable {
                reason: "no free room for 1280 MiB".to_owned(),
            }),
            (Some(Answer::Panic), _) => panic!("the port broke off"),
            (None, Some(golden)) => Ok(golden.clone()),
            (None, None) => FakeModels.tokenize(card, room, text).await,
        }
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, messages).await
    }
}
