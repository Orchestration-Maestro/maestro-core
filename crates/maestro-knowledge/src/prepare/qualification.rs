//! Strict native token-ID evidence for one v2 model identity.

use maestro_kernel::{
    artifact::Digest,
    gateway::{CardIdentity, ModelCard, Role, card_v2::QualificationMethod},
};
use serde::Deserialize;
use std::{collections::BTreeMap, error, fmt};

/// Strict artifact schema accepted for tokenizer qualification.
const SCHEMA: &str = "maestro-tokenizer-qualification/1";

/// A candidate's exact native tokenizer qualification artifact.
#[derive(Debug, Clone)]
pub struct TokenizerQualification {
    /// Digest of the exact serialized evidence bytes.
    digest: Digest,
    /// Whether evidence came from real native execution or synthetic CI.
    mode: QualificationMode,
    /// Digest of the candidate weights qualified by this artifact.
    model_digest: Digest,
    /// Digest of the candidate tokenizer qualified by this artifact.
    tokenizer_digest: Digest,
    /// llama.cpp build identity used during qualification.
    llama_cpp_build: String,
    /// Native tokenizer executable provenance.
    native_tool: Tool,
    /// Tokenizer library provenance.
    library: Tool,
    /// Qualification date recorded in the artifact.
    created: String,
    /// Ordered tokenizer fixtures and their confirmatory IDs.
    pub(super) fixtures: Vec<Fixture>,
}

/// Whether qualification evidence came from native execution or synthetic CI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationMode {
    /// Qualification was performed by native tokenizer execution.
    Native,
    /// Qualification was produced by synthetic test evidence.
    Synthetic,
}

/// One native tokenizer fixture, qualified twice with ordered IDs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fixture {
    /// Stable name from the committed tokenizer fixture set.
    pub(super) name: String,
    /// Exact UTF-8 input tokenized in both qualification passes.
    pub(super) input: String,
    /// Ordered token IDs from the first native pass.
    pub(super) ids: Vec<u32>,
    /// Ordered token IDs from the confirmatory native pass.
    confirmatory_ids: Vec<u32>,
    /// Whether the fixture is a tokenizer canary.
    pub(super) canary: bool,
}

/// Why a tokenizer qualification artifact was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationError(String);

/// The strict JSON envelope before its candidate facts are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    /// Versioned schema identifier.
    schema: String,
    /// Origin of this qualification evidence.
    mode: QualificationMode,
    /// Candidate model-weight digest.
    model_digest: Digest,
    /// Candidate tokenizer digest.
    tokenizer_digest: Digest,
    /// llama.cpp build used for the native pass.
    llama_cpp_build: String,
    /// Native tokenizer executable identity.
    native_tool: Tool,
    /// Tokenizer library identity.
    library: Tool,
    /// Qualification date in YYYY-MM-DD form.
    created: String,
    /// Whether special tokens were added during tokenization.
    add_special: bool,
    /// Whether special tokens were parsed during tokenization.
    parse_special: bool,
    /// Native vocabulary ID bounds.
    vocabulary: Vocabulary,
    /// Ordered qualification fixture observations.
    fixtures: Vec<Fixture>,
}

/// Provenance for one producing executable or library.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    /// Stable executable or library name.
    name: String,
    /// Exact producing tool version.
    version: String,
}

/// Inclusive token-ID bounds measured by the native tokenizer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vocabulary {
    /// Smallest valid token ID observed for this candidate.
    minimum_id: u32,
    /// Largest valid token ID observed for this candidate.
    maximum_id: u32,
}

impl TokenizerQualification {
    /// Parses and validates exact-byte tokenizer qualification evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or unsupported artifacts, incomplete
    /// fixture coverage, unstable IDs, invalid bounds, or invalid provenance.
    pub fn parse(bytes: &[u8]) -> Result<Self, QualificationError> {
        let artifact: Artifact =
            serde_json::from_slice(bytes).map_err(|error| QualificationError(error.to_string()))?;
        if artifact.schema != SCHEMA {
            return Err(refused("unknown tokenizer qualification schema"));
        }
        if !artifact.add_special || !artifact.parse_special {
            return Err(refused(
                "qualification policy must match router add_special=true and parse_special=true",
            ));
        }
        nonblank("native_tool.name", &artifact.native_tool.name)?;
        nonblank("native_tool.version", &artifact.native_tool.version)?;
        nonblank("library.name", &artifact.library.name)?;
        nonblank("library.version", &artifact.library.version)?;
        if !valid_date(&artifact.created) {
            return Err(refused("created must be a YYYY-MM-DD date"));
        }
        if artifact.vocabulary.minimum_id > artifact.vocabulary.maximum_id {
            return Err(refused("vocabulary minimum_id exceeds maximum_id"));
        }
        validate_fixtures(&artifact.fixtures, &artifact.vocabulary)?;
        Ok(Self {
            digest: Digest::of(bytes),
            mode: artifact.mode,
            model_digest: artifact.model_digest,
            tokenizer_digest: artifact.tokenizer_digest,
            llama_cpp_build: artifact.llama_cpp_build,
            native_tool: artifact.native_tool,
            library: artifact.library,
            created: artifact.created,
            fixtures: artifact.fixtures,
        })
    }

