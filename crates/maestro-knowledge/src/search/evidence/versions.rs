//! Numeric release ordering and exact-section version collapse.

use super::families::{CandidateFamily, corresponds};
use maestro_kernel::evidence::Alternate;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// One loaded candidate considered for documentary-version collapse.
#[derive(Debug, Clone)]
pub(crate) struct VersionCandidate {
    /// Manifest-authorized correspondence family and canonical section location.
    pub(crate) family: CandidateFamily,
    /// Original reranker position for deterministic mirror selection.
    pub(crate) input_position: usize,
    /// Candidate source revision.
    pub(crate) revision_id: String,
    /// Canonical section that supplied the candidate, if any.
    pub(crate) section_id: Option<String>,
    /// Effective literal version, if present.
    pub(crate) version: Option<String>,
    /// Exact full section bytes used for conservative equality.
    pub(crate) section_text: String,
    /// Conflicting evidence is retained independently.
    pub(crate) conflict_member: bool,
}

/// Candidates to suppress and provenance to attach to each retained primary.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct VersionCollapse {
    /// Indices whose text is represented only as a retained section alternate.
    pub(crate) suppressed: BTreeSet<usize>,
    /// Older versions, keyed by the stable retained candidate index.
    pub(crate) alternates: BTreeMap<usize, Vec<Alternate>>,
    /// At least one otherwise-collapsible class has no numeric ordering.
    pub(crate) latest_undetermined: bool,
}

/// The numeric latest candidates and their strictly older members.
struct VersionPartition {
    /// Winning candidates, including same-version mirrors.
    winners: Vec<usize>,
    /// Candidates represented as alternates of the stable winner.
    older: Vec<usize>,
}

