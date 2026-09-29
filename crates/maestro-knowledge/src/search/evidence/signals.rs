//! Produces reading order, trace metadata, and deterministic known gaps.

use maestro_canonicalization::{BlockAttributes, CanonicalDocument};
use maestro_kernel::evidence::{Passage, Span, Trace};
use maestro_kernel::retrieval::contains_identifier;
use std::collections::{BTreeMap, BTreeSet};

use super::{
    delivery_graph::PrimaryContribution,
    sections::{block_span, contains, valid_span},
    spans::SeedSpan,
};

/// Passage paired with its best retained input position.
pub(crate) struct PassageOrder {
    /// Passage whose final reading position is assigned here.
    pub(crate) passage: Passage,
    /// Earliest candidate position supporting the passage.
    pub(crate) input_position: usize,
}

/// One structured disagreement whose values share a final passage.
pub(crate) struct WithinPassageConflict<'a> {
    /// Final one-based passage number.
    pub(crate) passage_number: u32,
    /// Exact table entity or parameter.
    pub(crate) entity: &'a str,
    /// Exact table attribute.
    pub(crate) attribute: &'a str,
}

/// Authorized warning details to attach to a final passage's gaps.
pub(crate) struct SourceWarning<'a> {
    /// Final one-based passage number.
    pub(crate) passage_number: u32,
    /// Accepted-with-warnings disposition reasons.
    pub(crate) disposition_reasons: &'a [String],
    /// Applied disposition rule IDs.
    pub(crate) rule_ids: &'a [String],
    /// Canonicalization warning codes.
    pub(crate) warning_codes: &'a [String],
}

/// Which optional evidence omissions should produce a gap.
#[derive(Default)]
pub(crate) struct OmissionStatus {
    /// Table-prefix candidates the evidence or passage budget omitted.
    pub(crate) table_prefix_omissions: usize,
    /// Some candidate evidence did not fit the evidence or passage budget.
    pub(crate) evidence: bool,
    /// A conflict unit could not fit as a whole.
    pub(crate) conflict: bool,
}

/// Inputs needed to derive deterministic, authorized known-gap messages.
#[derive(Default)]
pub(crate) struct GapInput<'a> {
    /// Gaps inherited from the query/route handoff.
    pub(crate) inherited: Vec<String>,
    /// Final source passages in reading order.
    pub(crate) passages: &'a [Passage],
    /// Whether the request retained an inventory.
    pub(crate) has_inventory: bool,
    /// Whether any candidate could not be resolved in current scopes.
    pub(crate) unresolved_candidate: bool,
    /// Distinct required identifiers from query understanding.
    pub(crate) identifiers: &'a [String],
    /// Authorized proposed passage text used to distinguish budget omissions.
    pub(crate) candidate_texts: &'a [String],
    /// Exact effective version filter, if supplied.
    pub(crate) requested_version: Option<&'a str>,
    /// Whether one or more families could not be ordered as latest.
    pub(crate) latest_undetermined: bool,
    /// Evidence and conflict omission categories.
    pub(crate) omissions: OmissionStatus,
    /// Structured disagreements combined into a single final passage.
    pub(crate) within_passage_conflicts: &'a [WithinPassageConflict<'a>],
    /// Warnings from authorized retained revisions.
    pub(crate) source_warnings: &'a [SourceWarning<'a>],
}

/// Data required to construct one passage's trace entry.
#[derive(Clone, Copy)]
pub(crate) struct TraceInput<'a> {
    /// Final one-based passage number.
    pub(crate) number: u32,
    /// Revision containing the passage.
    pub(crate) revision_id: &'a str,
    /// Final source span of the passage.
    pub(crate) span: Span,
    /// All candidate seeds eligible for this trace.
    pub(crate) seeds: &'a [SeedSpan],
    /// Seed-linked exact primary contributions for disjoint retrieval views.
    pub(crate) primary: &'a [PrimaryContribution],
    /// Explicit parent-context seed references, validated against the completed bundle.
    pub(crate) parent_context_of: &'a [String],
    /// Canonical source document used for structural signals.
    pub(crate) document: &'a CanonicalDocument,
    /// Authoritative original Markdown text.
    pub(crate) markdown: &'a str,
}

