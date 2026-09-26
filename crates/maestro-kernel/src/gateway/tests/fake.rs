//! Tests of the deterministic fake: its outputs are fixed by its inputs, the
//! same on every run and every platform, and each call keeps to its role.
//! The fake loads nothing, so each call names a room it ignores.

use super::{
    super::{CardFields, Error, FakeModels, Message, ModelPort, Role, Room, Speaker},
    fixture::{card, card_of, fields},
};
use std::num::NonZeroUsize;

/// A message of `speaker` saying `content`.
fn message(speaker: Speaker, content: &str) -> Message {
    Message {
        speaker,
        content: content.to_owned(),
    }
}

#[tokio::test]
async fn fake_vectors_derive_from_a_digest_of_the_text() {
    let card = card_of(&CardFields {
        dimensions: NonZeroUsize::new(20),
        ..fields(Role::Embedder)
    });
    let texts = ["The whale sings.", "A whale sings."].map(str::to_owned);
    let vectors = FakeModels.embed(&card, Room::Free, &texts).await.unwrap();
    // Each value is two bytes of SHA-256(text ‖ block as 4 big-endian bytes),
    // read as a little-endian i16 over 32768, 16 values a block: Python's
    // hashlib and struct give these.
    let expected: [[f32; 20]; 2] = [
        [
            -0.289_672_85,
            0.335_723_88,
            0.807_922_36,
            -0.290_710_45,
            0.872_650_15,
            0.628_448_5,
            -0.729_064_94,
            -0.105_926_51,
            0.977_722_17,
            -0.451_385_5,
            -0.564_819_34,
            -0.305_725_1,
            0.435_882_57,
            -0.218_109_13,
            -0.494_201_66,
            0.794_586_2,
            -0.991_058_35,
            -0.242_553_71,
            0.552_368_16,
            -0.016_235_352,
        ],
        [
            -0.480_133_06,
            0.709_045_4,
            0.636_016_85,
            0.816_833_5,
            0.596_649_17,
            0.199_127_2,
            -0.287_323,
            0.962_585_45,
            -0.464_050_3,
            0.933_471_7,
            0.367_645_26,
            -0.005_340_576,
            0.177_673_34,
            -0.453_430_18,
            0.835_418_7,
            0.908_233_64,
            -0.835_601_8,
            -0.338_806_15,
            0.831_115_7,
            -0.566_589_36,
        ],
    ];
    assert_eq!(vectors, expected);
}

#[tokio::test]
async fn the_fake_reranker_scores_the_words_shared_with_the_query() {
    let documents = [
        "Whales sing.",
        "The blue whale sings; a whale!",
        "whale whale whale",
        "BLUE sea, the end",
        "",
    ]
    .map(str::to_owned);
    let card = card(Role::Reranker);
    let scores = FakeModels
        .rerank(&card, Room::Free, "The blue WHALE sings", &documents)
        .await
        .unwrap();
    assert_eq!(scores, [0.0, 4.0, 1.0, 2.0, 0.0]);
}

#[tokio::test]
async fn fake_tokens_come_back_one_per_word_in_order() {
    // The first four bytes of each word's SHA-256, read little-endian.
    let (the, whale, sings, lower_the) = (249_054_387, 682_545_829, 929_686_021, 2_104_326_073);
    for role in [Role::Embedder, Role::Reranker, Role::Answerer] {
        let ids = FakeModels
            .tokenize(&card(role), Room::Free, "The whale sings, the whale.")
            .await
            .unwrap();
        assert_eq!(ids, [the, whale, sings, lower_the, whale]);
    }
}

#[tokio::test]
async fn the_fake_answers_with_the_evidence_line_closest_to_the_question() {
    let messages = [
        message(
            Speaker::System,
            "Say whether blue whales sing, from the evidence.",
        ),
        message(
            Speaker::User,
            "Ships sail at dawn.\nBlue whales sing long songs.\nWhales sing.",
        ),
        message(Speaker::Assistant, "Do blue whales sing long songs?"),
        message(Speaker::User, "Do blue whales sing?"),
    ];
    let reply = FakeModels
        .chat(&card(Role::Answerer), Room::Any, &messages)
        .await
        .unwrap();
    assert_eq!(reply, "Blue whales sing long songs.");
}

#[tokio::test]
async fn the_fake_keeps_the_first_of_equally_close_lines() {
    let messages = [
        message(Speaker::User, "Whales sing.\nWhales sing loudly."),
        message(Speaker::User, "Whales sing loudly, and ships sail."),
        message(Speaker::User, "Do whales sing?"),
    ];
    let reply = FakeModels
        .chat(&card(Role::Answerer), Room::Any, &messages)
        .await
        .unwrap();
    assert_eq!(reply, "Whales sing.");
}

#[tokio::test]
async fn the_fake_says_nothing_the_evidence_does_not_hold() {
    let card = card(Role::Answerer);
    let unrelated = [
        message(Speaker::User, "Ships sail at dawn."),
        message(Speaker::User, "Do whales sing?"),
    ];
    let reply = FakeModels.chat(&card, Room::Any, &unrelated).await;
    assert_eq!(reply.unwrap(), "");
    let silence = FakeModels.chat(&card, Room::Any, &[]).await;
    assert_eq!(silence.unwrap(), "");
}

#[tokio::test]
async fn the_fake_refuses_a_card_for_another_role() {
    let (embedder, reranker) = (card(Role::Embedder), card(Role::Reranker));
    let texts = ["Whales sing.".to_owned()];
    let question = [message(Speaker::User, "Do whales sing?")];
    let refusals = [
        (
            FakeModels
                .embed(&reranker, Room::Free, &texts)
                .await
                .unwrap_err(),
            Role::Embedder,
        ),
        (
            FakeModels
                .rerank(&embedder, Room::Free, "whale", &texts)
                .await
                .unwrap_err(),
            Role::Reranker,
        ),
        (
            FakeModels
                .chat(&embedder, Room::Any, &question)
                .await
                .unwrap_err(),
            Role::Answerer,
        ),
    ];
    for (error, needed) in refusals {
        assert!(
            matches!(&error, Error::WrongRole { needed: wanted, .. } if *wanted == needed),
            "{error}"
        );
    }
}