    /// Exact SHA-256 of the qualification artifact bytes.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Whether this artifact is synthetic and cannot support real eligibility.
    #[must_use]
    pub fn is_synthetic(&self) -> bool {
        self.mode == QualificationMode::Synthetic
    }

    /// The provenance mode recorded by this qualification artifact.
    #[must_use]
    pub const fn qualification_mode(&self) -> QualificationMode {
        self.mode
    }

    /// Ensures evidence was produced for the exact candidate card before a
    /// tokenizer call can be made.
    pub(crate) fn validate_for_card(&self, card: &ModelCard) -> Result<(), QualificationError> {
        let identity = card
            .identity()
            .ok_or_else(|| refused("candidate qualification requires a v2 card"))?;
        validate_identity(self, identity)
    }
}

/// Checks all named fixtures and ordered IDs against the committed category
/// set and the profile's candidate-specific vocabulary bounds.
fn validate_fixtures(
    fixtures: &[Fixture],
    vocabulary: &Vocabulary,
) -> Result<(), QualificationError> {
    let expected = super::parity::fixtures().map_err(|error| refused(error.to_string()))?;
    let expected_by_name: BTreeMap<_, _> = expected
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture))
        .collect();
    let expected_canaries: BTreeMap<_, _> = expected
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture.canary))
        .collect();
    let mut found = BTreeMap::new();
    for fixture in fixtures {
        nonblank("fixture.name", &fixture.name)?;
        if found
            .insert(fixture.name.as_str(), fixture.canary)
            .is_some()
        {
            return Err(refused(format!("duplicate fixture {:?}", fixture.name)));
        }
        let Some(native) = expected_by_name.get(fixture.name.as_str()) else {
            return Err(refused(format!("unknown fixture {:?}", fixture.name)));
        };
        if let Some(count) = boundary_token_count(&fixture.name) {
            if fixture.ids.len() != count {
                return Err(refused(format!(
                    "fixture {:?} must contain exactly {count} token IDs",
                    fixture.name
                )));
            }
        } else if !matches!(
            fixture.name.as_str(),
            "specials" | "special_only" | "mask_spaces"
        ) && fixture.input != native.input
        {
            return Err(refused(format!(
                "fixture {:?} input differs from the committed native fixture",
                fixture.name
            )));
        }
        if fixture.ids != fixture.confirmatory_ids {
            return Err(refused(format!(
                "fixture {:?} differs between native qualification runs",
                fixture.name
            )));
        }
        for id in &fixture.ids {
            if !(vocabulary.minimum_id..=vocabulary.maximum_id).contains(id) {
                return Err(refused(format!(
                    "fixture {:?} token ID {id} is outside qualified vocabulary bounds",
                    fixture.name
                )));
            }
        }
    }
    if found != expected_canaries {
        return Err(refused(
            "qualification must contain each of the 41 named fixtures and eight canaries",
        ));
    }
    Ok(())
}

/// The exact ID count required by a plain or context boundary fixture.
fn boundary_token_count(name: &str) -> Option<usize> {
    name.strip_prefix("plain_")
        .or_else(|| name.strip_prefix("context_"))?
        .parse()
        .ok()
}

/// Verifies card, tokenizer, build, tool and library provenance.
fn validate_identity(
    profile: &TokenizerQualification,
    identity: &CardIdentity,
) -> Result<(), QualificationError> {
    if identity.role != Role::Embedder {
        return Err(refused("qualification card is not an embedder"));
    }
    if identity.formats.qualification_digest != profile.digest {
        return Err(refused("qualification digest does not match the v2 card"));
    }
    if identity.weights.gguf_digest != profile.model_digest {
        return Err(refused(
            "qualification weight digest does not match the v2 card",
        ));
    }
    if identity.formats.tokenizer_digest != profile.tokenizer_digest {
        return Err(refused(
            "qualification tokenizer digest does not match the v2 card",
        ));
    }
    if identity.invocation.llama_cpp_build != profile.llama_cpp_build {
        return Err(refused(
            "qualification native build does not match the v2 card",
        ));
    }
    if identity.provenance.qualification_created != profile.created {
        return Err(refused("qualification date does not match the v2 card"));
    }
    if identity.provenance.qualification_method != QualificationMethod::NativeTokenizer {
        return Err(refused(
            "card provenance does not name native tokenizer qualification",
        ));
    }
    for tool in [&profile.native_tool, &profile.library] {
        if identity.provenance.tool_versions.get(&tool.name) != Some(&tool.version) {
            return Err(refused(format!(
                "qualification tool {:?} does not match the v2 card provenance",
                tool.name
            )));
        }
    }
    Ok(())
}

/// Whether `date` has the fixed YYYY-MM-DD spelling, without a date library.
fn valid_date(date: &str) -> bool {
    date.len() == 10
        && date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-')
        && date
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

/// Requires a nonblank provenance label.
fn nonblank(field: &str, value: &str) -> Result<(), QualificationError> {
    if value.trim().is_empty() {
        Err(refused(format!("{field} is blank")))
    } else {
        Ok(())
    }
}

/// Names a rejected artifact fact.
fn refused(reason: impl Into<String>) -> QualificationError {
    QualificationError(reason.into())
}

impl fmt::Display for QualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl error::Error for QualificationError {}
