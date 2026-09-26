//! The checks of what a canonical document records beside its body: its
//! verdict, its provenance, its assets and the shape of its tables.

use super::flag::{Flag, counted, number};
use maestro_canonicalization::{
    AssetStatus, Block, BlockType, CanonicalDocument, Severity, ValidationStatus,
};
use maestro_kernel::document::Outcome;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `canonicalization.failed`: canonicalization found an error, so the
/// document is failed (docs/architecture/01 §5, §13): it stays inspectable
/// and needs another extraction. Warnings alone leave it valid.
pub(super) fn canonicalization_failed(document: &CanonicalDocument) -> Option<Flag> {
    if document.validation_status != ValidationStatus::Failed {
        return None;
    }
    let mut errors: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for finding in &document.warnings {
        if finding.severity == Severity::Error {
            *errors
                .entry((finding.code.as_str(), finding.message.as_str()))
                .or_default() += 1;
        }
    }
    let errors: Vec<String> = errors
        .into_iter()
        .map(|((code, message), times)| match times {
            1 => format!("{code}: {message}"),
            _ => format!("{code}: {message} ({times} times)"),
        })
        .collect();
    Some(Flag {
        rule: "canonicalization.failed",
        outcome: Outcome::NeedsReextraction,
        reason: format!("canonicalization failed: {}", errors.join("; ")),
    })
}

/// `metadata.missing`: no source reference, or no title, the two fields
/// every citation shows, so its provenance is unresolved until someone
/// reviews it. Its language, its extraction and its access policy may be
/// unknown: an import rarely knows them.
pub(super) fn metadata_missing(document: &CanonicalDocument) -> Option<Flag> {
    let blank = |text: Option<&str>| text.is_none_or(|text| text.trim().is_empty());
    let missing: Vec<&str> = [
        (
            "source reference",
            blank(document.source_reference.as_deref()),
        ),
        ("title", blank(document.source_metadata.title.as_deref())),
    ]
    .into_iter()
    .filter_map(|(field, missing)| missing.then_some(field))
    .collect();
    (!missing.is_empty()).then(|| Flag {
        rule: "metadata.missing",
        outcome: Outcome::Quarantined,
        reason: format!(
            "it has no {}: its provenance is unresolved until someone reviews it",
            missing.join(" and no ")
        ),
    })
}

/// `assets.missing`: an asset the document refers to is missing, or outside
/// the root it may be read from. An asset nobody checked, as an import's,
/// is unknown rather than missing, and a remote one is never fetched.
pub(super) fn assets_missing(document: &CanonicalDocument) -> Option<Flag> {
    let missing = document
        .blocks
        .iter()
        .flat_map(|block| &block.asset_references)
        .filter(|asset| {
            matches!(
                asset.status,
                AssetStatus::Missing | AssetStatus::OutsideRoot
            )
        })
        .count();
    (missing > 0).then(|| Flag {
        rule: "assets.missing",
        outcome: Outcome::AcceptedWithWarnings,
        reason: format!(
            "{} it refers to: missing, or outside the root it may be read from",
            counted(number(missing), "asset")
        ),
    })
}

/// `table.incomplete`: a table row with fewer cells than its header, which
/// the parser fills with a cell that holds no source, or with more, whose
/// surplus canonicalization keeps verbatim beside the cells. A cell written
/// empty is a value, and a table without rows is whole.
pub(super) fn table_incomplete(document: &CanonicalDocument) -> Option<Flag> {
    let by_id: HashMap<&str, &Block> = document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    let rows: BTreeSet<&str> = document
        .blocks
        .iter()
        .filter_map(|block| {
            let row = block.parent_block_id.as_deref()?;
            let filled = block.block_type == BlockType::TableCell
                && block.source_spans.iter().all(|span| span.start == span.end);
            let surplus = block.block_type == BlockType::Raw
                && by_id.get(row).is_some_and(|row| {
                    matches!(row.block_type, BlockType::TableRow | BlockType::TableHead)
                });
            (filled || surplus).then_some(row)
        })
        .collect();
    (!rows.is_empty()).then(|| Flag {
        rule: "table.incomplete",
        outcome: Outcome::AcceptedWithWarnings,
        reason: format!(
            "{}: fewer or more cells than its table's header",
            counted(number(rows.len()), "table row")
        ),
    })
}
