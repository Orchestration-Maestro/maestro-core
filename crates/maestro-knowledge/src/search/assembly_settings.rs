//! Per-request evidence assembly knobs, separate from copyable search ranking.

use serde::{Deserialize, Serialize};

/// Allocation strategy for canonical source windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionMode {
    /// Preserve full-section-first allocation.
    #[default]
    FullSection,
    /// Admit matched whole blocks before expanding neighboring context.
    RelevantBlocks,
    /// Admit the smallest complete parent-chain choice as exact separate ranges, then grow.
    ParentChain,
}

/// Order for admitting complete parent-chain choices with identical ranked seeds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentChainOrder {
    /// Reserve the smallest complete unit before growing context.
    #[default]
    MinimumCompleteFirst,
    /// Prefer the largest complete parent that fits the remaining budget.
    LargestFittingParent,
}

/// Named counter policy; exact mode refuses until answerer qualification is available.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterMode {
    /// Count the complete passage wire representation, as before.
    #[default]
    Utf8,
    /// Count only the fields passed to the answerer, with a separate wire ceiling.
    Utf8AnswerBound,
    /// Require a qualified tokenizer of the resolved answerer, never approximate silently.
    Exact,
}

/// Evidence settings carried alongside search configuration and request bounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EvidenceSettings {
    /// How to allocate source context.
    pub expansion: ExpansionMode,
    /// Optional order override, valid only for parent-chain expansion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_chain_order: Option<ParentChainOrder>,
    /// Which representation and unit to charge.
    pub evidence_counter: CounterMode,
}
