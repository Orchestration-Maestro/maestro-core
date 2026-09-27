//! What the router tokenizer's tests share: model cards, recorded in a
//! scratch store that is gone once they are made, and a deadline a test can
//! wait out.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, CardIdentity, Limits, ModelCard, Role, RouterEntry,
        card_v2::{
            Backend, CachePolicy, Capability, CardFormats, Dimensions, EmbeddingFormat, FlagValue,
            HardwareIdentity, KvCache, KvCacheType, MemoryEstimate, Normalization, Observation,
            OffloadMode, OffloadPolicy, Pooling, Provenance, QualificationMethod, QualifiedLimits,
            Quantization, Resources, RuntimeLimits, Sampling, Template, TextFormat,
            TokenizerDerivation, WeightIdentity,
        },
    },
};
use std::{
    collections::BTreeMap,
    env, fs,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

/// The llama.cpp build the stub router's embedder reports.
pub(crate) const BUILD: &str = "b6500-3f2c9a1b";
/// The file digest the embedder's cards record, SHA-256 of `model file`.
pub(crate) const MODEL_FILE: &str =
    "ca56847038f3f329524caec5a86e14865f49918d68094c90dd021a5e67b927f6";
/// Another model file's digest, SHA-256 of `another model file`.
pub(crate) const OTHER_FILE: &str =
    "dfb5dcc5de5d764a869a534c112278852326fa24ba93e2c13dac6886876c3fd2";
/// A deadline short enough for a test to wait out, and long enough for the
/// goldens port, which answers at once, to answer every call within it.
pub(super) const SHORT_DEADLINE: Duration = Duration::from_millis(500);

/// The digest `hex` names.
pub(crate) fn digest(hex: &str) -> Digest {
    Digest::parse(hex).unwrap()
}

/// The card of a model filling `role` as the router's entry `embed`, from
/// the file `file_digest` names, served by the llama.cpp build
/// `server_build`.
pub(super) fn card(role: Role, file_digest: &str, server_build: &str) -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let fields = CardFields {
        role,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: digest(file_digest),
        template_digest: None,
        server_build: server_build.to_owned(),
        dimensions: (role == Role::Embedder).then(|| NonZeroUsize::new(1024).unwrap()),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let root = env::temp_dir().join(format!(
        "maestro-knowledge-prepare-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
}

/// The embedder's card the stub router serves: [`MODEL_FILE`] under
/// [`BUILD`].
pub(super) fn embedder() -> ModelCard {
    card(Role::Embedder, MODEL_FILE, BUILD)
}

/// A valid v2 embedder identity bound to one qualification artifact.
pub(crate) fn identity(qualification_digest: Digest) -> CardIdentity {
    CardIdentity {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        weights: WeightIdentity {
            upstream_model_id: "test/model".to_owned(),
            upstream_revision: "revision-1".to_owned(),
            source_url: "https://example.invalid/model".to_owned(),
            licence_id: "apache-2.0".to_owned(),
            licence_terms_source: "https://example.invalid/license".to_owned(),
            gguf_digest: digest(MODEL_FILE),
            gguf_bytes: NonZeroU64::new(1024).unwrap(),
            quantization: Quantization::Q8_0,
            adapters: BTreeMap::default(),
            drafts: BTreeMap::default(),
            projectors: BTreeMap::default(),
        },
        formats: formats(qualification_digest),
        invocation: RuntimeLimits {
            limits: Limits {
                context_tokens: NonZeroU32::new(8192).unwrap(),
                output_tokens: None,
            },
            dimensions: Dimensions::Measured(NonZeroUsize::new(1024).unwrap()),
            sampling: Sampling::NotApplicable,
            reasoning: Capability::NotApplicable,
            llama_cpp_build: BUILD.to_owned(),
            runtime_binary_digest: digest(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            ),
            backend: Backend::Cpu,
            server_flags: [(
                "--model".to_owned(),
                FlagValue::Asset {
                    name: "weights".to_owned(),
                    digest: digest(MODEL_FILE),
                },
            )]
            .into(),
        },
        resources: Resources {
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
                host_memory_bytes: Observation::Unavailable {
                    reason: "synthetic fixture".to_owned(),
                },
            },
            qualified_limits: QualifiedLimits {
                context_tokens: Observation::Measured {
                    value: NonZeroU32::new(8192).unwrap(),
                    provenance: "synthetic fixture".to_owned(),
                },
                output_tokens: Observation::Unavailable {
                    reason: "not applicable".to_owned(),
                },
                concurrency: Observation::Measured {
                    value: NonZeroU32::new(1).unwrap(),
                    provenance: "synthetic fixture".to_owned(),
                },
                peak_memory_bytes: Observation::Unavailable {
                    reason: "synthetic fixture".to_owned(),
                },
            },
        },
        provenance: Provenance {
            identity_created: "2026-09-27".to_owned(),
            qualification_created: "2026-09-27".to_owned(),
            qualification_method: QualificationMethod::NativeTokenizer,
            tool_versions: [
                ("llama.cpp".to_owned(), BUILD.to_owned()),
                ("native-tool".to_owned(), "1.0".to_owned()),
                ("tokenizer-library".to_owned(), "1.0".to_owned()),
            ]
            .into(),
            artifacts: BTreeMap::default(),
        },
    }
}

/// Formats a synthetic document/query profile pinned to qualification evidence.
fn formats(qualification_digest: Digest) -> CardFormats {
    CardFormats {
        tokenizer_digest: digest(MODEL_FILE),
        tokenizer_derivation: TokenizerDerivation::GgufEmbedded,
        qualification_digest,
        template: Template::Absent,
        system_format: String::new(),
        tool_format: Capability::NotApplicable,
        document: Capability::Supported(TextFormat {
            prefix: "doc: ".to_owned(),
            suffix: String::new(),
        }),
        query: Capability::Supported(TextFormat {
            prefix: concat!(
                "Instruct: Given a web search query, retrieve relevant passages that ",
                "answer the query\nQuery: "
            )
            .to_owned(),
            suffix: String::new(),
        }),
        embedding: EmbeddingFormat::Supported {
            pooling: Pooling::LastToken,
            normalization: Normalization::L2,
        },
    }
}
