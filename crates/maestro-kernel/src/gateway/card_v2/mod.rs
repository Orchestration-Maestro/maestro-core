//! Strict v2 model-card identity encoding and validation.

mod paths;
mod runtime;
mod types;
mod validation;

use super::card_types::Role;

pub use types::{
    Backend, CachePolicy, Capability, CardFormats, CardIdentity, ControlValue, Dimensions,
    EmbeddingFormat, FlagValue, HardwareIdentity, KvCache, KvCacheType, MemoryEstimate,
    Normalization, Observation, OffloadMode, OffloadPolicy, Pooling, Provenance,
    QualificationMethod, QualifiedLimits, Quantization, Resources, RuntimeLimits, Sampling,
    SamplingParameters, Template, TextFormat, TokenizerDerivation, WeightIdentity,
};
pub(super) use types::{CARD_SCHEMA_V2, CardV2Json};
