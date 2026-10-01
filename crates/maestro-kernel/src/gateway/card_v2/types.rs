//! Strict typed identity recorded by `maestro-model-card/2`.

use super::super::card_types::{Limits, Role, RouterEntry};
use crate::artifact::Digest;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::BTreeMap,
    fmt,
    marker::PhantomData,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
};

/// Schema identifier for new content-addressed model identities.
pub(crate) const CARD_SCHEMA_V2: &str = "maestro-model-card/2";

/// Strict v2 artifact envelope stored under the card digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CardV2Json {
    /// Card schema version.
    pub(crate) schema: String,
    /// Full immutable model/runtime identity.
    pub(crate) identity: CardIdentity,
}

/// The full immutable identity of one model/runtime configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardIdentity {
    /// Role whose gateway operations this immutable identity configures.
    pub role: Role,
    /// Router catalog alias included in the canonical identity digest.
    pub router_entry: RouterEntry,
    /// Upstream weights, quantization, and optional companion identities.
    pub weights: WeightIdentity,
    /// Tokenizer, template, and literal input-format identities.
    pub formats: CardFormats,
    /// Effective runtime invocation configuration.
    pub invocation: RuntimeLimits,
    /// Resource policy, reference hardware, and qualification observations.
    pub resources: Resources,
    /// Creation dates, qualification method, tool versions, and evidence.
    pub provenance: Provenance,
}

/// Weight and upstream facts; maps use stable, sorted names rather than paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightIdentity {
    /// Stable upstream model identifier.
    pub upstream_model_id: String,
    /// Exact upstream revision used for this file.
    pub upstream_revision: String,
    /// Publisher URL for the model source.
    pub source_url: String,
    /// Licence identifier attached to these weights.
    pub licence_id: String,
    /// HTTP(S) URL of the exact licence terms.
    pub licence_terms_source: String,
    /// SHA-256 of the complete GGUF file.
    pub gguf_digest: Digest,
    /// Exact GGUF byte count.
    pub gguf_bytes: NonZeroU64,
    /// Typed GGUF quantization format.
    pub quantization: Quantization,
    /// Named adapter identities and their content digests; empty when unused.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub adapters: BTreeMap<String, Digest>,
    /// Named draft-model identities and their content digests; empty when unused.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub drafts: BTreeMap<String, Digest>,
    /// Named projector identities and their content digests; empty when unused.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub projectors: BTreeMap<String, Digest>,
}

/// Supported GGUF quantization identities; unknown variants are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantization {
    /// Full 32-bit floating-point weights.
    #[serde(rename = "F32")]
    F32,
    /// Full 16-bit floating-point weights.
    #[serde(rename = "F16")]
    F16,
    /// Brain floating-point 16-bit weights.
    #[serde(rename = "BF16")]
    Bf16,
    /// Eight-bit block quantization.
    #[serde(rename = "Q8_0")]
    Q8_0,
    /// Six-bit K-quantization.
    #[serde(rename = "Q6_K")]
    Q6K,
    /// Small five-bit K-quantization.
    #[serde(rename = "Q5_K_S")]
    Q5KSmall,
    /// Medium five-bit K-quantization.
    #[serde(rename = "Q5_K_M")]
    Q5KMedium,
    /// Five-bit legacy quantization.
    #[serde(rename = "Q5_0")]
    Q5_0,
    /// Five-bit legacy quantization variant.
    #[serde(rename = "Q5_1")]
    Q5_1,
    /// Small four-bit K-quantization.
    #[serde(rename = "Q4_K_S")]
    Q4KSmall,
    /// Medium four-bit K-quantization.
    #[serde(rename = "Q4_K_M")]
    Q4KMedium,
    /// Four-bit legacy quantization.
    #[serde(rename = "Q4_0")]
    Q4_0,
    /// Four-bit legacy quantization variant.
    #[serde(rename = "Q4_1")]
    Q4_1,
    /// Small three-bit K-quantization.
    #[serde(rename = "Q3_K_S")]
    Q3KSmall,
    /// Medium three-bit K-quantization.
    #[serde(rename = "Q3_K_M")]
    Q3KMedium,
    /// Large three-bit K-quantization.
    #[serde(rename = "Q3_K_L")]
    Q3KLarge,
    /// Two-bit K-quantization.
    #[serde(rename = "Q2_K")]
    Q2K,
    /// One-bit IQ quantization.
    #[serde(rename = "IQ1_S")]
    Iq1S,
    /// Extra-extra-small two-bit IQ quantization.
    #[serde(rename = "IQ2_XXS")]
    Iq2Xxs,
    /// Extra-small two-bit IQ quantization.
    #[serde(rename = "IQ2_XS")]
    Iq2Xs,
    /// Extra-extra-small three-bit IQ quantization.
    #[serde(rename = "IQ3_XXS")]
    Iq3Xxs,
    /// Three-bit IQ quantization.
    #[serde(rename = "IQ3_S")]
    Iq3S,
    /// Nonlinear four-bit IQ quantization.
    #[serde(rename = "IQ4_NL")]
    Iq4Nl,
    /// Extra-small four-bit IQ quantization.
    #[serde(rename = "IQ4_XS")]
    Iq4Xs,
}

