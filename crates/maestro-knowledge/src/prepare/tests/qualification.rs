//! Qualification: a router tokenizer exists only once the port gives every
//! parity fixture the native counter's ordered IDs.

use super::{
    super::{
        QualificationMode, RouterTokenizer, TokenizerError, TokenizerQualification,
        parity::fixtures,
    },
    port::{Answer, Goldens},
    support::{BUILD, MODEL_FILE, OTHER_FILE, SHORT_DEADLINE, card, embedder, identity},
};
use maestro_canonicalization::TokenCounter;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardIdentity, ModelCard, Role, Room,
        card_v2::{FlagValue, QualificationMethod},
    },
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};

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
            "# Relay-X 4.2.17\n\n## Server / SSL\n\n{}",
            "a ".repeat(687)
        )
    );
    assert_eq!(port.rooms(), [Room::Free; 41]);
}

#[test]
fn a_port_that_differs_on_any_one_fixture_is_refused_naming_it() {
    let mut refused = 0;
    for fixture in fixtures().unwrap() {
        // The same IDs in the reverse order, so the same count; all of them
        // but the last, a prefix; and one more.
        let mut reversed = fixture.ids.clone();
        reversed.reverse();
        let prefix = fixture.ids[..fixture.ids.len() - 1].to_vec();
        let mut longer = fixture.ids.clone();
        longer.insert(1, 6);
        for answered in [reversed, prefix, longer] {
            let port = Goldens::new();
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
                other => panic!("{} answered {answered:?}: {other:?}", fixture.name),
            }
            refused += 1;
        }
    }
    assert_eq!(refused, 3 * 41);
}

#[test]
fn qualification_stops_at_the_first_disagreement() {
    let port = Goldens::new();
    // Each golden with two of its IDs swapped: the same IDs, in another order.
    port.answer("a\0b", Answer::Ids(vec![0, 10, 275, 3, 2]));
    port.answer(
        "café naïve Ångström",
        Answer::Ids(vec![0, 26216, 9392, 24, 272, 8839, 449, 30011, 2]),
    );
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
    assert_eq!(router, [0, 26216, 9392, 24, 272, 8839, 449, 30011, 2]);
    // `nfc` is the ninth fixture, `nul` the seventeenth: never asked.
    assert_eq!(port.texts().len(), 9);
}

