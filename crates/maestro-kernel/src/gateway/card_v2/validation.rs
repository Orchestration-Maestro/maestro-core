//! Semantic validation and legacy gateway projection for v2 identities.

use super::super::card_types::{CardError, CardFields};
use super::{
    Backend, Capability, CardFormats, CardIdentity, ControlValue, Dimensions, EmbeddingFormat,
    FlagValue, Observation, OffloadMode, Provenance, Resources, Role, RuntimeLimits, Sampling,
    SamplingParameters, Template, TokenizerDerivation, WeightIdentity,
};
use super::{
    paths::{contains_machine_path, is_absolute_machine_path, is_machine_path},
    runtime::validate_runtime_flags,
};
use crate::artifact::Digest;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

impl CardIdentity {
    /// Validates typed metadata before it becomes a content-addressed identity.
    pub(crate) fn validate(&self) -> Result<(), CardError> {
        validate_weights(&self.weights)?;
        validate_formats(&self.formats, &self.weights)?;
        validate_invocation(&self.invocation)?;
        validate_resources(&self.resources, self.invocation.backend)?;
        validate_provenance(&self.provenance)?;
        validate_role(self)?;
        validate_runtime_flags(self)
    }

    /// Evidence and runtime assets to pin; the model GGUF stays external.
    pub(crate) fn artifact_digests(&self) -> BTreeSet<Digest> {
        let mut digests = BTreeSet::from([self.formats.qualification_digest.clone()]);
        if self.formats.tokenizer_derivation != TokenizerDerivation::GgufEmbedded {
            digests.insert(self.formats.tokenizer_digest.clone());
        }
        if let Template::Digest(digest) = &self.formats.template {
            digests.insert(digest.clone());
        }
        digests.extend(self.provenance.artifacts.values().cloned());
        for value in self.invocation.server_flags.values() {
            if let FlagValue::Asset { digest, .. } = value
                && digest != &self.weights.gguf_digest
                && !self
                    .weights
                    .adapters
                    .values()
                    .any(|weight| weight == digest)
                && !self.weights.drafts.values().any(|weight| weight == digest)
                && !self
                    .weights
                    .projectors
                    .values()
                    .any(|weight| weight == digest)
            {
                digests.insert(digest.clone());
            }
        }
        digests
    }

    /// Projects identity fields needed by existing gateway ports.
    pub(crate) fn legacy_fields(&self) -> CardFields {
        CardFields {
            role: self.role,
            router_entry: self.router_entry.clone(),
            file_digest: self.weights.gguf_digest.clone(),
            template_digest: match &self.formats.template {
                Template::Absent => None,
                Template::Digest(digest) => Some(digest.clone()),
            },
            server_build: self.invocation.llama_cpp_build.clone(),
            dimensions: match self.invocation.dimensions {
                Dimensions::Measured(value) => Some(value),
                Dimensions::NotApplicable => None,
            },
            limits: self.invocation.limits,
            suite_results: Vec::new(),
        }
    }
}

/// Validates upstream weight facts and named companion assets.
fn validate_weights(weights: &WeightIdentity) -> Result<(), CardError> {
    for (field, value) in [
        ("upstream_model_id", weights.upstream_model_id.as_str()),
        ("upstream_revision", weights.upstream_revision.as_str()),
        ("source_url", weights.source_url.as_str()),
        ("licence_id", weights.licence_id.as_str()),
        (
            "licence_terms_source",
            weights.licence_terms_source.as_str(),
        ),
    ] {
        nonblank(field, value)?;
    }
    if is_absolute_machine_path(&weights.upstream_model_id) {
        return Err(invalid("upstream_model_id is a machine path"));
    }
    validate_public_url("source_url", &weights.source_url)?;
    validate_public_url("licence_terms_source", &weights.licence_terms_source)?;
    for (kind, assets) in [
        ("adapter", &weights.adapters),
        ("draft", &weights.drafts),
        ("projector", &weights.projectors),
    ] {
        for name in assets.keys() {
            asset_name(kind, name)?;
        }
    }
    Ok(())
}