/// Exact tokenization, chat, and embedding input-format facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardFormats {
    /// Identity digest for the tokenizer bytes or conservative GGUF identity.
    pub tokenizer_digest: Digest,
    /// How the tokenizer identity was derived.
    pub tokenizer_derivation: TokenizerDerivation,
    /// Native qualification artifact digest, independent of this card digest.
    pub qualification_digest: Digest,
    /// Exact chat-template digest or an explicit absence.
    pub template: Template,
    /// Exact system-message format, including an empty format.
    pub system_format: String,
    /// Tool format or its explicit unsupported/inapplicable state.
    pub tool_format: Capability<String>,
    /// Literal document prefix and suffix, or an explicit unsupported state.
    pub document: Capability<TextFormat>,
    /// Literal query prefix and suffix, or an explicit unsupported state.
    pub query: Capability<TextFormat>,
    /// Pooling and normalization for embeddings, or an explicit absence.
    pub embedding: EmbeddingFormat,
}

/// Provenance method for deriving tokenizer identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenizerDerivation {
    /// Tokenizer is embedded; digest conservatively equals the whole GGUF digest.
    GgufEmbedded,
    /// Tokenizer bytes are an external content-addressed artifact.
    ExternalArtifact,
    /// Identity was produced by an approved native tokenizer tool.
    NativeProduced,
}

/// Whether a model has a chat template, explicit rather than inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "digest",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Template {
    /// The model has no chat template.
    Absent,
    /// SHA-256 of the exact template bytes.
    Digest(Digest),
}

/// A literal byte prefix and suffix; no general template engine is implied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextFormat {
    /// Literal bytes prepended before the input.
    pub prefix: String,
    /// Literal bytes appended after the input.
    pub suffix: String,
}

/// A capability's supported, unsupported, or inapplicable state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Capability<T> {
    /// Capability is supported with this exact value or configuration.
    Supported(T),
    /// The runtime does not support this capability.
    Unsupported,
    /// This capability does not apply to the model role.
    NotApplicable,
}

/// Token-vector pooling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pooling {
    /// Use the CLS token vector.
    Cls,
    /// Mean-pool token vectors.
    Mean,
    /// Use the final token vector.
    LastToken,
}

/// Embedding vector normalization policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalization {
    /// Return unnormalized vectors.
    None,
    /// Apply L1 normalization.
    L1,
    /// Apply L2 normalization.
    L2,
}

/// Embedder-specific pooling and normalization, explicit when inapplicable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EmbeddingFormat {
    /// Embedder pooling and normalization contract.
    Supported {
        /// Pooling method applied to token outputs.
        pooling: Pooling,
        /// Vector normalization applied after pooling.
        normalization: Normalization,
    },
    /// Embedding operations are unsupported.
    Unsupported,
    /// The role does not produce embeddings.
    NotApplicable,
}

