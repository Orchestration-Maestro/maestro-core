//! V2 card schema, weight, and runtime flag validation tests.

use super::v2::{answerer_identity, card_identity, digest, scratch_store};
use crate::{
    artifact::Store,
    gateway::{
        ModelCard, Role,
        card_types::CardError,
        card_v2::{
            Capability, CardIdentity, ControlValue, Dimensions, EmbeddingFormat, FlagValue,
            KvCacheType, Quantization, SamplingParameters,
        },
    },
};
use std::{collections::BTreeMap, fs};

pub(super) fn refuses(identity: &CardIdentity, store: &Store, expected: &str) {
    let Err(error) = ModelCard::record_v2(store, identity) else {
        panic!("accepted identity; expected refusal containing {expected:?}");
    };
    let CardError::Invalid(reason) = error else {
        panic!("expected a card validation error, got {error}");
    };
    assert!(
        reason.contains(expected),
        "expected refusal containing {expected:?}, got {reason:?}"
    );
}

pub(super) fn accepts(identity: &CardIdentity, store: &Store) {
    assert!(
        ModelCard::record_v2(store, identity).is_ok(),
        "refused valid identity"
    );
}

pub(super) fn configured_sampling() -> SamplingParameters {
    SamplingParameters {
        temperature: 0.0,
        top_p: 1.0,
        top_k: 40,
        min_p: 0.0,
        typical_p: 0.0,
        repeat_penalty: 0.0001,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: Some(1),
    }
}

pub(super) fn reranker_identity() -> CardIdentity {
    let mut identity = card_identity();
    identity.role = Role::Reranker;
    identity.invocation.dimensions = Dimensions::NotApplicable;
    identity.formats.embedding = EmbeddingFormat::NotApplicable;
    identity.formats.document = Capability::NotApplicable;
    identity.formats.query = Capability::NotApplicable;
    identity
}

