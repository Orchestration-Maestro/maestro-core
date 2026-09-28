//! `knowledge prepare` qualifies a v2 embedder card against the tokenizer
//! qualification artifact the card names, and a v1 card as before.

use super::{
    knowledge_publish::{StubRouter, card, ids, input},
    support::{Home, Running},
};
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardIdentity, Limits, ModelCard, Role, RouterEntry,
        card_v2::{
            Backend, CachePolicy, Capability, CardFormats, Dimensions, EmbeddingFormat, FlagValue,
            HardwareIdentity, KvCache, KvCacheType, MemoryEstimate, Normalization, Observation,
            OffloadMode, OffloadPolicy, Pooling, Provenance, QualificationMethod, QualifiedLimits,
            Quantization, Resources, RuntimeLimits, Sampling, Template, TextFormat,
            TokenizerDerivation, WeightIdentity,
        },
    },
};
use serde_json::{Value, json};
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

/// The llama.cpp build the stub router reports for the entry `embed`.
const BUILD: &str = "test-build";

/// A qualification artifact for the weights `model`: the committed parity
/// fixtures, whose IDs the stub router answers.
fn qualification(model: &Digest) -> Vec<u8> {
    const PARITY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-knowledge/src/prepare/native-parity.json"
    ));
    let parity: Value = serde_json::from_str(PARITY).unwrap();
    let fixtures: Vec<Value> = parity["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fixture| {
            let ids = ids(&fixture["ids"]);
            json!({
                "name": fixture["name"],
                "input": input(&fixture["input"]),
                "ids": ids,
                "confirmatory_ids": ids,
                "canary": fixture["canary"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    serde_json::to_vec(&json!({
        "schema": "maestro-tokenizer-qualification/1",
        "mode": "synthetic",
        "model_digest": model,
        "tokenizer_digest": model,
        "llama_cpp_build": BUILD,
        "native_tool": {"name": "native-tool", "version": "1.0"},
        "library": {"name": "tokenizer-library", "version": "1.0"},
        "created": "2026-09-28",
        "add_special": true,
        "parse_special": true,
        "vocabulary": {"minimum_id": 0, "maximum_id": 250_001},
        "fixtures": fixtures,
    }))
    .unwrap()
}

/// Records in `home` a v2 embedder card served as the router's `embed`,
/// whose weights are `model` and whose qualification digest is `qualification`.
fn v2_card(home: &Home, model: &Digest, qualification: Digest) -> Digest {
    let store = Store::new(home.data().join("artifacts"));
    let card = ModelCard::record_v2(&store, &identity(model, qualification)).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    let recorded = home.database().put(&bytes, "application/json").unwrap();
    assert_eq!(&recorded, card.digest());
    recorded
}

/// A valid v2 embedder identity for the weights `model`.
fn identity(model: &Digest, qualification_digest: Digest) -> CardIdentity {
    CardIdentity {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        weights: WeightIdentity {
            upstream_model_id: "test/model".to_owned(),
            upstream_revision: "revision-1".to_owned(),
            source_url: "https://example.invalid/model".to_owned(),
            licence_id: "apache-2.0".to_owned(),
            licence_terms_source: "https://example.invalid/license".to_owned(),
            gguf_digest: model.clone(),
            gguf_bytes: NonZeroU64::new(1024).unwrap(),
            quantization: Quantization::Q8_0,
            adapters: [].into(),
            drafts: [].into(),
            projectors: [].into(),
        },
        formats: CardFormats {
            tokenizer_digest: model.clone(),
            tokenizer_derivation: TokenizerDerivation::GgufEmbedded,
            qualification_digest,
            template: Template::Absent,
            system_format: String::new(),
            tool_format: Capability::NotApplicable,
            document: Capability::Supported(TextFormat {
                prefix: String::new(),
                suffix: String::new(),
            }),
            query: Capability::Supported(TextFormat {
                prefix: String::new(),
                suffix: String::new(),
            }),
            embedding: EmbeddingFormat::Supported {
                pooling: Pooling::Cls,
                normalization: Normalization::L2,
            },
        },
        invocation: RuntimeLimits {
            limits: Limits {
                context_tokens: NonZeroU32::new(8192).unwrap(),
                output_tokens: None,
            },
            dimensions: Dimensions::Measured(NonZeroUsize::new(3).unwrap()),
            sampling: Sampling::NotApplicable,
            reasoning: Capability::NotApplicable,
            llama_cpp_build: BUILD.to_owned(),
            runtime_binary_digest: Digest::of(b"runtime"),
            backend: Backend::Cpu,
            server_flags: [(
                "--model".to_owned(),
                FlagValue::Asset {
                    name: "weights".to_owned(),
                    digest: model.clone(),
                },
            )]
            .into(),
        },
        resources: resources(),
        provenance: Provenance {
            identity_created: "2026-09-28".to_owned(),
            qualification_created: "2026-09-28".to_owned(),
            qualification_method: QualificationMethod::NativeTokenizer,
            tool_versions: [
                ("native-tool".to_owned(), "1.0".to_owned()),
                ("tokenizer-library".to_owned(), "1.0".to_owned()),
            ]
            .into(),
            artifacts: [].into(),
        },
    }
}

/// The synthetic card's CPU-only resources.
fn resources() -> Resources {
    let measured = |value| Observation::Measured {
        value: NonZeroU32::new(value).unwrap(),
        provenance: "synthetic fixture".to_owned(),
    };
    Resources {
        offload: OffloadPolicy {
            mode: OffloadMode::CpuOnly,
            devices: Vec::new(),
        },
        memory_estimate: MemoryEstimate {
            bytes: NonZeroU64::new(1024).unwrap(),
            source: "synthetic fixture".to_owned(),
        },
        slots: NonZeroU32::new(1).unwrap(),
        kv_cache: KvCache {
            key: KvCacheType::F16,
            value: KvCacheType::F16,
        },
        cache_policy: CachePolicy::KeepLoaded,
        batch_size: NonZeroU32::new(64).unwrap(),
        micro_batch_size: NonZeroU32::new(16).unwrap(),
        hardware: HardwareIdentity {
            device: "synthetic".to_owned(),
            driver_version: "n/a".to_owned(),
            host_cpu: "synthetic".to_owned(),
            host_memory_bytes: unavailable(),
        },
        qualified_limits: QualifiedLimits {
            context_tokens: measured(8192),
            output_tokens: unavailable(),
            concurrency: measured(1),
            peak_memory_bytes: unavailable(),
        },
    }
}

/// An observation the synthetic card did not measure.
fn unavailable<T>() -> Observation<T> {
    Observation::Unavailable {
        reason: "synthetic fixture".to_owned(),
    }
}

/// Runs `knowledge prepare` of `synthetic` with `card` against `router`, and
/// returns its exit code and JSON document.
fn prepare(home: &Home, card: &Digest, router: &StubRouter) -> (Option<i32>, Value) {
    let mut command = home.command(&[
        "knowledge",
        "prepare",
        "--collection",
        "synthetic",
        "--card",
        card.as_str(),
        "--json",
    ]);
    command.env("MAESTRO_ROUTER_URL", router.url());
    let result = Running::of(command).finish();
    assert!(result.stderr.contains("job "), "{result:?}");
    (result.code, serde_json::from_str(&result.stdout).unwrap())
}

/// Asserts that `document` is a failed prepare job whose error names `reason`.
fn assert_refused(code: Option<i32>, document: &Value, reason: &str) {
    assert_eq!(code, Some(1), "{document}");
    assert_eq!(document["state"], "failed", "{document}");
    let error = document["outcome"]["error"].as_str().unwrap();
    assert!(error.contains(reason), "{error}");
}

#[test]
fn prepare_qualifies_a_v2_card_against_its_recorded_qualification() {
    let home = Home::new();
    home.add_synthetic();
    let model = Digest::of(b"model");
    let evidence = home
        .database()
        .put(&qualification(&model), "application/json")
        .unwrap();
    let card = v2_card(&home, &model, evidence);
    let router = StubRouter::serve();

    let (code, document) = prepare(&home, &card, &router);
    assert_eq!(code, Some(0), "{document}");
    assert_eq!(document["schema"], "maestro-cli/knowledge-prepare/1");
    assert_eq!(document["state"], "succeeded", "{document}");
}

#[test]
fn prepare_refuses_a_v2_card_whose_qualification_is_not_recorded() {
    let home = Home::new();
    home.add_synthetic();
    let model = Digest::of(b"model");
    let unrecorded = Digest::of(&qualification(&model));
    let card = v2_card(&home, &model, unrecorded.clone());
    let router = StubRouter::serve();

    let (code, document) = prepare(&home, &card, &router);
    assert_refused(code, &document, unrecorded.as_str());
}

#[test]
fn prepare_refuses_a_v2_card_whose_qualification_names_other_weights() {
    let home = Home::new();
    home.add_synthetic();
    let model = Digest::of(b"model");
    let evidence = home
        .database()
        .put(
            &qualification(&Digest::of(b"other model")),
            "application/json",
        )
        .unwrap();
    let card = v2_card(&home, &model, evidence);
    let router = StubRouter::serve();

    let (code, document) = prepare(&home, &card, &router);
    assert_refused(
        code,
        &document,
        "qualification weight digest does not match the v2 card",
    );
}

#[test]
fn prepare_qualifies_a_v1_card_by_the_committed_fixtures_without_an_artifact() {
    let home = Home::new();
    home.add_synthetic();
    let card = card(&home, Role::Embedder);
    let router = StubRouter::serve();

    let (code, document) = prepare(&home, &card, &router);
    assert_eq!(code, Some(0), "{document}");
    assert_eq!(document["state"], "succeeded", "{document}");
}