/// Effective invocation identity, including resolved inherited flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLimits {
    /// Context and output token limits.
    pub limits: Limits,
    /// Measured embedding dimensions or explicit non-applicability.
    pub dimensions: Dimensions,
    /// Effective sampling settings or explicit non-applicability.
    pub sampling: Sampling,
    /// Supported reasoning controls or an explicit unsupported state.
    #[serde(deserialize_with = "deserialize_reasoning")]
    pub reasoning: Capability<BTreeMap<String, ControlValue>>,
    /// Exact llama.cpp build identifier.
    pub llama_cpp_build: String,
    /// SHA-256 of the runtime executable.
    pub runtime_binary_digest: Digest,
    /// Typed serving backend.
    pub backend: Backend,
    /// Fully resolved server flags; asset paths use named digests.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub server_flags: BTreeMap<String, FlagValue>,
}

/// Measured embedding dimensions or explicit role-based absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Dimensions {
    /// Measured output dimension for an embedder.
    Measured(NonZeroUsize),
    /// The role does not return embedding vectors.
    NotApplicable,
}

/// Supported llama.cpp runtime backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// CPU backend.
    Cpu,
    /// CUDA backend.
    Cuda,
    /// Vulkan backend.
    Vulkan,
    /// Metal backend.
    Metal,
    /// `ROCm` backend.
    Rocm,
}

/// Configured generation sampling or explicit role-based absence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Sampling {
    /// Exact sampling parameters for generation.
    Configured(SamplingParameters),
    /// The role does not sample generated tokens.
    NotApplicable,
}

/// Sampling and penalty values recorded exactly as configured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingParameters {
    /// Sampling temperature.
    pub temperature: f64,
    /// Nucleus sampling probability limit.
    pub top_p: f64,
    /// Top-k sampling limit; zero records disabled.
    pub top_k: u32,
    /// Minimum sampling probability.
    pub min_p: f64,
    /// Typical sampling probability.
    pub typical_p: f64,
    /// Repetition penalty.
    pub repeat_penalty: f64,
    /// Frequency penalty.
    pub frequency_penalty: f64,
    /// Presence penalty.
    pub presence_penalty: f64,
    /// Explicit deterministic seed, when configured.
    pub seed: Option<u64>,
}

/// Typed value of one effective llama.cpp flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FlagValue {
    /// Boolean runtime flag value.
    Boolean(bool),
    /// Integer runtime flag value.
    Integer(i64),
    /// Finite floating-point runtime flag value.
    Number(f64),
    /// Non-path string runtime flag value.
    Text(String),
    /// File argument represented by a stable logical name and digest.
    Asset {
        /// Named asset, never its private machine path.
        name: String,
        /// SHA-256 of the asset bytes.
        digest: Digest,
    },
}

/// Typed value of a supported reasoning control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ControlValue {
    /// Boolean reasoning control.
    Boolean(bool),
    /// Integer reasoning control.
    Integer(i64),
    /// Finite floating-point reasoning control.
    Number(f64),
    /// String reasoning control.
    Text(String),
}

/// Resource policy, declared estimates, and measured limits with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// Effective offload and device policy.
    pub offload: OffloadPolicy,
    /// Declared memory estimate and its source, not an observation.
    pub memory_estimate: MemoryEstimate,
    /// Configured model-serving concurrency.
    pub slots: NonZeroU32,
    /// Key and value cache data types.
    pub kv_cache: KvCache,
    /// Runtime cache lifecycle policy.
    pub cache_policy: CachePolicy,
    /// Configured prompt batch size.
    pub batch_size: NonZeroU32,
    /// Configured micro-batch size, no larger than `batch_size`.
    pub micro_batch_size: NonZeroU32,
    /// Reference hardware for this qualification.
    pub hardware: HardwareIdentity,
    /// Qualified hard limits with provenance or explicit unavailable states.
    pub qualified_limits: QualifiedLimits,
}

/// Effective device offload policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OffloadPolicy {
    /// Automatic, CPU-only, or explicit device selection.
    pub mode: OffloadMode,
    /// Stable device identifiers; empty for CPU-only or automatic policy.
    pub devices: Vec<String>,
}

/// How accelerator offload devices are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffloadMode {
    /// Runtime chooses offload devices.
    Automatic,
    /// No accelerator offload.
    CpuOnly,
    /// Use the listed devices only.
    ExplicitDevices,
}

/// Declared memory estimate and provenance; not a measured limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEstimate {
    /// Nonzero publisher or operator estimate in bytes.
    pub bytes: NonZeroU64,
    /// Provenance of this declared estimate.
    pub source: String,
}

