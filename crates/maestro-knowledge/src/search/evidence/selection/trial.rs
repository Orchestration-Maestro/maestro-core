//! Validates, renders and charges complete delivery trials.

use super::super::{
    budget::count_passages,
    delivery_graph::DeliveryChoice,
    selection_candidate::SelectionCandidate,
    types::{EvidenceCounter, EvidenceError},
};
use super::{
    render::{RenderedTrial, group_selected_spans, render_trial},
    types::{SelectionBudget, check, integrity},
};
use crate::search::assembly_settings::ExpansionMode;
use std::collections::BTreeMap;

/// Renders one trial and measures its full compact passage JSON.
pub(super) fn fits_trial(
    candidates: &[SelectionCandidate<'_>],
    spans: &BTreeMap<usize, DeliveryChoice>,
    budget: &SelectionBudget<'_>,
) -> Result<(bool, RenderedTrial), EvidenceError> {
    check(budget.control)?;
    if matches!(
        budget.counter,
        EvidenceCounter::Utf8Bytes | EvidenceCounter::AnswerBoundUtf8Bytes
    ) && spans.values().any(|choice| {
        choice.ranges().iter().any(|range| {
            range.end.saturating_sub(range.start)
                > usize::try_from(budget.evidence_bytes).unwrap_or(usize::MAX)
        })
    }) {
        group_selected_spans(candidates, spans)
            .map_err(|_| integrity("candidate trial is invalid"))?;
        check(budget.control)?;
        return Ok((false, RenderedTrial::default()));
    }
    let mut rendered = render_trial(candidates, spans)
        .map_err(|_| integrity("candidate trial could not be rendered"))?;
    if budget.expansion == ExpansionMode::ParentChain {
        rendered
            .covered_candidates
            .retain(|index| spans.contains_key(index));
    }
    check(budget.control)?;
    if rendered.passages.len() > budget.max_passages {
        return Ok((false, rendered));
    }
    let tokens = count_passages(
        &rendered.passages,
        budget.counter,
        budget.counter_info,
        budget.evidence_bytes,
    )
    .map_err(EvidenceError::from)?;
    check(budget.control)?;
    Ok((tokens <= budget.evidence_bytes, rendered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::assembly_settings::ParentChainOrder;
    use crate::search::evidence::{
        budget::counter_info,
        delivery_graph::{ChoiceKind, DeliveryChoice, LegacyCanonicalGraph},
        tests::{CandidateSource, candidate, control, prepared},
        types::EvidenceCounter,
    };

    use maestro_canonicalization::{Error, TokenCounter};
    use maestro_kernel::evidence::Span;
    use std::{
        collections::{BTreeMap, BTreeSet},
        slice,
        sync::Arc,
    };

    struct OneToken;
    impl TokenCounter for OneToken {
        fn contract_id(&self) -> &'static str {
            "parent-trial/one-token"
        }
        fn verify(&self) -> Result<(), Error> {
            Ok(())
        }
        fn token_ids(&self, _: &str) -> Result<Vec<u32>, Error> {
            Ok(vec![1])
        }
    }

    #[test]
    fn parent_chain_trials_cover_only_selected_candidates() {
        let markdown = "# Guide\n\nAlpha text. Beta text.\n";
        let (document, sections) = prepared(markdown, "selected-trial.md");
        let source = CandidateSource {
            markdown,
            document: &document,
            sections: &sections,
        };
        let candidates = [
            candidate(source, "Guide", "Alpha", 0, None),
            candidate(source, "Guide", "Beta", 1, None),
        ];
        let first = &candidates[0];
        let second = &candidates[1];
        let spans = BTreeMap::from([(
            0,
            DeliveryChoice::canonical(
                first,
                vec![Span {
                    start: first.required_span.start,
                    end: second.required_span.end,
                }],
                ChoiceKind::Section,
            ),
        )]);
        let counter = EvidenceCounter::Utf8Bytes;

        let result = fits_trial(
            &candidates,
            &spans,
            &SelectionBudget {
                expansion: ExpansionMode::ParentChain,
                graph: &LegacyCanonicalGraph,
                parent_chain_order: ParentChainOrder::default(),
                max_passages: 5,
                evidence_bytes: u32::MAX,
                counter: &counter,
                counter_info: &counter_info(&counter).unwrap(),
                control: &control(),
            },
        )
        .unwrap();

        assert!(result.0);
        assert_eq!(result.1.covered_candidates, BTreeSet::from([0]));
    }

    #[test]
    fn impossible_byte_trials_are_rejected_without_materializing_passages() {
        let markdown = format!("# Guide\n\n{}", "large source ".repeat(100));
        let (document, sections) = prepared(&markdown, "large.md");
        let candidate = candidate(
            CandidateSource {
                markdown: &markdown,
                document: &document,
                sections: &sections,
            },
            "Guide",
            "source",
            0,
            None,
        );
        let choices = BTreeMap::from([(
            0,
            DeliveryChoice::canonical(
                &candidate,
                vec![candidate.expansion.extent],
                ChoiceKind::Section,
            ),
        )]);
        for counter in [
            EvidenceCounter::Utf8Bytes,
            EvidenceCounter::AnswerBoundUtf8Bytes,
            EvidenceCounter::Exact(Arc::new(OneToken)),
        ] {
            let result = fits_trial(
                slice::from_ref(&candidate),
                &choices,
                &SelectionBudget {
                    expansion: ExpansionMode::ParentChain,
                    graph: &LegacyCanonicalGraph,
                    parent_chain_order: ParentChainOrder::default(),
                    max_passages: 5,
                    evidence_bytes: 100,
                    counter: &counter,
                    counter_info: &counter_info(&counter).unwrap(),
                    control: &control(),
                },
            )
            .unwrap();
            if matches!(counter, EvidenceCounter::Exact(_)) {
                assert!(result.0);
                assert_eq!(result.1.passages.len(), 1);
            } else {
                assert!(!result.0);
                assert!(
                    result.1.passages.is_empty(),
                    "an impossible byte trial must not render source text"
                );
            }
        }
    }
}
