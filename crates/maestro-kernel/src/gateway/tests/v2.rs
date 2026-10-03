//! V2 model-card identity contract.

use super::super::card_v2::{
    Backend, CachePolicy, Capability, CardFormats, CardIdentity, Dimensions, EmbeddingFormat,
    FlagValue, HardwareIdentity, KvCache, KvCacheType, MemoryEstimate, Normalization, Observation,
    OffloadMode, OffloadPolicy, Pooling, Provenance, QualificationMethod, QualifiedLimits,
    Quantization, Resources, RuntimeLimits, Sampling, SamplingParameters, Template, TextFormat,
    TokenizerDerivation, WeightIdentity,
};
use super::super::{CardError, Limits, ModelCard, Role, RouterEntry};
use super::v2_golden::CANONICAL_V2_CARD;
use crate::artifact::{Digest, Store};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    str,
};

pub(super) fn digest(byte: char) -> Digest {
    Digest::parse(&byte.to_string().repeat(64)).unwrap()
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture spells out every required immutable identity field"
)]
pub(super) fn card_identity() -> CardIdentity {
    CardIdentity {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        weights: WeightIdentity {
            upstream_model_id: "org/model".to_owned(),
            upstream_revision: "0123456789abcdef".to_owned(),
            source_url: "https://example.invalid/model".to_owned(),
            licence_id: "apache-2.0".to_owned(),
            licence_terms_source: "https://example.invalid/license".to_owned(),
            gguf_digest: digest('a'),
            gguf_bytes: NonZeroU64::new(1024).unwrap(),
            quantization: Quantization::Q8_0,
            adapters: BTreeMap::default(),
            drafts: BTreeMap::default(),
            projectors: BTreeMap::default(),
        },
        formats: CardFormats {
            tokenizer_digest: digest('a'),
            tokenizer_derivation: TokenizerDerivation::GgufEmbedded,
            qualification_digest: digest('c'),
            template: Template::Absent,
            system_format: String::new(),
            tool_format: Capability::Unsupported,
            document: Capability::Supported(TextFormat {
                prefix: String::new(),
                suffix: String::new(),
            }),
            query: Capability::Supported(TextFormat {
                prefix: "Instruct: retrieve\\nQuery: ".to_owned(),
                suffix: String::new(),
            }),
            embedding: EmbeddingFormat::Supported {
                pooling: Pooling::LastToken,
                normalization: Normalization::L2,
            },
        },
        invocation: RuntimeLimits {
            limits: Limits {
                context_tokens: NonZeroU32::new(4096).unwrap(),
                output_tokens: None,
            },
            dimensions: Dimensions::Measured(NonZeroUsize::new(1024).unwrap()),
            sampling: Sampling::NotApplicable,
            reasoning: Capability::NotApplicable,
            llama_cpp_build: "b1234-abcdef".to_owned(),
            runtime_binary_digest: digest('d'),
            backend: Backend::Cuda,
            server_flags: [(
                "--model".to_owned(),
                FlagValue::Asset {
                    name: "weights".to_owned(),
                    digest: digest('a'),
                },
            )]
            .into(),
        },
        resources: Resources {
            offload: OffloadPolicy {
                mode: OffloadMode::ExplicitDevices,
                devices: vec!["gpu0".to_owned()],
            },
            memory_estimate: MemoryEstimate {
                bytes: NonZeroU64::new(2_000_000_000).unwrap(),
                source: "publisher estimate".to_owned(),
            },
            slots: NonZeroU32::new(1).unwrap(),
            kv_cache: KvCache {
                key: KvCacheType::Q8_0,
                value: KvCacheType::Q8_0,
            },
            cache_policy: CachePolicy::KeepLoaded,
            batch_size: NonZeroU32::new(512).unwrap(),
            micro_batch_size: NonZeroU32::new(128).unwrap(),
            hardware: HardwareIdentity {
                device: "RTX 5090".to_owned(),
                driver_version: "616.56".to_owned(),
                host_cpu: "test cpu".to_owned(),
                host_memory_bytes: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
            },
            qualified_limits: QualifiedLimits {
                context_tokens: Observation::Measured {
                    value: NonZeroU32::new(4096).unwrap(),
                    provenance: "qualification run".to_owned(),
                },
                output_tokens: Observation::Unavailable {
                    reason: "not applicable".to_owned(),
                },
                concurrency: Observation::Measured {
                    value: NonZeroU32::new(1).unwrap(),
                    provenance: "qualification run".to_owned(),
                },
                peak_memory_bytes: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
            },
        },
        provenance: Provenance {
            identity_created: "2026-09-27".to_owned(),
            qualification_created: "2026-09-27".to_owned(),
            qualification_method: QualificationMethod::NativeTokenizer,
            tool_versions: [("llama.cpp".to_owned(), "b1234".to_owned())].into(),
            artifacts: [("native-qualification".to_owned(), digest('c'))].into(),
        },
    }
}