#[test]
fn a_port_that_never_answers_refuses_to_qualify_at_the_deadline() {
    let port = Goldens::new();
    port.answer("Hello world", Answer::Never);
    let refused =
        RouterTokenizer::qualify_within(port.clone(), embedder(), SHORT_DEADLINE).unwrap_err();
    let TokenizerError::TimedOut { after } = refused else {
        panic!("{refused:?}");
    };
    assert_eq!(after, SHORT_DEADLINE);
    // The empty input, then `Hello world`, which never answered: nothing
    // after it was asked.
    assert_eq!(port.texts(), ["", "Hello world"]);
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

#[test]
fn legacy_qualification_refuses_v2_cards_before_any_tokenize_call() {
    let profile = TokenizerQualification::parse(&qualification_bytes(0, MODEL_FILE)).unwrap();
    let port = Goldens::new();
    let refused =
        RouterTokenizer::qualify(port.clone(), v2_card(&profile, MODEL_FILE)).unwrap_err();

    assert!(matches!(
        refused,
        TokenizerError::Qualification { ref reason }
            if reason.contains("qualify_with_profile")
    ));
    assert!(port.texts().is_empty());
}

#[test]
fn v2_candidate_qualifies_only_against_its_own_ordered_native_ids() {
    let first_bytes = qualification_bytes(0, MODEL_FILE);
    let first = TokenizerQualification::parse(&first_bytes).unwrap();
    assert_eq!(first.digest(), &Digest::of(&first_bytes));
    assert!(first.is_synthetic());
    assert_eq!(first.qualification_mode(), QualificationMode::Synthetic);
    let first_card = v2_card(&first, MODEL_FILE);
    assert_eq!(first_card.format_document("body"), "doc: body");
    assert_eq!(
        first_card.format_query("question en français"),
        concat!(
            "Instruct: Given a web search query, retrieve relevant passages that ",
            "answer the query\nQuery: question en français"
        )
    );
    assert_eq!(embedder().format_document("body"), "body");
    assert_eq!(embedder().format_query("query"), "query");
    let first_port = Goldens::new();
    let first_tokenizer =
        RouterTokenizer::qualify_with_profile(first_port.clone(), first_card.clone(), &first)
            .unwrap();
    assert_eq!(
        first_tokenizer.qualification_mode(),
        QualificationMode::Synthetic
    );
    assert!(first_tokenizer.is_qualified_for(&first_card, &first));
    assert!(!first_tokenizer.is_qualified_for(&v2_card(&first, OTHER_FILE), &first));
    assert_eq!(first_port.texts().len(), 41);
    assert_eq!(first_port.rooms(), [Room::Free; 41]);
    assert_ne!(
        first_tokenizer.count("document").unwrap(),
        first_tokenizer.count_document("document").unwrap()
    );
    assert_eq!(first_port.texts().last().unwrap(), "doc: document");
}

#[test]
fn a_v2_tokenizer_check_refuses_candidate_canary_drift() {
    let profile = TokenizerQualification::parse(&qualification_bytes(0, MODEL_FILE)).unwrap();
    let card = v2_card(&profile, MODEL_FILE);
    let port = Goldens::new();
    let tokenizer = RouterTokenizer::qualify_with_profile(port.clone(), card, &profile).unwrap();
    let canary = fixtures()
        .unwrap()
        .into_iter()
        .find(|fixture| fixture.name == "specials")
        .unwrap();
    let mut changed = canary.ids.clone();
    changed[4] = changed[4].wrapping_add(1);
    port.answer(&canary.input, Answer::Ids(changed.clone()));

    assert!(matches!(
        tokenizer.check(),
        Err(TokenizerError::Disagreement { fixture, router, .. })
            if fixture == "specials" && router == changed
    ));
}

#[test]
fn another_v2_candidate_uses_its_own_ordered_native_ids() {
    let second_bytes = qualification_bytes(1, OTHER_FILE);
    let second = TokenizerQualification::parse(&second_bytes).unwrap();
    let second_card = v2_card(&second, OTHER_FILE);
    let second_port = Goldens::new();
    for fixture in fixtures().unwrap() {
        let ids = fixture.ids.into_iter().map(|id| id + 1).collect();
        second_port.answer(&fixture.input, Answer::Ids(ids));
    }
    RouterTokenizer::qualify_with_profile(second_port.clone(), second_card, &second).unwrap();
    assert_eq!(second_port.texts().len(), 41);
    assert_eq!(second_port.rooms(), [Room::Free; 41]);

    let wrong_port = Goldens::new();
    assert!(matches!(
        RouterTokenizer::qualify_with_profile(
            wrong_port.clone(),
            v2_card(&second, OTHER_FILE),
            &second
        ),
        Err(TokenizerError::Disagreement { .. })
    ));
    assert_eq!(wrong_port.texts().len(), 1);
}

#[test]
fn each_candidate_provenance_mismatch_is_refused_before_tokenization() {
    for mismatch in [
        "model",
        "tokenizer",
        "build",
        "date",
        "native_tool",
        "library",
        "method",
    ] {
        let mut artifact = qualification_value(0, MODEL_FILE);
        match mismatch {
            "model" => artifact["model_digest"] = json!(OTHER_FILE),
            "tokenizer" => artifact["tokenizer_digest"] = json!(OTHER_FILE),
            "build" => artifact["llama_cpp_build"] = json!("b6600-9e8d7c6b"),
            "date" => artifact["created"] = json!("2025-09-27"),
            "native_tool" => artifact["native_tool"]["version"] = json!("2.0"),
            "library" => artifact["library"]["version"] = json!("2.0"),
            "method" => {}
            _ => assert_eq!(mismatch, "method"),
        }
        let profile =
            TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).unwrap();
        let mut candidate = identity(profile.digest().clone());
        if mismatch == "method" {
            candidate.provenance.qualification_method = QualificationMethod::NativeRuntime;
        }
        let card = record_v2_card(&candidate);
        let port = Goldens::new();
        assert!(
            matches!(
                RouterTokenizer::qualify_with_profile(port.clone(), card, &profile),
                Err(TokenizerError::Qualification { .. })
            ),
            "{mismatch}"
        );
        assert!(port.texts().is_empty(), "{mismatch}");
    }
}