/// Compares dot-separated ASCII integer components without integer overflow.
pub(crate) fn compare_numeric_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = components(left)?;
    let right = components(right)?;
    for index in 0..left.len().max(right.len()) {
        let left_component = left.get(index).copied().unwrap_or("0");
        let right_component = right.get(index).copied().unwrap_or("0");
        let order = compare_component(left_component, right_component);
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

/// Collapses only identical, same-location sections with a provable numeric latest.
pub(crate) fn collapse_versions(
    candidates: &[VersionCandidate],
    enabled: bool,
) -> Result<VersionCollapse, String> {
    if !enabled {
        return Ok(VersionCollapse::default());
    }
    validate_candidates(candidates)?;
    let mut result = VersionCollapse::default();
    for group in equivalent_groups(candidates)? {
        collapse_group(candidates, &group, &mut result)?;
    }
    Ok(result)
}

/// Validates source identities before the bounded pairwise family comparison.
fn validate_candidates(candidates: &[VersionCandidate]) -> Result<(), String> {
    if candidates.iter().any(|candidate| {
        candidate.revision_id.trim().is_empty()
            || candidate.family.document_id.trim().is_empty()
            || candidate
                .family
                .near_group_ids
                .iter()
                .any(|group| group.trim().is_empty())
            || candidate
                .section_id
                .as_ref()
                .is_some_and(|section| section.trim().is_empty())
    }) {
        return Err("version candidate identity is invalid".to_owned());
    }
    Ok(())
}

/// Builds exact-text family classes; the query is capped at 120 candidates.
fn equivalent_groups(candidates: &[VersionCandidate]) -> Result<Vec<Vec<usize>>, String> {
    // ponytail: O(n²) for the 120-candidate request cap; partition first if that cap rises.
    let mut remaining: BTreeSet<_> = (0..candidates.len()).collect();
    let mut groups = Vec::new();
    while let Some(start) = remaining.iter().next().copied() {
        remaining.remove(&start);
        let first = candidates
            .get(start)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        if first.conflict_member || first.section_id.is_none() {
            continue;
        }
        let mut group = vec![start];
        let mut pending = vec![start];
        while let Some(current_index) = pending.pop() {
            let current = candidates
                .get(current_index)
                .ok_or_else(|| "version candidate index is invalid".to_owned())?;
            let matching = matching_candidates(candidates, current, &remaining)?;
            for index in matching.into_iter().filter(|index| remaining.remove(index)) {
                pending.push(index);
                group.push(index);
            }
        }
        groups.push(group);
    }
    Ok(groups)
}

/// Returns unvisited candidates with the current section's exact family identity.
fn matching_candidates(
    candidates: &[VersionCandidate],
    current: &VersionCandidate,
    remaining: &BTreeSet<usize>,
) -> Result<Vec<usize>, String> {
    let mut matching = Vec::new();
    for index in remaining {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        if !candidate.conflict_member
            && candidate.section_id.is_some()
            && candidate.section_text == current.section_text
            && corresponds(&current.family, &candidate.family)
        {
            matching.push(*index);
        }
    }
    Ok(matching)
}

/// Suppresses only strictly older numerically ordered sections in one exact class.
fn collapse_group(
    candidates: &[VersionCandidate],
    group: &[usize],
    result: &mut VersionCollapse,
) -> Result<(), String> {
    let Some((first_index, rest)) = group.split_first() else {
        return Ok(());
    };
    if !spans_multiple_revisions(candidates, *first_index, rest)? {
        return Ok(());
    }
    let Some(latest) = latest_version(candidates, group, result)? else {
        return Ok(());
    };
    let Some(partition) = partition_versions(candidates, group, latest, result)? else {
        return Ok(());
    };
    if partition.older.is_empty() {
        return Ok(());
    }
    let primary = primary_candidate(candidates, partition.winners)?;
    let alternates = older_alternates(candidates, partition.older, result)?;
    result.alternates.insert(primary, alternates);
    Ok(())
}

/// Whether the class represents more than one distinct revision.
fn spans_multiple_revisions(
    candidates: &[VersionCandidate],
    first_index: usize,
    rest: &[usize],
) -> Result<bool, String> {
    let first_revision = candidates
        .get(first_index)
        .ok_or_else(|| "version candidate index is invalid".to_owned())?
        .revision_id
        .as_str();
    for index in rest {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        if candidate.revision_id != first_revision {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The greatest valid numeric version, or `None` when a class is unorderable.
fn latest_version<'a>(
    candidates: &'a [VersionCandidate],
    group: &[usize],
    result: &mut VersionCollapse,
) -> Result<Option<&'a str>, String> {
    let mut latest: Option<&str> = None;
    for index in group {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        let Some(version) = candidate.version.as_deref() else {
            result.latest_undetermined = true;
            return Ok(None);
        };
        match latest {
            None => latest = Some(version),
            Some(current) => match compare_numeric_versions(version, current) {
                Some(Ordering::Greater) => latest = Some(version),
                Some(Ordering::Less | Ordering::Equal) => {}
                None => {
                    result.latest_undetermined = true;
                    return Ok(None);
                }
            },
        }
    }
    Ok(latest)
}

/// Partitions a version class into numerically latest and strictly older members.
fn partition_versions(
    candidates: &[VersionCandidate],
    group: &[usize],
    latest: &str,
    result: &mut VersionCollapse,
) -> Result<Option<VersionPartition>, String> {
    let mut winners = Vec::new();
    let mut older = Vec::new();
    for index in group {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        let Some(version) = candidate.version.as_deref() else {
            result.latest_undetermined = true;
            return Ok(None);
        };
        match compare_numeric_versions(version, latest) {
            Some(Ordering::Equal) => winners.push(*index),
            Some(Ordering::Less) => older.push(*index),
            Some(Ordering::Greater) => {
                return Err("latest version selection is inconsistent".to_owned());
            }
            None => {
                result.latest_undetermined = true;
                return Ok(None);
            }
        }
    }
    Ok(Some(VersionPartition { winners, older }))
}

/// Chooses the stable representative among numerically latest mirrors.
fn primary_candidate(
    candidates: &[VersionCandidate],
    mut winners: Vec<usize>,
) -> Result<usize, String> {
    winners.sort_by_key(|index| {
        candidates.get(*index).map(|candidate| {
            (
                candidate.input_position,
                candidate.revision_id.as_str(),
                candidate.section_id.as_deref().unwrap_or_default(),
            )
        })
    });
    winners
        .first()
        .copied()
        .ok_or_else(|| "latest version has no retained candidate".to_owned())
}

/// Records suppressed provenance as sorted, distinct alternates.
fn older_alternates(
    candidates: &[VersionCandidate],
    older: Vec<usize>,
    result: &mut VersionCollapse,
) -> Result<Vec<Alternate>, String> {
    let mut alternates = Vec::new();
    for index in older {
        let candidate = candidates
            .get(index)
            .ok_or_else(|| "version candidate index is invalid".to_owned())?;
        let version = candidate
            .version
            .clone()
            .ok_or_else(|| "older version is missing its label".to_owned())?;
        let section_id = candidate
            .section_id
            .clone()
            .ok_or_else(|| "older version is missing its section".to_owned())?;
        result.suppressed.insert(index);
        alternates.push(Alternate {
            version: Some(version),
            section_id,
        });
    }
    alternates.sort_by(|left, right| {
        left.version
            .cmp(&right.version)
            .then_with(|| left.section_id.cmp(&right.section_id))
    });
    alternates.dedup();
    Ok(alternates)
}

/// Splits a valid nonempty dot-separated numeric version.
fn components(version: &str) -> Option<Vec<&str>> {
    if version.is_empty() {
        return None;
    }
    let components: Vec<_> = version.split('.').collect();
    components
        .iter()
        .all(|component| {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        })
        .then_some(components)
}

/// Compares components by normalized decimal length and then digits.
fn compare_component(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    let left = if left.is_empty() { "0" } else { left };
    let right = if right.is_empty() { "0" } else { right };
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::evidence::families::ConflictContext;

    #[test]
    fn already_removed_candidate_indexes_are_not_grouped_twice() {
        let family = |document_id: &str| CandidateFamily {
            document_id: document_id.to_owned(),
            near_group_ids: BTreeSet::from(["near".to_owned()]),
            section_path: vec!["Guide".to_owned()],
            occurrence: 1,
            context: ConflictContext::default(),
        };
        let candidates = [
            VersionCandidate {
                family: family("doc-a"),
                input_position: 0,
                revision_id: "rev-a".to_owned(),
                section_id: Some("section-a".to_owned()),
                version: Some("1".to_owned()),
                section_text: "same".to_owned(),
                conflict_member: false,
            },
            VersionCandidate {
                family: family("doc-b"),
                input_position: 1,
                revision_id: "rev-b".to_owned(),
                section_id: Some("section-b".to_owned()),
                version: Some("2".to_owned()),
                section_text: "same".to_owned(),
                conflict_member: false,
            },
            VersionCandidate {
                family: family("doc-c"),
                input_position: 2,
                revision_id: "rev-c".to_owned(),
                section_id: Some("section-c".to_owned()),
                version: Some("3".to_owned()),
                section_text: "same".to_owned(),
                conflict_member: false,
            },
        ];

        assert_eq!(equivalent_groups(&candidates).unwrap(), [vec![0, 1, 2]]);
    }
}
