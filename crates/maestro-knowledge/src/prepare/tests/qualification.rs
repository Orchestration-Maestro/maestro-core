//! Qualification: a router tokenizer exists only once the port gives every
//! parity fixture the native counter's ordered IDs.

use super::{
    super::{RouterTokenizer, TokenizerError, parity::fixtures},
    port::{Answer, Goldens},
    support::{BUILD, MODEL_FILE, OTHER_FILE, card, embedder},
};
use maestro_canonicalization::TokenCounter;
use maestro_kernel::gateway::{Role, Room};
use std::collections::BTreeSet;

#[test]
fn a_port_that_answers_the_native_goldens_qualifies_after_every_fixture() {
    let port = Goldens::new();
    RouterTokenizer::qualify(port.clone(), embedder()).unwrap();
    let texts = port.texts();
    // The 41 fixtures, once each, in the file's order: the 27 short inputs,
    // then those at the boundaries of the budgets and of the context.
    assert_eq!(texts.len(), 41);
    assert_eq!(texts[0], "");
    assert_eq!(texts[1], "Hello world");
    assert_eq!(texts[34], "a ".repeat(8191));
    assert_eq!(
        texts[40],
        format!(
            "# Control-M 9.0.22\n\n## Server / SSL\n\n{}",
            "a ".repeat(688)
        )
    );
    assert_eq!(port.rooms(), [Room::Free; 41]);
}

#[test]
fn a_port_that_differs_on_any_one_fixture_is_refused_naming_it() {
    let mut refused = 0;
    for fixture in fixtures().unwrap() {
        let port = Goldens::new();
        let mut answered = fixture.ids.clone();
        answered.insert(1, 6);
        port.answer(&fixture.input, Answer::Ids(answered.clone()));
        match RouterTokenizer::qualify(port, embedder()) {
            Err(TokenizerError::Disagreement {
                fixture: name,
                input,
                native,
                router,
            }) => {
                assert_eq!(name, fixture.name);
                assert_eq!(input, fixture.input, "{name}");
                assert_eq!(native, fixture.ids, "{name}");
                assert_eq!(router, answered, "{name}");
            }
            other => panic!("{}: {other:?}", fixture.name),
        }
        refused += 1;
    }
    assert_eq!(refused, 41);
}

#[test]
fn qualification_stops_at_the_first_disagreement() {
    let port = Goldens::new();
    port.answer("a\0b", Answer::Ids(vec![0, 10, 275, 2]));
    port.answer("café naïve Ångström", Answer::Ids(vec![0, 26216, 2]));
    let refused = RouterTokenizer::qualify(port.clone(), embedder()).unwrap_err();
    let TokenizerError::Disagreement {
        fixture,
        input,
        native,
        router,
    } = refused
    else {
        panic!("{refused:?}");
    };
    assert_eq!(fixture, "nfc");
    assert_eq!(input, "café naïve Ångström");
    assert_eq!(native, [0, 26216, 24, 9392, 272, 8839, 449, 30011, 2]);
    assert_eq!(router, [0, 26216, 2]);
    // `nfc` is the ninth fixture, `nul` the seventeenth: never asked.
    assert_eq!(port.texts().len(), 9);
}

#[test]
fn an_embedder_that_does_not_fit_in_free_room_refuses_to_qualify() {
    let port = Goldens::new();
    port.answer("", Answer::Unavailable);
    let refused = RouterTokenizer::qualify(port.clone(), embedder()).unwrap_err();
    let TokenizerError::Unavailable { reason } = refused else {
        panic!("{refused:?}");
    };
    assert_eq!(reason, "no free room for 1280 MiB");
    assert_eq!(port.rooms(), [Room::Free]);
}

#[test]
fn a_card_that_is_not_an_embedders_is_refused_before_any_call() {
    let port = Goldens::new();
    let reranker = card(Role::Reranker, MODEL_FILE, BUILD);
    let refused = RouterTokenizer::qualify(port.clone(), reranker.clone()).unwrap_err();
    let TokenizerError::NotAnEmbedder { card, role } = refused else {
        panic!("{refused:?}");
    };
    assert_eq!((&card, role), (reranker.digest(), Role::Reranker));
    assert!(port.texts().is_empty());
}

#[test]
fn the_contract_id_changes_with_the_card_and_with_the_router_build() {
    let cards = [
        embedder(),
        card(Role::Embedder, OTHER_FILE, BUILD),
        card(Role::Embedder, MODEL_FILE, "b6600-9e8d7c6b"),
    ];
    let mut contract_ids = BTreeSet::new();
    for card in cards {
        let expected = format!("router/1:sha256:{}", card.digest().as_str());
        let tokenizer = RouterTokenizer::qualify(Goldens::new(), card).unwrap();
        assert_eq!(tokenizer.contract_id(), expected);
        contract_ids.insert(expected);
    }
    assert_eq!(contract_ids.len(), 3);
}