/// Requires a credential-free HTTP(S) publisher or licence URL.
fn validate_public_url(field: &str, text: &str) -> Result<(), CardError> {
    let url = reqwest::Url::parse(text).map_err(|error| invalid(format!("{field}: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(format!(
            "{field} must be an HTTP(S) URL without credentials"
        )));
    }
    Ok(())
}

/// Validates tokenizer/template identities and embedding input formats.
fn validate_formats(formats: &CardFormats, weights: &WeightIdentity) -> Result<(), CardError> {
    if formats.tokenizer_derivation == TokenizerDerivation::GgufEmbedded
        && formats.tokenizer_digest != weights.gguf_digest
    {
        return Err(invalid(
            "GGUF-embedded tokenizer digest must equal the complete GGUF digest",
        ));
    }
    Ok(())
}

/// Validates resolved runtime settings and their finite numeric values.
fn validate_invocation(invocation: &RuntimeLimits) -> Result<(), CardError> {
    nonblank("llama_cpp_build", &invocation.llama_cpp_build)?;
    for flag in invocation.server_flags.keys() {
        nonblank("server_flags key", flag)?;
        let canonical = flag.strip_prefix("--").is_some_and(|name| {
            !name.is_empty()
                && name.split('-').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                })
        });
        if !canonical {
            let hint = canonical_flag_hint(flag).map_or_else(
                || "use its long option name".to_owned(),
                |name| format!("use {name} and keep its value separate"),
            );
            return Err(invalid(format!(
                "server_flags.{flag} is not a canonical long option; {hint}"
            )));
        }
    }
    for (flag, value) in &invocation.server_flags {
        match value {
            FlagValue::Number(value) => finite("server_flags", *value)?,
            FlagValue::Text(value) if requires_asset(flag, value) => {
                return Err(invalid(format!(
                    "server_flags.{flag} file path must be a named digest-bound asset"
                )));
            }
            FlagValue::Asset { name, .. } => asset_name(flag, name)?,
            _ => {}
        }
    }
    if let Capability::Supported(values) = &invocation.reasoning {
        validate_control_map(values)?;
    }
    if let Sampling::Configured(sampling) = &invocation.sampling {
        sampling.validate()?;
    }
    Ok(())
}

/// Suggests a real llama.cpp spelling without inventing one from a short key.
fn canonical_flag_hint(flag: &str) -> Option<String> {
    let key = flag.split_once('=').map_or(flag, |(key, _)| key);
    let alias = match key {
        "-m" => Some("--model"),
        "-c" => Some("--ctx-size"),
        "-b" => Some("--batch-size"),
        "-ub" => Some("--ubatch-size"),
        "-ctk" => Some("--cache-type-k"),
        "-ctv" => Some("--cache-type-v"),
        _ => None,
    };
    if let Some(alias) = alias {
        return Some(alias.to_owned());
    }
    let name = key.strip_prefix("--")?;
    let normalized = name.replace('_', "-").to_ascii_lowercase();
    if normalized.is_empty()
        || !normalized.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some(format!("--{normalized}"))
}

/// Validates resource policy and makes every missing measurement explicit.
fn validate_resources(resources: &Resources, backend: Backend) -> Result<(), CardError> {
    validate_path_free_text("memory_estimate.source", &resources.memory_estimate.source)?;
    for (field, value) in [
        ("hardware.device", resources.hardware.device.as_str()),
        (
            "hardware.driver_version",
            resources.hardware.driver_version.as_str(),
        ),
        ("hardware.host_cpu", resources.hardware.host_cpu.as_str()),
    ] {
        nonblank(field, value)?;
    }
    if resources.micro_batch_size > resources.batch_size {
        return Err(invalid("micro_batch_size exceeds batch_size"));
    }
    match resources.offload.mode {
        OffloadMode::Automatic | OffloadMode::CpuOnly if !resources.offload.devices.is_empty() => {
            return Err(invalid(
                "automatic and cpu_only offload have no device names",
            ));
        }
        OffloadMode::ExplicitDevices if resources.offload.devices.is_empty() => {
            return Err(invalid("explicit_devices offload has no devices"));
        }
        _ => {}
    }
    if backend == Backend::Cpu && resources.offload.mode == OffloadMode::ExplicitDevices {
        return Err(invalid("CPU backend cannot use explicit devices"));
    }
    for device in &resources.offload.devices {
        nonblank("offload.device", device)?;
        if is_machine_path(device) {
            return Err(invalid("offload.device is a machine path"));
        }
    }
    observation("host_memory_bytes", &resources.hardware.host_memory_bytes)?;
    observation(
        "qualified_limits.context_tokens",
        &resources.qualified_limits.context_tokens,
    )?;
    observation(
        "qualified_limits.output_tokens",
        &resources.qualified_limits.output_tokens,
    )?;
    observation(
        "qualified_limits.concurrency",
        &resources.qualified_limits.concurrency,
    )?;
    observation(
        "qualified_limits.peak_memory_bytes",
        &resources.qualified_limits.peak_memory_bytes,
    )?;
    Ok(())
}