#[test]
fn v2_qualification_rejects_incomplete_duplicate_or_out_of_vocabulary_fixtures() {
    let valid = qualification_value(0, MODEL_FILE);

    let mut missing = valid.clone();
    missing["fixtures"].as_array_mut().unwrap().pop();
    assert!(TokenizerQualification::parse(&serde_json::to_vec(&missing).unwrap()).is_err());

    let mut duplicate = valid.clone();
    let first = duplicate["fixtures"][0].clone();
    duplicate["fixtures"].as_array_mut().unwrap().push(first);
    assert!(TokenizerQualification::parse(&serde_json::to_vec(&duplicate).unwrap()).is_err());

    let mut unqualified_canary = valid.clone();
    unqualified_canary["fixtures"][0]["canary"] = Value::Bool(false);
    assert!(
        TokenizerQualification::parse(&serde_json::to_vec(&unqualified_canary).unwrap()).is_err()
    );

    let mut out_of_vocabulary = valid;
    out_of_vocabulary["fixtures"][0]["ids"][0] = json!(100_000);
    assert!(
        TokenizerQualification::parse(&serde_json::to_vec(&out_of_vocabulary).unwrap()).is_err()
    );
}

#[test]
fn qualification_requires_exact_id_counts_at_token_budget_boundaries() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    let plain_500 = fixture_mut(&mut artifact, "plain_500");
    plain_500["ids"] = json!([0]);
    plain_500["confirmatory_ids"] = json!([0]);

    assert!(TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err());
}

#[test]
fn qualification_requires_committed_input_for_fixed_fixtures() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    fixture_mut(&mut artifact, "ascii")["input"] = json!("one-word input");

    assert!(TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err());
}

#[test]
fn qualification_allows_candidate_specific_boundary_and_special_inputs() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    fixture_mut(&mut artifact, "plain_500")["input"] = json!("candidate boundary fill");
    for name in ["specials", "special_only", "mask_spaces"] {
        fixture_mut(&mut artifact, name)["input"] = json!(format!("<|candidate-{name}|>"));
    }

    assert!(TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_ok());
}

#[test]
fn qualification_refuses_41_single_token_inputs() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    for fixture in artifact["fixtures"].as_array_mut().unwrap() {
        fixture["input"] = json!("x");
        fixture["ids"] = json!([0]);
        fixture["confirmatory_ids"] = json!([0]);
    }

    assert!(TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err());
}

fn fixture_mut<'a>(artifact: &'a mut Value, name: &str) -> &'a mut Value {
    artifact["fixtures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fixture| fixture["name"].as_str() == Some(name))
        .unwrap()
}

fn qualification_bytes(id_offset: u32, model_digest: &str) -> Vec<u8> {
    serde_json::to_vec(&qualification_value(id_offset, model_digest)).unwrap()
}

pub(super) fn qualification_value(id_offset: u32, model_digest: &str) -> Value {
    let fixtures = fixtures().unwrap();
    let maximum = fixtures
        .iter()
        .flat_map(|fixture| fixture.ids.iter())
        .copied()
        .max()
        .unwrap_or_default()
        + id_offset;
    json!({
        "schema": "maestro-tokenizer-qualification/1",
        "mode": "synthetic",
        "model_digest": model_digest,
        "tokenizer_digest": model_digest,
        "llama_cpp_build": BUILD,
        "native_tool": {"name": "native-tool", "version": "1.0"},
        "library": {"name": "tokenizer-library", "version": "1.0"},
        "created": "2026-09-27",
        "add_special": true,
        "parse_special": true,
        "vocabulary": {"minimum_id": 0, "maximum_id": maximum + 1},
        "fixtures": fixtures.into_iter().map(|fixture| {
            let ids: Vec<u32> = fixture.ids.iter().map(|id| id + id_offset).collect();
            json!({
                "name": fixture.name,
                "input": fixture.input,
                "ids": ids.clone(),
                "confirmatory_ids": ids,
                "canary": fixture.canary,
            })
        }).collect::<Vec<_>>(),
    })
}

pub(super) fn v2_card(profile: &TokenizerQualification, model_digest: &str) -> ModelCard {
    let mut candidate = identity(profile.digest().clone());
    if model_digest != MODEL_FILE {
        let digest = Digest::parse(model_digest).unwrap();
        candidate.weights.gguf_digest = digest.clone();
        candidate.formats.tokenizer_digest = digest.clone();
        candidate.invocation.server_flags.insert(
            "--model".to_owned(),
            FlagValue::Asset {
                name: "weights".to_owned(),
                digest,
            },
        );
    }
    record_v2_card(&candidate)
}

fn record_v2_card(identity: &CardIdentity) -> ModelCard {
    let root = scratch_directory().unwrap();
    let card = ModelCard::record_v2(&Store::new(&root), identity).unwrap();
    fs::remove_dir_all(root).unwrap();
    card
}
