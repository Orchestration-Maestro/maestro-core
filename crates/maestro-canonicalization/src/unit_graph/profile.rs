//! Digest-pinned /4 graph and ranking rules.
use crate::{chunk_split::MAX_TOKENS, error::Error, hashing::digest};
use serde::{Deserialize, Serialize};

/// Retrieval-unit construction rules for the two /4 comparison arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankedUnit {
    /// Reuse /3 complete-idea retrieval packing as the control arm.
    CompleteIdeas,
    /// Rank the bounded structural delivery units directly.
    V2Unit,
}

impl RankedUnit {
    /// Parse a supported rule name; unknown values do not fall back.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "CompleteIdeas" => Some(Self::CompleteIdeas),
            "V2Unit" => Some(Self::V2Unit),
            _ => None,
        }
    }
}

/// Explicit counter-based limits for each kind; all values are provisional pending measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSizeLimits {
    /// Maximum verified tokens for section units.
    pub section_tokens: usize,
    /// Maximum verified tokens for table units.
    pub table_tokens: usize,
    /// Maximum verified tokens for row units.
    pub row_tokens: usize,
    /// Maximum verified tokens for procedure units.
    pub procedure_tokens: usize,
    /// Maximum verified tokens for code units.
    pub code_tokens: usize,
    /// Maximum verified tokens for coherent paragraph groups.
    pub paragraphs_tokens: usize,
}

impl UnitSizeLimits {
    /// Whether every configured limit is nonzero and within the existing verified-counter cap.
    #[must_use]
    pub const fn all_within_verified_counter_cap(self) -> bool {
        self.section_tokens > 0
            && self.section_tokens <= MAX_TOKENS
            && self.table_tokens > 0
            && self.table_tokens <= MAX_TOKENS
            && self.row_tokens > 0
            && self.row_tokens <= MAX_TOKENS
            && self.procedure_tokens > 0
            && self.procedure_tokens <= MAX_TOKENS
            && self.code_tokens > 0
            && self.code_tokens <= MAX_TOKENS
            && self.paragraphs_tokens > 0
            && self.paragraphs_tokens <= MAX_TOKENS
    }

    /// The profile's limits remain provisional until paired measurement pins final values.
    #[must_use]
    pub const fn provisional(self) -> bool {
        let _ = self;
        true
    }
}

/// Rules that produce the /4 delivery graph and one pinned ranking view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitProfile {
    /// The retrieval ranking unit policy.
    pub ranked_unit: RankedUnit,
    /// Per-kind verified-token ceilings.
    pub size_limits: UnitSizeLimits,
}

impl UnitProfile {
    /// New /4 profile with provisional ceilings based on the prototype's structural caps and /3's
    /// verified counter maximum. Values are token counts, not byte or character estimates.
    #[must_use]
    pub const fn new(ranked_unit: RankedUnit) -> Self {
        Self {
            ranked_unit,
            size_limits: UnitSizeLimits {
                section_tokens: 500,
                table_tokens: 500,
                row_tokens: 500,
                procedure_tokens: 500,
                code_tokens: 500,
                paragraphs_tokens: 500,
            },
        }
    }

    /// The new chunker name; /2 and /3 remain named only by `ChunkProfile`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        "mapped-structural-chunks/4"
    }

    /// The /4 preparation name.
    #[must_use]
    pub const fn preparation_name(self) -> &'static str {
        "canonical-context-parts/v3"
    }

    /// Ranking rules this profile pins.
    #[must_use]
    pub const fn ranked_unit(self) -> RankedUnit {
        self.ranked_unit
    }

    /// Per-kind counter limits pinned into the profile.
    #[must_use]
    pub const fn size_limits(self) -> UnitSizeLimits {
        self.size_limits
    }

    /// SHA-256 of the complete serialized rules, including context and family policy versions.
    ///
    /// # Errors
    /// Returns an error if the profile record cannot be serialized.
    pub fn profile_digest(self) -> Result<String, Error> {
        let bytes = serde_json::to_vec(&(
            "unit-profile/1",
            self,
            "required-context/2",
            "heading-path-from-ancestors/1",
            "family-normalization/1",
        ))
        .map_err(|error| Error(error.to_string()))?;
        Ok(format!("sha256:{}", digest(&bytes)))
    }
}