/// Input group for ordering passages within one document revision.
struct PassageGroup {
    /// Best input position represented by this group.
    input_position: usize,
    /// Passages in the group before source ordering.
    passages: Vec<Passage>,
}

/// Groups passages by revision, sorts by source span, and assigns numbers.
pub(crate) fn order_passages(passages: Vec<PassageOrder>) -> Result<Vec<Passage>, String> {
    let mut groups: BTreeMap<(String, String), PassageGroup> = BTreeMap::new();
    for item in passages {
        let passage = item.passage;
        let key = (passage.document_id.clone(), passage.revision_id.clone());
        let group = groups.entry(key).or_insert_with(|| PassageGroup {
            input_position: item.input_position,
            passages: Vec::new(),
        });
        group.input_position = group.input_position.min(item.input_position);
        group.passages.push(passage);
    }

    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|(left_key, left), (right_key, right)| {
        left.input_position
            .cmp(&right.input_position)
            .then_with(|| left_key.cmp(right_key))
    });
    let mut ordered = Vec::new();
    for (_, mut group) in groups {
        group
            .passages
            .sort_by_key(|passage| (passage.span.start, passage.span.end));
        ordered.extend(group.passages);
    }
    for (index, passage) in ordered.iter_mut().enumerate() {
        let number = index
            .checked_add(1)
            .and_then(|number| u32::try_from(number).ok())
            .ok_or_else(|| "passage numbering exceeds u32".to_owned())?;
        passage.n = number;
    }
    Ok(ordered)
}

/// Builds trace provenance and the complete ordered-list structural signal.
pub(crate) fn trace_for_passage(input: &TraceInput<'_>) -> Result<Trace, String> {
    let TraceInput {
        number,
        revision_id,
        span,
        seeds,
        document,
        markdown,
        parent_context_of,
        primary,
    } = *input;
    if !valid_span(span, markdown) {
        return Err("passage span is invalid".to_owned());
    }
    let mut chunk_ids = BTreeSet::new();
    let mut routes = BTreeSet::new();
    let mut score = None;
    for seed in seeds.iter().filter(|seed| seed.revision_id == revision_id) {
        if seed.span.start >= seed.span.end || !valid_span(seed.span, markdown) {
            return Err("candidate seed span is invalid".to_owned());
        }
        let contained = contains(span, seed.span)
            || primary.iter().any(|part| {
                part.chunk_id == seed.chunk_id
                    && contains(seed.span, part.span)
                    && contains(span, part.span)
            });
        if !contained && !parent_context_of.contains(&seed.chunk_id) {
            continue;
        }
        if seed.chunk_id.trim().is_empty() {
            return Err("candidate chunk ID is blank".to_owned());
        }
        if let Some(candidate_score) = seed.score {
            if !candidate_score.is_finite() {
                return Err("candidate rerank score is non-finite".to_owned());
            }
            score = Some(score.map_or(candidate_score, |best: f64| best.max(candidate_score)));
        }
        if contained {
            chunk_ids.insert(seed.chunk_id.clone());
        }
        routes.extend(seed.routes.iter().map(|route| route.name().to_owned()));
    }
    if chunk_ids.is_empty() && parent_context_of.is_empty() {
        return Err("passage has no primary chunk references".to_owned());
    }
    let procedural =
        document
            .blocks
            .iter()
            .try_fold(false, |found, block| -> Result<bool, String> {
                if found
                    || !matches!(
                        &block.structured_content.attributes,
                        BlockAttributes::List { start: Some(_) }
                    )
                {
                    return Ok(found);
                }
                let list_span = block_span(block, markdown)?;
                Ok(list_span.is_some_and(|list_span| contains(span, list_span)))
            })?;
    Ok(Trace {
        parent_context_of: parent_context_of.to_vec(),
        n: number,
        score,
        routes: routes.into_iter().collect(),
        chunk_ids: chunk_ids.into_iter().collect(),
        procedural,
    })
}

