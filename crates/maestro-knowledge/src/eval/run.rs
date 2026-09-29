//! A v1 run: every question of a suite, resolved in the generation it
//! evaluates, then retrieved, timed and judged.

use super::{
    error::RunError,
    judge::judge,
    metric::measure,
    reports::{Expected, Header, Report, Schema},
};
use crate::search::evidence::SectionIndex;
use crate::suite::{ExpectedSection, Question, Resolved, Suite};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::evidence::Bundle;
use std::{collections::BTreeMap, time::Instant};

/// The report of `suite` over the generation `header` names, which names the
/// digest of the suite's text.
///
/// First, every section the suite expects is resolved to its ID in the
/// canonical document `documents` gives for its `source_ref`, and every
/// document without sections it expects whole to that document's ID: the
/// document of the evaluated generation, looked up once for all the names of
/// its `source_ref`, one after the other. Then each question, in the suite's
/// order, is retrieved by `retrieve`, timed from the call to its return, and
/// judged; the metrics and their intervals, drawn with the header's seed,
/// close the report.
///
/// # Errors
///
/// [`RunError::Documents`] and [`RunError::Retrieval`] with the caller's own
/// error; [`RunError::NoDocument`] for a `source_ref` the lookup does not
/// find, [`RunError::Unresolved`] for a name that gives no one section, or no
/// document without sections; [`RunError::SectionExtent`] for a resolved
/// section without a valid canonical source extent; and [`RunError::SameSection`]
/// and [`RunError::SameDocument`] for a question that names one section or
/// document twice outside a group, or across groups, all before any retrieval; and
/// [`RunError::OtherGeneration`] for a bundle of another collection or
/// generation than the header's.
pub fn run<E>(
    header: Header,
    suite: &Suite,
    mut documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
    mut retrieve: impl FnMut(&Question) -> Result<Bundle, E>,
) -> Result<Report, RunError<E>> {
    let expected = resolve(suite, &mut documents)?;
    let mut questions = Vec::with_capacity(suite.questions.len());
    for (question, expected) in suite.questions.iter().zip(&expected) {
        let start = Instant::now();
        let bundle = retrieve(question).map_err(|error| RunError::Retrieval {
            question: question.id.clone(),
            error,
        })?;
        let latency_us = u32::try_from(start.elapsed().as_micros()).unwrap_or(u32::MAX);
        if bundle.collection != header.collection || bundle.generation != header.generation {
            return Err(RunError::OtherGeneration {
                question: question.id.clone(),
                collection: bundle.collection,
                generation: bundle.generation,
            });
        }
        questions.push(judge(question, expected, &bundle, latency_us));
    }
    let metrics = measure(&questions, header.seed);
    let degraded_searches = questions.iter().filter(|result| result.degraded).count();
    Ok(Report {
        schema: Schema::V1,
        suite: header.suite,
        suite_digest: suite.digest.clone(),
        collection: header.collection,
        generation: header.generation,
        profiles: header.profiles,
        seed: header.seed,
        questions,
        degraded_searches,
        metrics,
        run_id: None,
        candidate_id: None,
        card_id: None,
        card_digest: None,
        manifest_digest: None,
        planned_repetitions: None,
        planned_warm_ups: None,
        corpus_digest: None,
        input_digest: None,
        mode: None,
        integrity_violation: None,
        cohorts: None,
        subgroups: None,
    })
}

/// A name of a section a question expects: the question's place in its
/// suite, the name's place among its expected sections, the question, and
/// the name.
type Name<'suite> = (usize, usize, &'suite Question, &'suite ExpectedSection);

/// An expected item plus the source identity needed only by the ladder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedExpected {
    /// The existing run report fields.
    pub expected: Expected,
    /// The pinned source revision.
    pub revision_id: String,
    /// The expected section's half-open source extent.
    pub span: Option<[usize; 2]>,
    /// The part of a composed answer it gives, as the suite names it.
    pub component: Option<String>,
}

/// The sections and documents each question of `suite` expects, unranked,
/// in the suite's order, each resolved in the canonical document `documents`
/// gives for its `source_ref`, which is looked up once and dropped once its
/// names resolve.
pub(super) fn resolve<E>(
    suite: &Suite,
    documents: &mut impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
) -> Result<Vec<Vec<Expected>>, RunError<E>> {
    Ok(resolve_details(suite, documents)?
        .into_iter()
        .map(|row| row.into_iter().map(|item| item.expected).collect())
        .collect())
}