#[test]
fn quantization_uses_exact_gguf_wire_spellings() {
    let cases = [
        (Quantization::F32, r#""F32""#),
        (Quantization::F16, r#""F16""#),
        (Quantization::Bf16, r#""BF16""#),
        (Quantization::Q8_0, r#""Q8_0""#),
        (Quantization::Q6K, r#""Q6_K""#),
        (Quantization::Q5KSmall, r#""Q5_K_S""#),
        (Quantization::Q5KMedium, r#""Q5_K_M""#),
        (Quantization::Q5_0, r#""Q5_0""#),
        (Quantization::Q5_1, r#""Q5_1""#),
        (Quantization::Q4KSmall, r#""Q4_K_S""#),
        (Quantization::Q4KMedium, r#""Q4_K_M""#),
        (Quantization::Q4_0, r#""Q4_0""#),
        (Quantization::Q4_1, r#""Q4_1""#),
        (Quantization::Q3KSmall, r#""Q3_K_S""#),
        (Quantization::Q3KMedium, r#""Q3_K_M""#),
        (Quantization::Q3KLarge, r#""Q3_K_L""#),
        (Quantization::Q2K, r#""Q2_K""#),
        (Quantization::Iq1S, r#""IQ1_S""#),
        (Quantization::Iq2Xxs, r#""IQ2_XXS""#),
        (Quantization::Iq2Xs, r#""IQ2_XS""#),
        (Quantization::Iq3Xxs, r#""IQ3_XXS""#),
        (Quantization::Iq3S, r#""IQ3_S""#),
        (Quantization::Iq4Nl, r#""IQ4_NL""#),
        (Quantization::Iq4Xs, r#""IQ4_XS""#),
    ];

    for (variant, expected) in cases {
        assert_eq!(serde_json::to_string(&variant).unwrap(), expected);
        let decoded: Quantization = serde_json::from_str(expected).unwrap();
        assert_eq!(serde_json::to_string(&decoded).unwrap(), expected);
    }
}

#[test]
fn v2_refuses_blank_weight_facts_bad_urls_and_unsafe_asset_names() {
    let (path, store) = scratch_store();
    for field in [
        "upstream_model_id",
        "upstream_revision",
        "source_url",
        "licence_id",
        "licence_terms_source",
    ] {
        let mut identity = card_identity();
        match field {
            "upstream_model_id" => identity.weights.upstream_model_id.clear(),
            "upstream_revision" => identity.weights.upstream_revision.clear(),
            "source_url" => identity.weights.source_url.clear(),
            "licence_id" => identity.weights.licence_id.clear(),
            "licence_terms_source" => identity.weights.licence_terms_source.clear(),
            _ => panic!("unknown weight field {field}"),
        }
        refuses(&identity, &store, &format!("{field} is blank"));
    }

    for (field, url) in [
        ("source_url", "ftp://example.invalid/model"),
        ("source_url", "http://user@example.invalid/model"),
        (
            "licence_terms_source",
            "https://:secret@example.invalid/license",
        ),
    ] {
        let mut identity = card_identity();
        match field {
            "source_url" => identity.weights.source_url = url.to_owned(),
            "licence_terms_source" => identity.weights.licence_terms_source = url.to_owned(),
            _ => panic!("unknown source URL field {field}"),
        }
        refuses(
            &identity,
            &store,
            &format!("{field} must be an HTTP(S) URL without credentials"),
        );
    }

    for (name, expected) in [
        ("/absolute", "adapter asset name is a path"),
        (r"dir\adapter", "adapter asset name is a path"),
        ("drive:adapter", "adapter asset name is a path"),
        ("a:b", "adapter asset name is a path"),
        ("..", "adapter asset name is a path"),
        ("nested/../adapter", "adapter asset name is a path"),
        (" ", "adapter is blank"),
    ] {
        let mut identity = card_identity();
        identity
            .weights
            .adapters
            .insert(name.to_owned(), digest('e'));
        refuses(&identity, &store, expected);
    }
    let mut identity = card_identity();
    identity
        .weights
        .drafts
        .insert("../draft".to_owned(), digest('e'));
    refuses(&identity, &store, "draft asset name is a path");
    let mut identity = card_identity();
    identity
        .weights
        .projectors
        .insert("../projector".to_owned(), digest('e'));
    refuses(&identity, &store, "projector asset name is a path");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_requires_embedded_tokenizer_identity_and_uses_gguf_pin_exceptions_exactly() {
    let (path, store) = scratch_store();
    let mut invalid = card_identity();
    invalid.formats.tokenizer_digest = digest('e');
    refuses(
        &invalid,
        &store,
        "GGUF-embedded tokenizer digest must equal the complete GGUF digest",
    );

    let embedded = card_identity();
    assert!(
        !embedded
            .artifact_digests()
            .contains(&embedded.weights.gguf_digest)
    );
    let mut separately_referenced = embedded.clone();
    separately_referenced.invocation.server_flags.insert(
        "--extra-weights".to_owned(),
        FlagValue::Asset {
            name: "extra".to_owned(),
            digest: embedded.weights.gguf_digest.clone(),
        },
    );
    assert!(
        !separately_referenced
            .artifact_digests()
            .contains(&embedded.weights.gguf_digest)
    );

    let mut external = card_identity();
    let adapter = digest('e');
    let draft = digest('f');
    let projector = digest('9');
    external
        .weights
        .adapters
        .insert("adapter".to_owned(), adapter.clone());
    external
        .weights
        .drafts
        .insert("draft".to_owned(), draft.clone());
    external
        .weights
        .projectors
        .insert("projector".to_owned(), projector.clone());
    for (flag, name, digest) in [
        ("--lora", "adapter", adapter.clone()),
        ("--model-draft", "draft", draft.clone()),
        ("--mmproj", "projector", projector.clone()),
    ] {
        external.invocation.server_flags.insert(
            flag.to_owned(),
            FlagValue::Asset {
                name: name.to_owned(),
                digest,
            },
        );
    }
    let pinned = external.artifact_digests();
    assert!(!pinned.contains(&adapter));
    assert!(!pinned.contains(&draft));
    assert!(!pinned.contains(&projector));
    accepts(&external, &store);
    accepts(&embedded, &store);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_runtime_flags_require_typed_assets_and_finite_values() {
    let (path, store) = scratch_store();
    let mut invalid = card_identity();
    invalid.invocation.llama_cpp_build = " ".to_owned();
    refuses(&invalid, &store, "llama_cpp_build is blank");

    let mut invalid = card_identity();
    invalid
        .invocation
        .server_flags
        .insert(" ".to_owned(), FlagValue::Text("plain".to_owned()));
    refuses(&invalid, &store, "server_flags key is blank");

    let mut invalid = card_identity();
    invalid
        .invocation
        .server_flags
        .insert("--temperature".to_owned(), FlagValue::Number(f64::NAN));
    refuses(&invalid, &store, "server_flags must be finite");

    for (flag, value) in [
        ("--model", "plain"),
        ("--path", "plain"),
        ("--file", "plain"),
        ("--adapter", "plain"),
        ("--projector", "plain"),
        ("--draft", "plain"),
        ("--lora", "plain"),
        ("--mmproj", "plain"),
        ("--weights", "reference"),
        ("--foo", "weights.gguf"),
        ("--weights", "/weights"),
        ("--weights", r"dir\weights"),
        ("--weights", "weights.gguf"),
        ("--weights", "weights.bin"),
        ("--weights", "weights.model"),
    ] {
        let mut invalid = card_identity();
        invalid
            .invocation
            .server_flags
            .insert(flag.to_owned(), FlagValue::Text(value.to_owned()));
        refuses(
            &invalid,
            &store,
            "file path must be a named digest-bound asset",
        );
    }

    let mut invalid = card_identity();
    invalid.invocation.server_flags.insert(
        "--adapter".to_owned(),
        FlagValue::Asset {
            name: "../adapter".to_owned(),
            digest: digest('e'),
        },
    );
    refuses(&invalid, &store, "--adapter asset name is a path");

    let mut invalid = answerer_identity();
    invalid.invocation.reasoning = Capability::Supported(BTreeMap::from([(
        " ".to_owned(),
        ControlValue::Text("value".to_owned()),
    )]));
    refuses(&invalid, &store, "reasoning control is blank");
    let mut invalid = answerer_identity();
    invalid.invocation.reasoning = Capability::Supported(BTreeMap::from([(
        "budget".to_owned(),
        ControlValue::Number(f64::NAN),
    )]));
    refuses(&invalid, &store, "reasoning control must be finite");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn runtime_cache_flags_cover_every_llama_element_type() {
    let (path, store) = scratch_store();
    for (cache, spelling) in [
        (KvCacheType::F32, "f32"),
        (KvCacheType::F16, "f16"),
        (KvCacheType::Bf16, "bf16"),
        (KvCacheType::Q8_0, "q8_0"),
        (KvCacheType::Q5_0, "q5_0"),
        (KvCacheType::Q4_0, "q4_0"),
        (KvCacheType::Q4_1, "q4_1"),
        (KvCacheType::Q5_1, "q5_1"),
        (KvCacheType::Iq4Nl, "iq4_nl"),
    ] {
        let mut identity = card_identity();
        identity.resources.kv_cache.key = cache;
        identity.invocation.server_flags.insert(
            "--cache-type-k".to_owned(),
            FlagValue::Text(spelling.to_owned()),
        );
        accepts(&identity, &store);
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_flag_names_must_be_canonical_long_options() {
    let (path, store) = scratch_store();
    for (flag, hint) in [
        ("-m", "--model"),
        ("-c", "--ctx-size"),
        ("-b", "--batch-size"),
        ("-ub", "--ubatch-size"),
        ("-ctk", "--cache-type-k"),
        ("-ctv", "--cache-type-v"),
        ("-np", "use its long option name"),
        ("-C", "use its long option name"),
        ("--ctx_size", "--ctx-size"),
        ("--batch_size", "--batch-size"),
        ("--ubatch_size", "--ubatch-size"),
        ("--cache_type_k", "--cache-type-k"),
        ("--cache_type_v", "--cache-type-v"),
        ("--foo.bar", "use its long option name"),
        ("--ctx-size=1", "--ctx-size"),
        ("--CTX-SIZE", "--ctx-size"),
        ("ctx-size", "use its long option name"),
    ] {
        let mut invalid = card_identity();
        invalid
            .invocation
            .server_flags
            .insert(flag.to_owned(), FlagValue::Integer(4096));
        refuses(&invalid, &store, hint);
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_refuses_noncanonical_aliases_for_typed_runtime_flags() {
    let cases = [
        (
            "-m",
            "--model",
            FlagValue::Asset {
                name: "weights".to_owned(),
                digest: digest('a'),
            },
        ),
        ("-c", "--ctx-size", FlagValue::Integer(4096)),
        ("-b", "--batch-size", FlagValue::Integer(512)),
        ("-ub", "--ubatch-size", FlagValue::Integer(128)),
        ("-ctk", "--cache-type-k", FlagValue::Text("q8_0".to_owned())),
        ("-ctv", "--cache-type-v", FlagValue::Text("q8_0".to_owned())),
    ];
    let (path, store) = scratch_store();
    for (alias, canonical, value) in cases {
        let mut identity = card_identity();
        identity
            .invocation
            .server_flags
            .insert(alias.to_owned(), value);
        let error = ModelCard::record_v2(&store, &identity).unwrap_err();
        assert!(
            matches!(&error, CardError::Invalid(reason) if reason.contains(canonical)),
            "{alias} should be refused in favor of {canonical}: {error}"
        );
    }
    fs::remove_dir_all(path).unwrap();
}
