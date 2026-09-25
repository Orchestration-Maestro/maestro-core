//! Section and extractor checks: the heading hierarchy and supplied extractor anchors.
use super::report::issue;
use crate::content::{Block, BlockType};
use crate::document::{CanonicalDocument, Section};
use crate::metadata::finding;
use crate::model::{Finding, Severity};
use std::collections::{BTreeMap, BTreeSet};

/// Each section's parent is an earlier, shallower section, and its heading path follows the
/// chain of its parents.
pub(super) fn validate_sections(
    doc: &CanonicalDocument,
    blocks: &BTreeMap<&str, &Block>,
    issues: &mut Vec<Finding>,
) {
    let mut seen = BTreeMap::new();
    for section in &doc.sections {
        let mut path = inherited_path(&seen, section, issues);
        path.push(section.title.clone());
        if path != section.heading_path
            || !(1..=6).contains(&section.level)
            || !blocks
                .get(section.section_id.as_str())
                .is_some_and(|heading| heading.block_type == BlockType::Heading)
        {
            issues.push(finding(
                "invalid_section",
                "section path, heading or level is inconsistent",
                Severity::Error,
                None,
            ));
        }
        if seen.insert(&section.section_id, section).is_some() {
            issues.push(finding(
                "invalid_section",
                "duplicate section identity",
                Severity::Error,
                None,
            ));
        }
    }
    for block in &doc.blocks {
        let expected = if block.block_type == BlockType::Heading {
            seen.get(&block.block_id)
        } else {
            block
                .parent_section_id
                .as_ref()
                .and_then(|parent_id| seen.get(parent_id))
        };
        if expected
            .map(|section| &section.heading_path)
            .cloned()
            .unwrap_or_default()
            != block.heading_path
            || block
                .parent_section_id
                .as_ref()
                .is_some_and(|parent_id| !seen.contains_key(parent_id))
        {
            issue(
                issues,
                block,
                "invalid_section_context",
                "block heading context is inconsistent",
                Severity::Error,
            );
        }
    }
}

/// The heading path a section inherits from its parent, which must be an earlier, shallower
/// section.
fn inherited_path(
    seen: &BTreeMap<&String, &Section>,
    section: &Section,
    issues: &mut Vec<Finding>,
) -> Vec<String> {
    let Some(parent_id) = &section.parent_section_id else {
        return Vec::new();
    };
    let Some(parent) = seen.get(parent_id) else {
        issues.push(finding(
            "invalid_section",
            "parent section must precede child",
            Severity::Error,
            None,
        ));
        return Vec::new();
    };
    if parent.level >= section.level {
        issues.push(finding(
            "invalid_section",
            "parent heading level is not shallower",
            Severity::Error,
            None,
        ));
    }
    parent.heading_path.clone()
}

/// Supplied extractor blocks need a unique identifier, at least one valid Markdown anchor and
/// one-based pages; their payload is retained, not verified.
pub(super) fn validate_extractor(
    doc: &CanonicalDocument,
    markdown: &str,
    issues: &mut Vec<Finding>,
) {
    let mut ids = BTreeSet::new();
    for supplied in &doc.extractor_blocks {
        if supplied.extractor_id.is_empty() || !ids.insert(&supplied.extractor_id) {
            issues.push(finding(
                "invalid_extractor_id",
                "extractor block IDs must be nonempty and unique",
                Severity::Error,
                None,
            ));
        }
        if supplied.markdown_spans.is_empty() {
            issues.push(finding(
                "unanchored_extractor_block",
                "extractor structure retained without inventing a Markdown mapping",
                Severity::Warning,
                None,
            ));
        }
        for span in &supplied.markdown_spans {
            if !span.is_valid(markdown) {
                issues.push(finding(
                    "invalid_extractor_span",
                    "supplied mapping is out of bounds or splits UTF-8",
                    Severity::Error,
                    Some(*span),
                ));
            }
        }
        for location in &supplied.original_locations {
            if location.page == Some(0) {
                issues.push(finding(
                    "invalid_original_location",
                    "supplied page number must be positive",
                    Severity::Error,
                    None,
                ));
            }
        }
        issues.push(finding(
            "extractor_payload_retained",
            concat!(
                "extractor anchors validated; opaque original structure retained, ",
                "semantic fidelity not independently verified"
            ),
            Severity::Warning,
            None,
        ));
    }
}
