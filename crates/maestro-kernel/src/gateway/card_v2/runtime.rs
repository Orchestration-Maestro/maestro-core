//! Canonical runtime-flag checks for values also held in typed identity fields.

use super::super::card_types::CardError;
use super::{CardIdentity, EmbeddingFormat, FlagValue, KvCacheType, Pooling};

/// Checks runtime flags that repeat typed identity values when present.
pub(super) fn validate_runtime_flags(identity: &CardIdentity) -> Result<(), CardError> {
    let flags = &identity.invocation.server_flags;
    if let Some(value) = flags.get("--model") {
        match value {
            FlagValue::Asset { digest, .. } if digest == &identity.weights.gguf_digest => {}
            _ => {
                return Err(invalid(
                    "server_flags.--model differs from weights.gguf_digest",
                ));
            }
        }
    }
    let mut expected = vec![
        (
            "--ctx-size",
            FlagValue::Integer(i64::from(identity.invocation.limits.context_tokens.get())),
        ),
        (
            "--batch-size",
            FlagValue::Integer(i64::from(identity.resources.batch_size.get())),
        ),
        (
            "--ubatch-size",
            FlagValue::Integer(i64::from(identity.resources.micro_batch_size.get())),
        ),
        (
            "--cache-type-k",
            FlagValue::Text(cache_type_name(identity.resources.kv_cache.key).to_owned()),
        ),
        (
            "--cache-type-v",
            FlagValue::Text(cache_type_name(identity.resources.kv_cache.value).to_owned()),
        ),
    ];
    if let EmbeddingFormat::Supported { pooling, .. } = identity.formats.embedding {
        expected.push((
            "--pooling",
            FlagValue::Text(pooling_name(pooling).to_owned()),
        ));
    }
    for (flag, expected) in expected {
        if flags.get(flag).is_some_and(|actual| actual != &expected) {
            return Err(invalid(format!(
                "server_flags.{flag} differs from its typed identity"
            )));
        }
    }
    Ok(())
}

/// The llama.cpp spelling of a typed key/value cache mode.
fn cache_type_name(cache_type: KvCacheType) -> &'static str {
    match cache_type {
        KvCacheType::F32 => "f32",
        KvCacheType::F16 => "f16",
        KvCacheType::Bf16 => "bf16",
        KvCacheType::Q8_0 => "q8_0",
        KvCacheType::Q5_0 => "q5_0",
        KvCacheType::Q4_0 => "q4_0",
        KvCacheType::Q4_1 => "q4_1",
        KvCacheType::Q5_1 => "q5_1",
        KvCacheType::Iq4Nl => "iq4_nl",
    }
}

/// The llama.cpp spelling of an embedding pooling mode.
fn pooling_name(pooling: Pooling) -> &'static str {
    match pooling {
        Pooling::Cls => "cls",
        Pooling::Mean => "mean",
        Pooling::LastToken => "last",
    }
}

/// Builds an invalid-card error with its reason.
fn invalid(reason: impl Into<String>) -> CardError {
    CardError::Invalid(reason.into())
}