pub(super) fn scratch_store() -> (PathBuf, Store) {
    let path = scratch_directory().unwrap();
    let store = Store::new(&path);
    (path, store)
}

pub(super) fn answerer_identity() -> CardIdentity {
    let mut identity = card_identity();
    identity.role = Role::Answerer;
    identity.invocation.dimensions = Dimensions::NotApplicable;
    identity.invocation.limits.output_tokens = NonZeroU32::new(128);
    identity.invocation.sampling = Sampling::Configured(SamplingParameters {
        temperature: 0.1,
        top_p: 0.9,
        top_k: 40,
        min_p: 0.0,
        typical_p: 1.0,
        repeat_penalty: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: Some(1),
    });
    identity.invocation.reasoning = Capability::Unsupported;
    identity.formats.embedding = EmbeddingFormat::NotApplicable;
    identity.formats.document = Capability::NotApplicable;
    identity.formats.query = Capability::NotApplicable;
    identity
        .invocation
        .server_flags
        .insert("--temperature".to_owned(), FlagValue::Number(0.1));
    identity
}

#[test]
fn a_pure_v2_card_uses_the_same_canonical_digest_as_recording() {
    let (_, store) = scratch_store();
    let identity = card_identity();
    let recorded = ModelCard::record_v2(&store, &identity).unwrap();
    let declared = ModelCard::from_identity(&identity).unwrap();

    assert_eq!(declared.digest(), recorded.digest());
    assert_eq!(declared.digest(), &Digest::of(CANONICAL_V2_CARD.as_bytes()));
    assert_eq!(declared.identity(), Some(&identity));
}

#[test]
fn a_pure_answerer_card_preserves_sampling_and_output_identity() {
    let identity = answerer_identity();
    let card = ModelCard::from_identity(&identity).unwrap();

    assert_eq!(
        card.digest(),
        &Digest::of(super::v2_golden::CANONICAL_ANSWERER_CARD.as_bytes())
    );
    assert_eq!(card.identity(), Some(&identity));
}