/// Validates provenance dates, versions, and supporting artifact names.
fn validate_provenance(provenance: &Provenance) -> Result<(), CardError> {
    validate_date("identity_created", &provenance.identity_created)?;
    validate_date("qualification_created", &provenance.qualification_created)?;
    validate_map("tool_versions", &provenance.tool_versions)?;
    if provenance
        .tool_versions
        .iter()
        .any(|(tool, version)| is_absolute_machine_path(tool) || is_absolute_machine_path(version))
    {
        return Err(invalid("tool_versions cannot contain a machine path"));
    }
    for name in provenance.artifacts.keys() {
        asset_name("provenance.artifacts key", name)?;
    }
    Ok(())
}

/// Enforces role-specific dimensions, formats, sampling, and output limits.
fn validate_role(identity: &CardIdentity) -> Result<(), CardError> {
    let no_embedding_profile = non_embedding_profile_is_not_applicable(identity);
    let valid = match identity.role {
        Role::Embedder => {
            matches!(identity.invocation.dimensions, Dimensions::Measured(_))
                && matches!(
                    identity.formats.embedding,
                    EmbeddingFormat::Supported { .. }
                )
                && matches!(identity.formats.document, Capability::Supported(_))
                && matches!(identity.formats.query, Capability::Supported(_))
                && matches!(identity.invocation.sampling, Sampling::NotApplicable)
                && matches!(identity.invocation.reasoning, Capability::NotApplicable)
                && identity.invocation.limits.output_tokens.is_none()
        }
        Role::Reranker => {
            no_embedding_profile
                && matches!(identity.invocation.sampling, Sampling::NotApplicable)
                && matches!(identity.invocation.reasoning, Capability::NotApplicable)
                && identity.invocation.limits.output_tokens.is_none()
        }
        Role::Answerer => {
            no_embedding_profile
                && matches!(identity.invocation.sampling, Sampling::Configured(_))
                && identity.invocation.limits.output_tokens.is_some()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            concat!(
                "identity formats, dimensions, sampling, reasoning, and output ",
                "limits disagree with {} role"
            ),
            identity.role
        )))
    }
}

impl SamplingParameters {
    /// Validates configured generation sampling ranges and finiteness.
    fn validate(&self) -> Result<(), CardError> {
        for (field, value) in [
            ("temperature", self.temperature),
            ("top_p", self.top_p),
            ("min_p", self.min_p),
            ("typical_p", self.typical_p),
            ("repeat_penalty", self.repeat_penalty),
            ("frequency_penalty", self.frequency_penalty),
            ("presence_penalty", self.presence_penalty),
        ] {
            finite(field, value)?;
        }
        if !(0.0..=1.0).contains(&self.top_p)
            || !(0.0..=1.0).contains(&self.min_p)
            || !(0.0..=1.0).contains(&self.typical_p)
            || self.temperature < 0.0
            || self.repeat_penalty <= 0.0
        {
            return Err(invalid("sampling probability or penalty is out of range"));
        }
        Ok(())
    }
}