/// Key and value cache type configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KvCache {
    /// Key cache quantization type.
    pub key: KvCacheType,
    /// Value cache quantization type.
    pub value: KvCacheType,
}

/// Supported key/value cache element types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheType {
    /// 32-bit floating-point cache.
    F32,
    /// 16-bit floating-point cache.
    F16,
    /// Brain floating-point 16-bit cache.
    Bf16,
    /// Eight-bit cache.
    Q8_0,
    /// Five-bit cache.
    Q5_0,
    /// Four-bit cache.
    Q4_0,
    /// Four-bit variant-one cache.
    Q4_1,
    /// Five-bit variant-one cache.
    Q5_1,
    /// Nonlinear four-bit IQ cache.
    Iq4Nl,
}

/// Cache retention policy for the loaded model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CachePolicy {
    /// Keep the model resident after requests.
    KeepLoaded,
    /// Permit eviction while idle.
    IdleEvictable,
    /// Do not retain cache state between requests.
    PerRequest,
}

/// Public reference hardware identity and host-memory observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareIdentity {
    /// Accelerator or CPU device identity.
    pub device: String,
    /// Exact driver version used by the qualification.
    pub driver_version: String,
    /// Host CPU identity.
    pub host_cpu: String,
    /// Host memory in bytes, measured with provenance or unavailable.
    pub host_memory_bytes: Observation<NonZeroU64>,
}

/// Qualified resource limits with explicit provenance for each measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedLimits {
    /// Qualified usable context size.
    pub context_tokens: Observation<NonZeroU32>,
    /// Qualified usable output limit.
    pub output_tokens: Observation<NonZeroU32>,
    /// Qualified request concurrency.
    pub concurrency: Observation<NonZeroU32>,
    /// Qualified peak memory in bytes.
    pub peak_memory_bytes: Observation<NonZeroU64>,
}

/// A measured nonzero value with provenance, or an explicit unavailable value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation<T> {
    /// Nonzero measurement with the source that observed it.
    Measured {
        /// Measured value.
        value: T,
        /// Named measurement provenance.
        provenance: String,
    },
    /// Measurement was not available; never encoded as zero.
    Unavailable {
        /// Why the measurement is unavailable.
        reason: String,
    },
}

/// How this identity and its qualification were produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Date the immutable identity was created, in YYYY-MM-DD form.
    pub identity_created: String,
    /// Date the qualification was created, in YYYY-MM-DD form.
    pub qualification_created: String,
    /// Typed qualification process.
    pub qualification_method: QualificationMethod,
    /// Tool names and exact versions used to produce the qualification.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub tool_versions: BTreeMap<String, String>,
    /// Supporting artifact names and their content digests.
    #[serde(deserialize_with = "deserialize_unique_map")]
    pub artifacts: BTreeMap<String, Digest>,
}

/// Deserializes a sorted map while refusing repeated object keys.
fn deserialize_unique_map<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(UniqueMapVisitor(PhantomData))
}

/// Visitor for maps whose JSON representation must have unique keys.
struct UniqueMapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for UniqueMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a map with unique keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = map.next_entry()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate map key"));
            }
            values.insert(key, value);
        }
        Ok(values)
    }
}

/// Deserializes reasoning capabilities with duplicate control names rejected.
fn deserialize_reasoning<'de, D>(
    deserializer: D,
) -> Result<Capability<BTreeMap<String, ControlValue>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(
        tag = "state",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    enum Reasoning {
        Supported(
            #[serde(deserialize_with = "deserialize_unique_map")] BTreeMap<String, ControlValue>,
        ),
        Unsupported,
        NotApplicable,
    }

    Ok(match Reasoning::deserialize(deserializer)? {
        Reasoning::Supported(values) => Capability::Supported(values),
        Reasoning::Unsupported => Capability::Unsupported,
        Reasoning::NotApplicable => Capability::NotApplicable,
    })
}

/// Provenance class for how candidate qualification was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationMethod {
    /// Native tokenizer produced the expected token identities.
    NativeTokenizer,
    /// Native runtime measured invocation or resource limits.
    NativeRuntime,
    /// Identity was derived from publisher-supplied evidence.
    PublisherEvidence,
}
