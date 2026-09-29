//! Ask-only ladder settings and validation.

use super::rung_prompt::RungPrompt;
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, evidence::RequestBudget};
use maestro_knowledge::{
    answer::AskBudget,
    search::evidence::{CounterMode, EvidenceSettings, ExpansionMode},
};
use serde::{Deserialize, Deserializer, de};
use serde_json::Value;

/// A rung's `ask` settings; each one absent is `ask`'s default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::min_ident_chars,
    reason = "ask's budget names this limit k, as the manifest does"
)]
pub(super) struct AskSettings {
    /// The most passages given to the answerer.
    pub(super) k: Option<u32>,
    /// The evidence budget, in UTF-8 bytes.
    pub(super) max_tokens: Option<u32>,
    /// The most tokens each answerer reply generates; absent, the answerer
    /// card's output limit.
    pub(super) output_tokens: Option<u32>,
    /// The answer prompt: a version, or a private prompt file.
    #[serde(alias = "answer_prompt")]
    pub(super) prompt: RungPrompt,
    /// Source-window allocation policy.
    pub(super) expansion: ExpansionMode,
    /// Representation charged against `max_tokens`.
    pub(super) evidence_counter: CounterMode,
    /// The SHA-256 digest, in hexadecimal, of the registered answerer card
    /// the rung asks with; absent, the latest registered answerer of the
    /// default model.
    pub(super) card: Option<String>,
}

impl AskSettings {
    /// Assembly settings carried alongside the search configuration and budget.
    pub(super) fn evidence(&self) -> EvidenceSettings {
        EvidenceSettings {
            parent_chain_order: None,
            expansion: self.expansion,
            evidence_counter: self.evidence_counter,
        }
    }
    /// The budget `ask` runs under: [`AskBudget::default`] with these
    /// settings.
    pub(super) fn budget(&self) -> AskBudget {
        let default = AskBudget::default();
        AskBudget {
            k: self.k.unwrap_or(default.k),
            max_tokens: self.max_tokens.unwrap_or(default.max_tokens),
            output_tokens: self.output_tokens.or(default.output_tokens),
            ..default
        }
    }

    /// Refuses an evidence budget over [`RequestBudget::MAX_EVIDENCE_BUDGET`],
    /// naming the ceiling.
    pub(super) fn check_evidence_budget(&self, rung: &str) -> Result<(), Failure> {
        match self.max_tokens {
            Some(bytes) if bytes > RequestBudget::MAX_EVIDENCE_BUDGET => {
                Err(Failure::refused(format!(
                    "the rung `{rung}` asks for {bytes} evidence bytes, over the \
                     {}-byte ceiling",
                    RequestBudget::MAX_EVIDENCE_BUDGET
                )))
            }
            _ => Ok(()),
        }
    }

    /// The digest of the answerer card the rung names, if any.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a card that is not a SHA-256 digest.
    pub(super) fn answerer_card(&self) -> Result<Option<Digest>, Failure> {
        self.card
            .as_deref()
            .map(|card| {
                Digest::parse(card)
                    .map_err(|_| Failure::refused("a rung's answerer card is not a SHA-256 digest"))
            })
            .transpose()
    }
}

/// Reads a rung's `ask`: `false`, `true` for the default settings, or its
/// settings.
pub(super) fn read_ask<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AskSettings>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Bool(asks) => Ok(asks.then(AskSettings::default)),
        value => AskSettings::deserialize(value)
            .map(Some)
            .map_err(de::Error::custom),
    }
}