#[test]
fn v2_identity_round_trips_deterministically_and_keeps_legacy_common_fields() {
    let (path, store) = scratch_store();
    let identity = card_identity();
    let first = ModelCard::record_v2(&store, &identity).unwrap();
    let second = ModelCard::record_v2(&store, &identity).unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.identity(), Some(&identity));
    assert_eq!(first.fields().role, Role::Embedder);
    assert_eq!(first.fields().dimensions, NonZeroUsize::new(1024));
    assert!(first.fields().suite_results.is_empty());
    let loaded = ModelCard::load(&store, first.digest()).unwrap();
    assert_eq!(loaded, first);
    let bytes = store.get(first.digest()).unwrap();
    assert_eq!(str::from_utf8(&bytes).unwrap(), CANONICAL_V2_CARD);
    assert_eq!(Digest::of(&bytes), *first.digest());
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["schema"],
        "maestro-model-card/2"
    );
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn each_immutable_identity_change_changes_the_v2_digest() {
    let (path, store) = scratch_store();
    let original = card_identity();
    let original_digest = ModelCard::record_v2(&store, &original)
        .unwrap()
        .digest()
        .clone();
    let mut changed = original.clone();
    changed.weights.gguf_digest = digest('e');
    changed.formats.tokenizer_digest = digest('e');
    changed.invocation.server_flags.insert(
        "--model".to_owned(),
        FlagValue::Asset {
            name: "weights".to_owned(),
            digest: digest('e'),
        },
    );
    assert_ne!(
        &original_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    let mut changed = original.clone();
    changed.weights.quantization = Quantization::Q5KMedium;
    assert_ne!(
        &original_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    let mut changed = original.clone();
    changed.formats.template = Template::Digest(digest('e'));
    assert_ne!(
        &original_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    let mut changed = original.clone();
    changed.formats.query = Capability::Supported(TextFormat {
        prefix: "different".to_owned(),
        suffix: String::new(),
    });
    assert_ne!(
        &original_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    let answerer = answerer_identity();
    let answerer_digest = ModelCard::record_v2(&store, &answerer)
        .unwrap()
        .digest()
        .clone();
    let mut changed = answerer;
    if let Sampling::Configured(settings) = &mut changed.invocation.sampling {
        settings.temperature = 0.2;
    }
    assert_ne!(
        &answerer_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    let mut changed = original;
    changed.invocation.limits.context_tokens = NonZeroU32::new(8192).unwrap();
    changed
        .invocation
        .server_flags
        .insert("--ctx-size".to_owned(), FlagValue::Integer(8192));
    assert_ne!(
        &original_digest,
        ModelCard::record_v2(&store, &changed).unwrap().digest()
    );
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_refuses_unknown_and_duplicate_fields() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &card_identity()).unwrap();
    let original = String::from_utf8(store.get(card.digest()).unwrap()).unwrap();

    let mut unknown: Value = serde_json::from_str(&original).unwrap();
    unknown["identity"]["unexpected"] = json!(true);
    let unknown_store = Store::new(path.join("unknown"));
    let unknown_digest = unknown_store.put(unknown.to_string().as_bytes()).unwrap();
    assert!(matches!(
        ModelCard::load(&unknown_store, &unknown_digest),
        Err(CardError::Invalid(_))
    ));

    let mut unknown_variant: Value = serde_json::from_str(&original).unwrap();
    unknown_variant["identity"]["invocation"]["server_flags"]["--model"]["unexpected"] =
        json!(true);
    let unknown_variant_store = Store::new(path.join("unknown-variant"));
    let unknown_variant_digest = unknown_variant_store
        .put(unknown_variant.to_string().as_bytes())
        .unwrap();
    assert!(matches!(
        ModelCard::load(&unknown_variant_store, &unknown_variant_digest),
        Err(CardError::Invalid(_))
    ));

    for (directory, malformed) in [
        (
            "duplicate-field",
            original.replacen('{', r#"{"schema":"maestro-model-card/2","#, 1),
        ),
        (
            "duplicate-map-key",
            original.replacen(
                "\"tool_versions\":{\"llama.cpp\":\"b1234\"}",
                "\"tool_versions\":{\"llama.cpp\":\"b1234\",\"llama.cpp\":\"b1234\"}",
                1,
            ),
        ),
        (
            "duplicate-nested-field",
            original.replacen(
                "\"gguf_bytes\":1024",
                "\"gguf_bytes\":1024,\"gguf_bytes\":1024",
                1,
            ),
        ),
    ] {
        let malformed_store = Store::new(path.join(directory));
        let malformed_digest = malformed_store.put(malformed.as_bytes()).unwrap();
        assert!(matches!(
            ModelCard::load(&malformed_store, &malformed_digest),
            Err(CardError::Invalid(_))
        ));
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn runtime_flags_must_match_repeated_typed_identity_fields() {
    let cases = [
        (
            "--model",
            FlagValue::Asset {
                name: "weights".to_owned(),
                digest: digest('e'),
            },
        ),
        ("--ctx-size", FlagValue::Integer(8192)),
        ("--batch-size", FlagValue::Integer(1024)),
        ("--ubatch-size", FlagValue::Integer(64)),
        ("--cache-type-k", FlagValue::Text("f16".to_owned())),
        ("--cache-type-v", FlagValue::Text("f16".to_owned())),
        ("--pooling", FlagValue::Text("cls".to_owned())),
    ];
    let (path, store) = scratch_store();
    for (flag, value) in cases {
        let mut identity = card_identity();
        identity
            .invocation
            .server_flags
            .insert(flag.to_owned(), value);
        let error = ModelCard::record_v2(&store, &identity).unwrap_err();
        let expected = if flag == "--model" {
            "server_flags.--model differs from weights.gguf_digest"
        } else {
            "differs from its typed identity"
        };
        assert!(
            matches!(&error, CardError::Invalid(reason) if reason.contains(expected)),
            "expected {flag} to disagree with its typed identity, got {error}"
        );
    }

    let mut identity = card_identity();
    for flag in [
        "--model",
        "--ctx-size",
        "--batch-size",
        "--ubatch-size",
        "--cache-type-k",
        "--cache-type-v",
        "--pooling",
    ] {
        identity.invocation.server_flags.remove(flag);
    }
    assert!(ModelCard::record_v2(&store, &identity).is_ok());

    let mut matching = card_identity();
    for (flag, value) in [
        ("--ctx-size", FlagValue::Integer(4096)),
        ("--batch-size", FlagValue::Integer(512)),
        ("--ubatch-size", FlagValue::Integer(128)),
        ("--cache-type-k", FlagValue::Text("q8_0".to_owned())),
        ("--cache-type-v", FlagValue::Text("q8_0".to_owned())),
        ("--pooling", FlagValue::Text("last".to_owned())),
    ] {
        matching
            .invocation
            .server_flags
            .insert(flag.to_owned(), value);
    }
    assert!(ModelCard::record_v2(&store, &matching).is_ok());
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_refuses_invalid_role_settings_and_absolute_paths() {
    let (path, store) = scratch_store();
    let mut invalid = card_identity();
    invalid.role = Role::Reranker;
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid.invocation.server_flags.insert(
        "--model".to_owned(),
        FlagValue::Text("/srv/models/model.gguf".to_owned()),
    );
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid
        .invocation
        .server_flags
        .insert("--temperature".to_owned(), FlagValue::Number(f64::NAN));
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid.provenance.identity_created = "2026-02-29".to_owned();
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid.weights.upstream_model_id = "/srv/models/model".to_owned();
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid
        .provenance
        .tool_versions
        .insert("runtime".to_owned(), "file:///srv/models/model".to_owned());
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    let mut invalid = card_identity();
    invalid.resources.offload.mode = OffloadMode::Automatic;
    assert!(matches!(
        ModelCard::record_v2(&store, &invalid),
        Err(CardError::Invalid(_))
    ));
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v1_cards_keep_their_exact_digest_and_have_no_inferred_identity() {
    let bytes = concat!(
        r#"{"schema":"maestro-model-card/1","role":"embedder","router_entry":"embed","#,
        r#""file_digest":"ca56847038f3f329524caec5a86e14865f49918d68094c90"#,
        r#"dd021a5e67b927f6","template_digest":null,"server_build":"b6500-3f2c9a1b","#,
        r#""dimensions":1024,"limits":{"context_tokens":8192,"#,
        r#""output_tokens":null},"suite_results":[]}"#,
    )
    .as_bytes();
    let (path, store) = scratch_store();
    let digest = store.put(bytes).unwrap();
    let loaded = ModelCard::load(&store, &digest).unwrap();
    assert_eq!(loaded.digest(), &Digest::of(bytes));
    assert!(loaded.identity().is_none());
    assert_eq!(loaded.fields().dimensions, NonZeroUsize::new(1024));
    fs::remove_dir_all(path).unwrap();
}