/// Checks that non-embedding roles carry no embedding dimensions or formats.
fn non_embedding_profile_is_not_applicable(identity: &CardIdentity) -> bool {
    matches!(identity.invocation.dimensions, Dimensions::NotApplicable)
        && matches!(identity.formats.embedding, EmbeddingFormat::NotApplicable)
        && matches!(identity.formats.document, Capability::NotApplicable)
        && matches!(identity.formats.query, Capability::NotApplicable)
}

/// Validates supported reasoning control names and finite numbers.
fn validate_control_map(map: &BTreeMap<String, ControlValue>) -> Result<(), CardError> {
    for (name, value) in map {
        nonblank("reasoning control", name)?;
        if let ControlValue::Number(value) = value {
            finite("reasoning control", *value)?;
        }
    }
    Ok(())
}

/// Requires a nonblank provenance for a measurement or its unavailable reason.
fn observation<T>(field: &str, value: &Observation<T>) -> Result<(), CardError> {
    match value {
        Observation::Measured { provenance, .. } => validate_path_free_text(field, provenance),
        Observation::Unavailable { reason } => validate_path_free_text(field, reason),
    }
}

/// Refuses local paths in human-readable provenance without flagging short tokens.
fn validate_path_free_text(field: &str, text: &str) -> Result<(), CardError> {
    nonblank(field, text)?;
    if contains_machine_path(text) {
        Err(invalid(format!("{field} contains a machine path")))
    } else {
        Ok(())
    }
}

/// Refuses blank names or versions in a provenance map.
fn validate_map(map_name: &str, map: &BTreeMap<String, String>) -> Result<(), CardError> {
    for (key, value) in map {
        nonblank(map_name, key)?;
        nonblank(map_name, value)?;
    }
    Ok(())
}

/// Validates a named asset and refuses path traversal or local paths.
fn asset_name(field: &str, name: &str) -> Result<(), CardError> {
    nonblank(field, name)?;
    if name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.split('/').any(|part| part == "..")
    {
        return Err(invalid(format!("{field} asset name is a path")));
    }
    Ok(())
}

/// Identifies flag strings that must instead reference a digest-bound asset.
fn requires_asset(flag: &str, value: &str) -> bool {
    let flag = flag.to_ascii_lowercase();
    let extension = Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str());
    let route_or_alias = matches!(flag.as_str(), "--api-prefix" | "--alias");
    flag.contains("model")
        || flag.contains("weight")
        || flag.contains("path")
        || flag.contains("file")
        || flag.contains("adapter")
        || flag.contains("projector")
        || flag.contains("draft")
        || flag.contains("lora")
        || flag.contains("mmproj")
        || (!route_or_alias && is_machine_path(value))
        || extension.is_some_and(|extension| {
            ["gguf", "bin", "model"]
                .iter()
                .any(|expected| extension.eq_ignore_ascii_case(expected))
        })
}

/// Checks a required date's exact calendar validity in YYYY-MM-DD form.
fn validate_date(field: &str, value: &str) -> Result<(), CardError> {
    let bytes = value.as_bytes();
    let valid_shape = value.len() == 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !valid_shape {
        return Err(invalid(format!("{field} must be YYYY-MM-DD")));
    }
    let year = value
        .get(0..4)
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    let month = value
        .get(5..7)
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    let day = value
        .get(8..10)
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let last_day = month
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| month_days.get(index))
        .copied();
    if day == 0 || last_day.is_none_or(|last_day| day > last_day) {
        return Err(invalid(format!("{field} is not a calendar date")));
    }
    Ok(())
}

/// Refuses a non-finite numeric configuration value.
fn finite(field: &str, value: f64) -> Result<(), CardError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(invalid(format!("{field} must be finite")))
    }
}

/// Refuses a mandatory string containing only whitespace.
fn nonblank(field: &str, value: &str) -> Result<(), CardError> {
    if value.trim().is_empty() {
        Err(invalid(format!("{field} is blank")))
    } else {
        Ok(())
    }
}

/// Builds the invalid-card error used by v2 validation.
fn invalid(reason: impl Into<String>) -> CardError {
    CardError::Invalid(reason.into())
}
