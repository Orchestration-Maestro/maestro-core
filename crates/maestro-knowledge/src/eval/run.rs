//! A run: every question of a suite, resolved in the generation it
//! evaluates, then retrieved, timed and judged.

use super::{
    error::RunError,
    judge::judge,
    metric::measure,
    report::{Header, Report, Schema},
};
use crate::suite::{ExpectedSection, Question, Suite};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::evidence::Bundle;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

/// The report of `suite` over the generation `header` names.
///
/// First, every section the suite expects is resolved to its ID in the
/// canonical document `documents` gives for its `source_ref`: the document of
/// the evaluated generation, looked up once for all the names of its
/// `source_ref`, one after the other. Then each question, in the suite's
/// order, is retrieved by `retrieve`, timed from the call to its return, and
/// judged; the metrics and their intervals, drawn with the header's seed,
/// close the report.
///
/// # Errors
///
/// [`RunError::Documents`] and [`RunError::Retrieval`] with the caller's own
/// error; [`RunError::NoDocument`] for a `source_ref` the lookup does not
/// find, [`RunError::Unresolved`] for a name that gives no one section and
/// [`RunError::SameSection`] for a question that names one section twice, all
/// before any retrieval; and [`RunError::OtherGeneration`] for a bundle of
/// another collection or generation than the header's.
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
        collection: header.collection,
        generation: header.generation,
        profiles: header.profiles,
        seed: header.seed,
        questions,
        degraded_searches,
        metrics,
    })
}

/// A name of a section a question expects: the question's place in its
/// suite, the name's place among its expected sections, the question, and
/// the name.
type Name<'suite> = (usize, usize, &'suite Question, &'suite ExpectedSection);

/// The section IDs each question of `suite` expects, in the suite's order,
/// each resolved in the canonical document `documents` gives for its
/// `source_ref`, which is looked up once and dropped once its names resolve.
fn resolve<E>(
    suite: &Suite,
    documents: &mut impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
) -> Result<Vec<Vec<String>>, RunError<E>> {
    let mut named: BTreeMap<&str, Vec<Name<'_>>> = BTreeMap::new();
    for (question_index, question) in suite.questions.iter().enumerate() {
        for (name_index, name) in question.expected.iter().enumerate() {
            let names = named.entry(name.source_ref.as_str()).or_default();
            names.push((question_index, name_index, question, name));
        }
    }
    let mut resolved: Vec<Vec<Option<String>>> = suite
        .questions
        .iter()
        .map(|question| vec![None; question.expected.len()])
        .collect();
    for (source_ref, names) in named {
        let document = documents(source_ref).map_err(|error| RunError::Documents {
            source_ref: source_ref.to_owned(),
            error,
        })?;
        for (question_index, name_index, question, name) in names {
            let section_id = resolve_name(document.as_ref(), question, name)?;
            let place = resolved
                .get_mut(question_index)
                .and_then(|ids| ids.get_mut(name_index));
            if let Some(place) = place {
                *place = Some(section_id);
            }
        }
    }
    suite
        .questions
        .iter()
        .zip(resolved)
        .map(|(question, ids)| distinct(question, ids.into_iter().flatten().collect()))
        .collect()
}

/// The ID of the section `name`, of `question`, gives in `document`, the
/// canonical document of its `source_ref` if the lookup found one.
fn resolve_name<E>(
    document: Option<&CanonicalDocument>,
    question: &Question,
    name: &ExpectedSection,
) -> Result<String, RunError<E>> {
    let document = document.ok_or_else(|| RunError::NoDocument {
        question: question.id.clone(),
        source_ref: name.source_ref.clone(),
    })?;
    let section = name
        .resolve(document)
        .map_err(|reason| RunError::Unresolved {
            question: question.id.clone(),
            source_ref: name.source_ref.clone(),
            heading_path: name.heading_path.clone(),
            reason,
        })?;
    Ok(section.section_id.clone())
}

/// `ids`, the section IDs `question` expects, unless two are one section.
fn distinct<E>(question: &Question, ids: Vec<String>) -> Result<Vec<String>, RunError<E>> {
    let mut seen = BTreeSet::new();
    match ids.iter().find(|id| !seen.insert(id.as_str())) {
        Some(repeated) => Err(RunError::SameSection {
            question: question.id.clone(),
            section_id: repeated.clone(),
        }),
        None => Ok(ids),
    }
}