/// Appends deterministic evidence limitations after inherited gaps.
pub(crate) fn build_known_gaps(mut input: GapInput<'_>) -> Result<Vec<String>, String> {
    let mut additions = Vec::new();
    if input.passages.is_empty() {
        additions.push(
            if input.has_inventory {
                concat!(
                    "No source passages were returned; inventory totals are ",
                    "independent of supporting passages."
                )
            } else {
                "No accessible evidence was returned for this query in the pinned generation."
            }
            .to_owned(),
        );
    }
    if input.unresolved_candidate {
        additions.push(
            "One or more candidate passages could not be resolved in the current scopes."
                .to_owned(),
        );
    }

    append_identifier_gaps(&input, &mut additions)?;
    if let Some(version) = input.requested_version
        && !input
            .passages
            .iter()
            .any(|passage| passage.version.as_deref() == Some(version))
    {
        additions.push(format!(
            "Requested version {} is not represented in returned passages.",
            quote(version)?
        ));
    }
    if input.latest_undetermined {
        additions.push(
            "Latest version could not be determined for one or more candidate families.".to_owned(),
        );
    }

    for passage in input.passages.iter().filter(|passage| passage.windowed) {
        additions.push(format!(
            "Passage {} is a window; surrounding section text is omitted.",
            passage.n
        ));
    }
    if input.omissions.table_prefix_omissions > 0 {
        additions.push(format!(
            "Table-prefix candidates omitted by the evidence or passage budget: {}.",
            input.omissions.table_prefix_omissions
        ));
    }
    if input.omissions.evidence {
        additions.push(
            "Some candidate evidence was omitted by the evidence or passage budget.".to_owned(),
        );
    }
    if input.omissions.conflict {
        additions.push(
            concat!(
                "Conflicting values could not be retained together within the ",
                "evidence or passage budget."
            )
            .to_owned(),
        );
    }
    let mut within_passage_conflicts: Vec<_> = input.within_passage_conflicts.iter().collect();
    within_passage_conflicts
        .sort_by_key(|conflict| (conflict.passage_number, conflict.entity, conflict.attribute));
    for conflict in within_passage_conflicts {
        additions.push(format!(
            "Passage {} contains different explicit values for {} / {}.",
            conflict.passage_number,
            quote(conflict.entity)?,
            quote(conflict.attribute)?
        ));
    }
    let mut warnings: Vec<_> = input.source_warnings.iter().collect();
    warnings.sort_by_key(|warning| warning.passage_number);
    for warning in warnings {
        for reason in warning.disposition_reasons {
            additions.push(format!(
                "Passage {} source warning: {}",
                warning.passage_number,
                quote(reason)?
            ));
        }
        for detail in warning.rule_ids.iter().chain(warning.warning_codes) {
            additions.push(format!(
                "Passage {} source warning: {detail}",
                warning.passage_number
            ));
        }
    }
    append_distinct_gaps(&mut input.inherited, additions);
    Ok(input.inherited)
}

/// Adds identifier omissions when candidates or final passages cannot support them.
fn append_identifier_gaps(input: &GapInput<'_>, additions: &mut Vec<String>) -> Result<(), String> {
    for identifier in input
        .identifiers
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
    {
        if input
            .passages
            .iter()
            .any(|passage| contains_identifier(&passage.text, identifier))
        {
            continue;
        }
        let quoted = quote(identifier)?;
        let found_in_candidates = input
            .candidate_texts
            .iter()
            .any(|text| contains_identifier(text, identifier));
        let message = if found_in_candidates {
            format!(
                "Required identifier {quoted} was found in candidates but \
                 is absent after budgeting."
            )
        } else {
            format!("Required identifier {quoted} is absent from returned passages.")
        };
        additions.push(message);
    }
    Ok(())
}

/// Appends only unseen messages without changing inherited gap order.
pub(crate) fn append_distinct_gaps(
    gaps: &mut Vec<String>,
    additions: impl IntoIterator<Item = String>,
) {
    let mut seen: BTreeSet<_> = gaps.iter().cloned().collect();
    gaps.extend(additions.into_iter().filter(|gap| seen.insert(gap.clone())));
}

/// Produces JSON-escaped text for an interpolated gap identifier.
fn quote(value: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|_| "gap value could not be quoted".to_owned())
}
