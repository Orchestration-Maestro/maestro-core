//! The deterministic fake behind the model port, which public CI uses since it
//! has no GPU (P-004): the same input always gives the same output, on every
//! platform.

use super::{
    card::{ModelCard, Role},
    port::{Error, Message, ModelPort, Room, Speaker, embedder_dimensions, require},
};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeSet,
    future::{self, Future},
};

/// Models that compute their answers from the text alone. The embedder's
/// vectors derive from a digest of the text, at the card's dimensions; the
/// reranker scores a document by the distinct words it shares with the query;
/// the tokenizer gives one token per word, whatever the card; and the
/// answerer replies with the line of evidence, from the user's messages
/// before the last, that shares the most words with the last message, the
/// question, or with nothing when no line shares one. They load nothing, so
/// the room a call names changes nothing.
#[derive(Debug)]
pub struct FakeModels;

impl ModelPort for FakeModels {
    fn embed(
        &self,
        card: &ModelCard,
        _room: Room,
        inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        future::ready(embedder_dimensions(card).map(|dimensions| {
            inputs
                .iter()
                .map(|input| vector(input, dimensions.get()))
                .collect()
        }))
    }

    fn rerank(
        &self,
        card: &ModelCard,
        _room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        future::ready(require(card, Role::Reranker).map(|()| {
            let wanted = vocabulary(query);
            documents
                .iter()
                .map(|document| shared(&wanted, document))
                .collect()
        }))
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        future::ready(Ok(words(text).map(token_id).collect()))
    }

    fn chat(
        &self,
        card: &ModelCard,
        _room: Room,
        messages: &[Message],
    ) -> impl Future<Output = Result<String, Error>> + Send {
        future::ready(require(card, Role::Answerer).map(|()| reply(messages).to_owned()))
    }
}

/// `dimensions` values from the blocks SHA-256(`text` ‖ block number as 4
/// big-endian bytes), each value two bytes of a block read as a
/// little-endian `i16` over 32768, so in [-1, 1).
fn vector(text: &str, dimensions: usize) -> Vec<f32> {
    (0_u32..)
        .flat_map(|block| {
            let digest = Sha256::new()
                .chain_update(text)
                .chain_update(block.to_be_bytes())
                .finalize();
            let (low, high) = (digest.iter().step_by(2), digest.iter().skip(1).step_by(2));
            low.zip(high)
                .map(|(low, high)| f32::from(i16::from_le_bytes([*low, *high])) / 32768.0)
                .collect::<Vec<f32>>()
        })
        .take(dimensions)
        .collect()
}

/// The words of `text`: its runs of letters and digits.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

/// The distinct words of `text`, in lower case.
fn vocabulary(text: &str) -> BTreeSet<String> {
    words(text).map(str::to_lowercase).collect()
}

/// How many distinct words of `wanted` `text` holds.
fn shared(wanted: &BTreeSet<String>, text: &str) -> f64 {
    let held = vocabulary(text);
    wanted
        .iter()
        .filter(|word| held.contains(*word))
        .map(|_| 1.0)
        .sum()
}

/// A word's token: the first four bytes of its SHA-256, read little-endian.
fn token_id(word: &str) -> u32 {
    let [first, second, third, fourth, ..]: [u8; 32] = Sha256::digest(word).into();
    u32::from_le_bytes([first, second, third, fourth])
}

/// The first line of the user's messages before the last that shares the
/// most words with the last; empty when none shares one.
fn reply(messages: &[Message]) -> &str {
    let Some((question, evidence)) = messages.split_last() else {
        return "";
    };
    let wanted = vocabulary(&question.content);
    let lines = evidence
        .iter()
        .filter(|message| message.speaker == Speaker::User)
        .flat_map(|message| message.content.lines());
    let mut best = ("", 0.0);
    for line in lines {
        let score = shared(&wanted, line);
        if score > best.1 {
            best = (line, score);
        }
    }
    best.0
}
