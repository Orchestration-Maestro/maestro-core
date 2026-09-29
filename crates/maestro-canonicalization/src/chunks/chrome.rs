//! The page chrome a profile leaves out of a document's indexed text, counted per rule.
use super::batch::ChunkDocument;
use super::validation::chrome_ranges;
use crate::chunk_mapping::{map_document, mapped_slice};
use crate::chunk_profile::{ChromeRule, ChunkProfile};
use crate::chunk_split::{Layout, heading_title};
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::model::SourceSpan;
use crate::source_units::TextRange;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The page chrome one rule left out: of how many units, and how many of their bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChromeCount {
    /// The units some or all of whose text the rule left out.
    pub units: u64,
    /// The bytes of unit text it left out.
    pub bytes: u64,
}

impl ChromeCount {
    /// This count with `other` added.
    #[must_use]
    pub const fn plus(self, other: Self) -> Self {
        Self {
            units: self.units + other.units,
            bytes: self.bytes + other.bytes,
        }
    }
}

impl ChunkDocument {
    /// The chrome its coverage records, per rule; empty when nothing was left out.
    #[must_use]
    pub fn chrome(&self) -> BTreeMap<ChromeRule, ChromeCount> {
        let mut counts = BTreeMap::new();
        for unit in &self.coverage {
            if let Some(rule) = unit.chrome_rule {
                add(&mut counts, rule, &unit.chrome_ranges);
            }
        }
        counts
    }
}

/// The chrome `profile` leaves out of `document`, whose original is `markdown`, per rule, as
/// [`ChunkDocument::chrome`] counts it once chunked, without chunking or counting tokens.
///
/// # Errors
/// Refuses a document whose units cannot be mapped to `markdown` or whose structure is invalid.
pub fn left_out_chrome(
    document: &CanonicalDocument,
    markdown: &str,
    profile: ChunkProfile,
) -> Result<BTreeMap<ChromeRule, ChromeCount>, Error> {
    let mapped = map_document(document, markdown)?;
    let layout = Layout::new(document, markdown, &mapped, profile)?;
    let mut counts = BTreeMap::new();
    for (index, unit) in mapped.units.iter().enumerate() {
        let Some(rule) = layout.chrome_rule(index) else {
            continue;
        };
        add(
            &mut counts,
            rule,
            &chrome_ranges(layout.kept(index), unit.text.len()),
        );
    }
    Ok(counts)
}

/// The byte ranges of `markdown`, the original of `document`, that `profile` leaves out of the
/// indexed text as page chrome, sorted, each an origin of left-out text; none, reading nothing,
/// under a profile without chrome rules. A reader that shows source text as a profile indexed
/// it cuts these.
///
/// # Errors
/// Refuses a document whose units cannot be mapped to `markdown` or whose structure is invalid.
pub fn chrome_spans(
    document: &CanonicalDocument,
    markdown: &str,
    profile: ChunkProfile,
) -> Result<Vec<SourceSpan>, Error> {
    if profile.rules().chrome.is_none() {
        return Ok(Vec::new());
    }
    let mapped = map_document(document, markdown)?;
    let layout = Layout::new(document, markdown, &mapped, profile)?;
    let mut spans = Vec::new();
    for (index, unit) in mapped.units.iter().enumerate() {
        if layout.chrome_rule(index).is_none() {
            continue;
        }
        for range in chrome_ranges(layout.kept(index), unit.text.len()) {
            for run in mapped_slice(unit, range, markdown)? {
                spans.extend(run.origins.iter().map(|origin| origin.span));
            }
        }
    }
    spans.sort_unstable_by_key(|span| (span.start, span.end));
    Ok(spans)
}

/// A heading title as `profile` indexes it: without a trailing label it leaves out as page
/// chrome.
#[must_use]
pub fn indexed_title(profile: ChunkProfile, title: &str) -> String {
    heading_title(profile.rules().chrome.as_ref(), title)
}

/// Count one unit's chrome `ranges` under `rule`.
fn add(counts: &mut BTreeMap<ChromeRule, ChromeCount>, rule: ChromeRule, ranges: &[TextRange]) {
    let bytes = ranges
        .iter()
        .map(|range| range.end - range.start)
        .sum::<usize>();
    let count = ChromeCount {
        units: 1,
        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
    };
    let total = counts.entry(rule).or_default();
    *total = total.plus(count);
}
