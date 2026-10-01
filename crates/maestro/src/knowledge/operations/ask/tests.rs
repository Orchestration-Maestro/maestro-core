use super::{answer_failure, registered_answerer};
use crate::{
    kernel::Kernel,
    knowledge::operations::{KnowledgeError, tests::Scratch},
};
use maestro_kernel::{
    gateway::{
        CardIdentity, Error as GatewayError, Limits, ModelCard, Role, RouterEntry,
        card_v2::{
            Backend, CachePolicy, Capability, CardFormats, ControlValue, Dimensions,
            EmbeddingFormat, FlagValue, HardwareIdentity, KvCache, KvCacheType, MemoryEstimate,
            Observation, OffloadMode, OffloadPolicy, Provenance, QualificationMethod,
            QualifiedLimits, Quantization, Resources, RuntimeLimits, Sampling, SamplingParameters,
            Template, TokenizerDerivation, WeightIdentity,
        },
    },
    generation::Generation,
    model::{
        EvaluationDisposition, EvaluationMode, NewModelCard, NewModelEvaluation, NewModelSelection,
    },
    store::Error as StoreError,
};
use maestro_knowledge::{
    answer::{AskBudget, AskError, AskRequest, DEFAULT_MODEL},
    search::{SearchError, evidence::EvidenceError},
};
use std::{
    collections::BTreeMap,
    io,
    num::{NonZeroU32, NonZeroU64},
    path::PathBuf,
};

#[test]
fn unregistered_default_answerer_is_not_resolved() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    register_answerer(&kernel, "qwen3-4b", b"registered Qwen answerer");
    let request = AskRequest {
        collection: "collection".to_owned(),
        question: "How can I configure the service?".to_owned(),
        model: DEFAULT_MODEL.to_owned(),
        version: None,
        budget: AskBudget::default(),
    };

    assert!(
        registered_answerer(&kernel, &kernel.scopes, &request)
            .expect("read scoped card registry")
            .is_none()
    );
}

#[test]
fn i7_latest_matching_card_is_selected_and_other_entries_are_ignored() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let request = AskRequest {
        collection: "collection".to_owned(),
        question: "How can I configure the service?".to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    };

    register_answerer(&kernel, "other-model", b"other model");
    assert!(
        registered_answerer(&kernel, &kernel.scopes, &request)
            .expect("read scoped card registry")
            .is_none()
    );

    let first = register_answerer(&kernel, "qwen3-4b", b"first answerer");
    assert_eq!(
        registered_answerer(&kernel, &kernel.scopes, &request)
            .expect("read scoped card registry")
            .expect("one answerer")
            .id,
        first
    );
    let latest = register_answerer(&kernel, "qwen3-4b", b"latest answerer");
    assert_eq!(
        registered_answerer(&kernel, &kernel.scopes, &request)
            .expect("read scoped card registry")
            .expect("latest answerer")
            .id,
        latest
    );
}

/// An ask of the default router entry in `collection`.
fn default_request() -> AskRequest {
    AskRequest {
        collection: "collection".to_owned(),
        question: "How can I configure the service?".to_owned(),
        model: "ask-gemma4-e4b-nonthinking".to_owned(),
        version: None,
        budget: AskBudget::default(),
    }
}

#[test]
fn a_thinking_card_registered_later_is_the_default() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let request = default_request();
    register_reasoning_answerer_for_entry(
        &kernel,
        "ask-gemma4-e4b-nonthinking",
        b"plain answerer",
        false,
    );
    register_answerer(&kernel, "other-model", b"later different alias");

    let (latest_thinking, _) = register_reasoning_answerer_for_entry(
        &kernel,
        "ask-gemma4-e4b-nonthinking",
        b"thinking answerer",
        true,
    );

    let resolved = registered_answerer(&kernel, &kernel.scopes, &request)
        .expect("read scoped card registry")
        .expect("the selected thinking answerer");
    assert_eq!(resolved.id, latest_thinking);
}

#[test]
fn a_plain_card_registered_after_a_thinking_one_is_the_default() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let request = default_request();
    register_reasoning_answerer_for_entry(
        &kernel,
        "ask-gemma4-e4b-nonthinking",
        b"thinking answerer",
        true,
    );

    let (plain, _) = register_reasoning_answerer_for_entry(
        &kernel,
        "ask-gemma4-e4b-nonthinking",
        b"plain answerer",
        false,
    );

    let resolved = registered_answerer(&kernel, &kernel.scopes, &request)
        .expect("read scoped card registry")
        .expect("the plain answerer");
    assert_eq!(resolved.id, plain);
}