/// Resolves expected items with the revision and extent used only by the ladder.
pub(super) fn resolve_details<E>(
    suite: &Suite,
    documents: &mut impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
) -> Result<Vec<Vec<ResolvedExpected>>, RunError<E>> {
    let mut named: BTreeMap<&str, Vec<Name<'_>>> = BTreeMap::new();
    for (question_index, question) in suite.questions.iter().enumerate() {
        for (name_index, name) in question.expected.iter().enumerate() {
            let names = named.entry(name.source_ref.as_str()).or_default();
            names.push((question_index, name_index, question, name));
        }
    }
    let mut resolved: Vec<Vec<Option<ResolvedExpected>>> = suite
        .questions
        .iter()
        .map(|question| vec![None; question.expected.len()])
        .collect();
    for (source_ref, names) in named {
        let document = documents(source_ref).map_err(|error| RunError::Documents {
            source_ref: source_ref.to_owned(),
            error,
        })?;
        let section_index = document.as_ref().and_then(|document| {
            names
                .iter()
                .any(|(_, _, _, name)| !name.heading_path.is_empty())
                .then(|| {
                    SectionIndex::new_from_length(
                        document,
                        document.original_markdown_reference.byte_length,
                    )
                })
        });
        for (question_index, name_index, question, name) in names {
            let expected = resolve_name(document.as_ref(), section_index.as_ref(), question, name)?;
            let place = resolved
                .get_mut(question_index)
                .and_then(|places| places.get_mut(name_index));
            if let Some(place) = place {
                *place = Some(expected);
            }
        }
    }
    suite
        .questions
        .iter()
        .zip(resolved)
        .map(|(question, places)| distinct(question, places.into_iter().flatten().collect()))
        .collect()
}

/// The section, or the document without sections, that `name`, of
/// `question`, gives in `document`, the canonical document of its
/// `source_ref` if the lookup found one; unranked.
fn resolve_name<E>(
    document: Option<&CanonicalDocument>,
    section_index: Option<&Result<SectionIndex, String>>,
    question: &Question,
    name: &ExpectedSection,
) -> Result<ResolvedExpected, RunError<E>> {
    let document = document.ok_or_else(|| RunError::NoDocument {
        question: question.id.clone(),
        source_ref: name.source_ref.clone(),
    })?;
    let resolved = name
        .resolve(document)
        .map_err(|reason| RunError::Unresolved {
            question: question.id.clone(),
            source_ref: name.source_ref.clone(),
            heading_path: name.heading_path.clone(),
            reason,
        })?;
    let (section_id, span) = match resolved {
        Resolved::Section(section) => {
            let extent = section_index
                .ok_or_else(|| RunError::SectionExtent {
                    question: question.id.clone(),
                    section_id: section.section_id.clone(),
                    reason: "section index was not built".to_owned(),
                })?
                .as_ref()
                .map_err(|reason| RunError::SectionExtent {
                    question: question.id.clone(),
                    section_id: section.section_id.clone(),
                    reason: reason.clone(),
                })?
                .section_extent(&section.section_id)
                .ok_or_else(|| RunError::SectionExtent {
                    question: question.id.clone(),
                    section_id: section.section_id.clone(),
                    reason: "canonical section has no source extent".to_owned(),
                })?;
            (
                Some(section.section_id.clone()),
                Some([extent.start, extent.end]),
            )
        }
        Resolved::Document(_) => (None, None),
    };
    Ok(ResolvedExpected {
        expected: Expected {
            document_id: document.document_id.clone(),
            section_id,
            group: name.group.clone(),
            rank: None,
        },
        revision_id: document.revision_id.clone(),
        span,
        component: name.component.clone(),
    })
}

/// `expected`, the sections and documents `question` expects, unless one is
/// named twice outside a group or across groups.
fn distinct<E>(
    question: &Question,
    expected: Vec<ResolvedExpected>,
) -> Result<Vec<ResolvedExpected>, RunError<E>> {
    let mut seen = BTreeMap::new();
    for item in &expected {
        let identity = (
            item.expected.document_id.as_str(),
            item.expected.section_id.as_deref(),
        );
        let group = item.expected.group.as_deref();
        if let Some(previous) = seen.get(&identity) {
            if group.is_some() && *previous == group {
                continue;
            }
            return match &item.expected.section_id {
                Some(section_id) => Err(RunError::SameSection {
                    question: question.id.clone(),
                    section_id: section_id.clone(),
                }),
                None => Err(RunError::SameDocument {
                    question: question.id.clone(),
                    document_id: item.expected.document_id.clone(),
                }),
            };
        }
        seen.insert(identity, group);
    }
    Ok(expected)
}
