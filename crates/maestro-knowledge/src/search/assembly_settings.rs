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
    /// Which representation and unit to charge.
    pub evidence_counter: CounterMode,
}