#[test]
fn i4_answerer_and_evidence_failures_keep_their_public_codes() {
    for error in [
        AskError::TimedOut,
        AskError::Backend(GatewayError::Unavailable {
            reason: "private router detail".to_owned(),
        }),
        AskError::Backend(GatewayError::Refused {
            status: 503,
            code: Some("insufficient_room".to_owned()),
            message: "private refusal detail".to_owned(),
        }),
    ] {
        assert_eq!(
            answer_failure(&error),
            KnowledgeError::Refused {
                code: "answerer_unavailable",
                message: "the answerer is unavailable",
            }
        );
    }

    assert_eq!(
        answer_failure(&AskError::Search(SearchError::PermissionsChanged)),
        KnowledgeError::Refused {
            code: "access_changed",
            message: "permissions changed during the request; no result was delivered",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Evidence(EvidenceError::PermissionsChanged)),
        KnowledgeError::Refused {
            code: "access_changed",
            message: "permissions changed during the request; no result was delivered",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Evidence(EvidenceError::NotVisible)),
        KnowledgeError::Refused {
            code: "not_found",
            message: "collection is unknown or has no published generation",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Evidence(EvidenceError::Integrity(
            "private integrity detail".to_owned(),
        ))),
        KnowledgeError::Failed {
            code: "integrity_error",
            message: "the source integrity check failed",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Evidence(EvidenceError::TimedOut)),
        KnowledgeError::Failed {
            code: "deadline_exceeded",
            message: "search exceeded its accepted deadline",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Search(SearchError::AdmissionTimedOut)),
        KnowledgeError::Failed {
            code: "deadline_exceeded",
            message: "search exceeded its accepted deadline",
        }
    );
    assert_eq!(
        answer_failure(&AskError::Evidence(EvidenceError::Store(StoreError::Io {
            path: PathBuf::new(),
            source: io::Error::other("private store detail"),
        }))),
        KnowledgeError::Failed {
            code: "kernel_unavailable",
            message: "the local knowledge store is unavailable",
        }
    );
}

/// Registers a reranker card in the scratch collection, evaluates it on
/// `generation` and selects it; returns the card.
pub(crate) fn select_reranker(kernel: &Kernel, generation: &Generation) -> ModelCard {
    let (id, card) = register_card(kernel, "collection", Role::Reranker, "rerank", b"reranker");
    select_card(kernel, generation, Role::Reranker, &id);
    card
}

/// Evaluates the registered card `id` of `role` on `generation` and
/// selects it for `role` in the scratch collection.
pub(crate) fn select_card(kernel: &Kernel, generation: &Generation, role: Role, id: &str) {
    let scopes = &kernel.scopes;
    let card_id = id.parse().expect("registration ULID");
    let evaluation = kernel
        .database
        .record_model_evaluation(
            scopes,
            &NewModelEvaluation {
                run_id: "run",
                collection_id: "collection",
                card_id,
                role,
                mode: EvaluationMode::Real,
                generation_id: Some(generation.id),
                disposition: EvaluationDisposition::Eligible,
                manifest: b"manifest",
                report: b"report",
            },
        )
        .expect("record evaluation");
    kernel
        .database
        .record_model_selection(
            scopes,
            &NewModelSelection {
                collection_id: "collection",
                role,
                card_id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "approved",
            },
        )
        .expect("select the card");
}

/// Registers a small v2 answerer card and returns its immutable registry ID.
fn register_answerer(kernel: &Kernel, entry: &str, weights: &[u8]) -> String {
    let (id, _) = register_card(kernel, "collection", Role::Answerer, entry, weights);
    id
}

/// Registers a small v2 card of `role` in `collection` and returns its
/// immutable registry ID and the card.
pub(crate) fn register_card(
    kernel: &Kernel,
    collection: &str,
    role: Role,
    entry: &str,
    weights: &[u8],
) -> (String, ModelCard) {
    register_identity(
        kernel,
        collection,
        &card_identity(kernel, role, entry, weights),
    )
}

/// Registers a small v2 answerer card in `collection` whose reasoning
/// controls set `enable_thinking` to `thinking`, and returns its registry ID
/// and the card.
pub(crate) fn register_reasoning_answerer(
    kernel: &Kernel,
    weights: &[u8],
    thinking: bool,
) -> (String, ModelCard) {
    register_reasoning_answerer_for_entry(kernel, "qwen3-4b", weights, thinking)
}

fn register_reasoning_answerer_for_entry(
    kernel: &Kernel,
    entry: &str,
    weights: &[u8],
    thinking: bool,
) -> (String, ModelCard) {
    let mut identity = card_identity(kernel, Role::Answerer, entry, weights);
    identity.invocation.reasoning = Capability::Supported(BTreeMap::from([(
        "enable_thinking".to_owned(),
        ControlValue::Boolean(thinking),
    )]));
    register_identity(kernel, "collection", &identity)
}

/// Records `identity` as a v2 card in `collection` and returns its registry
/// ID and the card.
fn register_identity(
    kernel: &Kernel,
    collection: &str,
    identity: &CardIdentity,
) -> (String, ModelCard) {
    let card = ModelCard::record_v2(&kernel.artifacts, identity).expect("record v2 card");
    let id = kernel
        .database
        .record_model_card(
            &kernel.scopes,
            &NewModelCard {
                collection_id: collection,
                card: &card,
            },
        )
        .expect("register v2 card")
        .id
        .to_string();
    (id, card)
}

