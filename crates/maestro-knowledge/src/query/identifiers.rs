//! Identifier detection and overlap resolution.

use super::identifier_numbers::{error_code_candidates, port_candidates, version_candidates};
use super::identifier_patterns::{command_candidates, parameter_candidates, path_candidates};
use super::identifier_types::{Candidate, Identifier, overlaps, priority};

/// Collects every family candidate before overlap arbitration.
pub(super) fn candidates(text: &str) -> Vec<Candidate> {
    let parameters = parameter_candidates(text);
    let mut candidates = command_candidates(text, &parameters);
    candidates.extend(path_candidates(text));
    candidates.extend(version_candidates(text));
    candidates.extend(port_candidates(text));
    candidates.extend(error_code_candidates(text));
    candidates.extend(parameters);
    candidates
}

/// Finds non-overlapping identifier spans and returns them in source order.
pub(super) fn find(text: &str) -> Vec<Identifier> {
    let mut candidates = candidates(text);
    candidates.sort_by(|left, right| {
        priority(left.family)
            .cmp(&priority(right.family))
            .then_with(|| left.start.cmp(&right.start))
    });

    let mut accepted = Vec::new();
    for candidate in candidates {
        if accepted
            .iter()
            .any(|known: &Candidate| overlaps(candidate, *known))
        {
            continue;
        }
        accepted.push(candidate);
    }
    accepted.sort_by_key(|candidate| candidate.start);
    accepted
        .into_iter()
        .filter_map(|candidate| {
            text.get(candidate.start..candidate.end)
                .map(|span| Identifier {
                    family: candidate.family,
                    text: span.to_owned(),
                })
        })
        .collect()
}