/// A small v2 identity of `role` whose weights are `weights`, stored in the
/// kernel.
#[expect(
    clippy::too_many_lines,
    reason = "the fixture needs one complete v2 identity"
)]
fn card_identity(kernel: &Kernel, role: Role, entry: &str, weights: &[u8]) -> CardIdentity {
    let answerer = role == Role::Answerer;
    let weight_digest = kernel
        .database
        .put(weights, "application/octet-stream")
        .expect("store model weights");
    let runtime_digest = kernel
        .database
        .put(b"runtime executable", "application/octet-stream")
        .expect("store runtime binary");
    let qualification_digest = kernel
        .database
        .put(b"qualification", "application/json")
        .expect("store qualification evidence");
    CardIdentity {
        role,
        router_entry: RouterEntry::parse(entry).expect("router entry"),
        weights: WeightIdentity {
            upstream_model_id: "test/answerer".to_owned(),
            upstream_revision: "0123456789abcdef".to_owned(),
            source_url: "https://example.invalid/model".to_owned(),
            licence_id: "apache-2.0".to_owned(),
            licence_terms_source: "https://example.invalid/license".to_owned(),
            gguf_digest: weight_digest.clone(),
            gguf_bytes: NonZeroU64::new(weights.len() as u64).expect("nonzero weights"),
            quantization: Quantization::Q8_0,
            adapters: BTreeMap::new(),
            drafts: BTreeMap::new(),
            projectors: BTreeMap::new(),
        },
        formats: CardFormats {
            tokenizer_digest: weight_digest.clone(),
            tokenizer_derivation: TokenizerDerivation::GgufEmbedded,
            qualification_digest: qualification_digest.clone(),
            template: Template::Absent,
            system_format: String::new(),
            tool_format: Capability::Unsupported,
            document: Capability::NotApplicable,
            query: Capability::NotApplicable,
            embedding: EmbeddingFormat::NotApplicable,
        },
        invocation: RuntimeLimits {
            limits: Limits {
                context_tokens: NonZeroU32::new(4096).expect("nonzero context"),
                output_tokens: answerer.then(|| NonZeroU32::new(1024).expect("nonzero output")),
            },
            dimensions: Dimensions::NotApplicable,
            sampling: if answerer {
                Sampling::Configured(SamplingParameters {
                    temperature: 0.1,
                    top_p: 0.9,
                    top_k: 40,
                    min_p: 0.0,
                    typical_p: 1.0,
                    repeat_penalty: 1.0,
                    frequency_penalty: 0.0,
                    presence_penalty: 0.0,
                    seed: Some(1),
                })
            } else {
                Sampling::NotApplicable
            },
            reasoning: if answerer {
                Capability::Unsupported
            } else {
                Capability::NotApplicable
            },
            llama_cpp_build: "b1234-abcdef".to_owned(),
            runtime_binary_digest: runtime_digest,
            backend: Backend::Cuda,
            server_flags: BTreeMap::from([
                (
                    "--model".to_owned(),
                    FlagValue::Asset {
                        name: "weights".to_owned(),
                        digest: weight_digest,
                    },
                ),
                ("--temperature".to_owned(), FlagValue::Number(0.1)),
            ]),
        },
        resources: Resources {
            offload: OffloadPolicy {
                mode: OffloadMode::ExplicitDevices,
                devices: vec!["gpu0".to_owned()],
            },
            memory_estimate: MemoryEstimate {
                bytes: NonZeroU64::new(2_000_000_000).expect("nonzero memory estimate"),
                source: "test estimate".to_owned(),
            },
            slots: NonZeroU32::new(1).expect("nonzero slots"),
            kv_cache: KvCache {
                key: KvCacheType::Q8_0,
                value: KvCacheType::Q8_0,
            },
            cache_policy: CachePolicy::KeepLoaded,
            batch_size: NonZeroU32::new(512).expect("nonzero batch"),
            micro_batch_size: NonZeroU32::new(128).expect("nonzero micro-batch"),
            hardware: HardwareIdentity {
                device: "test GPU".to_owned(),
                driver_version: "test driver".to_owned(),
                host_cpu: "test CPU".to_owned(),
                host_memory_bytes: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
            },
            qualified_limits: QualifiedLimits {
                context_tokens: Observation::Measured {
                    value: NonZeroU32::new(4096).expect("nonzero context"),
                    provenance: "test qualification".to_owned(),
                },
                output_tokens: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
                concurrency: Observation::Measured {
                    value: NonZeroU32::new(1).expect("nonzero concurrency"),
                    provenance: "test qualification".to_owned(),
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
            tool_versions: BTreeMap::from([("llama.cpp".to_owned(), "b1234".to_owned())]),
            artifacts: BTreeMap::from([("native-qualification".to_owned(), qualification_digest)]),
        },
    }
}
