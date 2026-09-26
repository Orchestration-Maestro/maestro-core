//! The public synthetic collection, `tests/fixtures/synthetic`, which stands in
//! for the private corpus in public CI (ADR-0009): its declaration, its corpus
//! manifest and its suite hold under their contracts, every document
//! canonicalizes without a blocking finding, and every section the suite
//! expects is exactly one section of its document.
#![cfg(test)]

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, Severity, canonicalize};
use maestro_kernel::{artifact::Digest, binding::Bindings};
use maestro_knowledge::{
    collection::{Declaration, Visibility},
    corpus::Entry,
    suite::{ExpectedSection, Language, Question, Suite},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The fixture directory, `tests/fixtures/synthetic` at the workspace root,
/// two levels above this crate's manifest directory.
fn fixture() -> PathBuf {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    workspace.join("tests").join("fixtures").join("synthetic")
}

/// The collection's declaration.
fn declaration() -> Declaration {
    let text = fs::read_to_string(fixture().join("collection.json")).unwrap();
    text.parse().unwrap()
}

/// The bindings the tests give: `synthetic_root`, to the fixture directory.
fn bindings() -> Bindings {
    format!("synthetic_root = '{}'\n", fixture().display())
        .parse()
        .unwrap()
}

/// The corpus manifest of the collection's one source.
fn manifest() -> PathBuf {
    let declaration = declaration();
    let resolved = declaration.manifest_paths(&bindings()).unwrap();
    let [(_, path)] = resolved.as_slice() else {
        panic!("one source: {resolved:?}");
    };
    path.clone()
}

/// The directory the manifest's paths are relative to: its own.
fn corpus() -> PathBuf {
    manifest().parent().unwrap().to_path_buf()
}

/// Every line of the manifest, read as the importer reads it.
fn entries() -> Vec<Entry> {
    let text = fs::read_to_string(manifest()).unwrap();
    text.lines()
        .map(|line| {
            line.parse()
                .unwrap_or_else(|error| panic!("{line}: {error}"))
        })
        .collect()
}

/// Every document of the manifest, canonicalized with the provenance its line
/// gives.
fn documents() -> Vec<(Entry, CanonicalDocument)> {
    let corpus = corpus();
    entries()
        .into_iter()
        .map(|entry| {
            let markdown = fs::read_to_string(entry.path.under(&corpus)).unwrap();
            let mut input = CanonicalizeInput::new(&markdown, entry.path.as_str());
            input.metadata.source_reference = Some(entry.source_ref.clone());
            input.metadata.title = Some(entry.title.clone());
            let document = canonicalize(input).unwrap();
            (entry, document)
        })
        .collect()
}

/// The questions of the suite the declaration names.
fn questions() -> Vec<Question> {
    let path = declaration().evals.suite.under(&fixture());
    let suite: Suite = fs::read_to_string(path).unwrap().parse().unwrap();
    suite.questions
}

/// The ID of the one section `expected` names among `documents`, or why it
/// names none or several.
fn resolve(
    documents: &[(Entry, CanonicalDocument)],
    expected: &ExpectedSection,
) -> Result<String, String> {
    let matching: Vec<&str> = documents
        .iter()
        .filter(|(entry, _)| entry.source_ref == expected.source_ref)
        .flat_map(|(_, document)| &document.sections)
        .filter(|section| section.heading_path == expected.heading_path)
        .map(|section| section.section_id.as_str())
        .collect();
    let named = match (expected.occurrence, matching.as_slice()) {
        (None, [only]) => Some(*only),
        (Some(occurrence), repeated) if repeated.len() > 1 => {
            let index = usize::try_from(occurrence.get()).unwrap() - 1;
            repeated.get(index).copied()
        }
        _ => None,
    };
    named.map(str::to_owned).ok_or_else(|| {
        format!(
            "{} {:?} occurrence {:?} matches {} sections",
            expected.source_ref,
            expected.heading_path,
            expected.occurrence,
            matching.len()
        )
    })
}

/// The language a document is written in: its path's first directory.
fn written_in(entry: &Entry) -> Language {
    match entry.path.as_str().split('/').next() {
        Some("fr") => Language::Fr,
        Some("en") => Language::En,
        _ => panic!("{} is under neither fr/ nor en/", entry.path.as_str()),
    }
}

/// The languages of the documents whose sections `question` expects.
fn answered_in(documents: &[(Entry, CanonicalDocument)], question: &Question) -> Vec<Language> {
    documents
        .iter()
        .filter(|(entry, _)| {
            question
                .expected
                .iter()
                .any(|expected| expected.source_ref == entry.source_ref)
        })
        .map(|(entry, _)| written_in(entry))
        .collect()
}

#[test]
fn the_declaration_is_a_public_collection_of_one_source_bound_to_the_fixture() {
    let declaration = declaration();
    assert_eq!(declaration.id, "synthetic");
    assert_eq!(declaration.visibility, Visibility::Public);
    let manifest = manifest();
    assert!(manifest.starts_with(fixture()), "{}", manifest.display());
    assert!(manifest.is_file(), "{}", manifest.display());
    let suite = declaration.evals.suite.under(&fixture());
    assert!(suite.is_file(), "{}", suite.display());
}

#[test]
fn every_manifest_line_holds_the_digest_and_size_of_its_file() {
    let corpus = corpus();
    let entries = entries();
    assert!(!entries.is_empty());
    let mismatched: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let bytes = fs::read(entry.path.under(&corpus)).unwrap();
            let (digest, size) = (Digest::of(&bytes), u64::try_from(bytes.len()).unwrap());
            (digest != entry.sha256 || size != entry.bytes.get()).then(|| {
                format!(
                    "{}: sha256 {}, {size} bytes",
                    entry.path.as_str(),
                    digest.as_str()
                )
            })
        })
        .collect();
    assert!(mismatched.is_empty(), "{mismatched:#?}");
}

#[test]
fn every_document_canonicalizes_without_a_blocking_finding() {
    let blocking: Vec<String> = documents()
        .iter()
        .flat_map(|(entry, document)| {
            document
                .warnings
                .iter()
                .filter(|finding| finding.severity == Severity::Error)
                .map(|finding| format!("{}: {}", entry.path.as_str(), finding.message))
        })
        .collect();
    assert!(blocking.is_empty(), "{blocking:#?}");
}

#[test]
fn the_suite_holds_40_to_60_questions_under_its_contract() {
    let count = questions().len();
    assert!((40..=60).contains(&count), "{count} questions");
}

#[test]
fn every_expected_section_names_exactly_one_section_of_its_document() {
    let documents = documents();
    let mut problems = Vec::new();
    for question in questions() {
        let mut named: Vec<String> = Vec::new();
        for expected in &question.expected {
            match resolve(&documents, expected) {
                Ok(section) if named.contains(&section) => {
                    problems.push(format!("{}: {section} named twice", question.id));
                }
                Ok(section) => named.push(section),
                Err(problem) => problems.push(format!("{}: {problem}", question.id)),
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_answerable_question_expects_one_to_three_sections_and_some_several() {
    let questions = questions();
    let outside: Vec<(&str, usize)> = questions
        .iter()
        .filter(|question| question.answerable)
        .map(|question| (question.id.as_str(), question.expected.len()))
        .filter(|(_, count)| !(1..=3).contains(count))
        .collect();
    assert!(outside.is_empty(), "{outside:?}");
    let several = questions
        .iter()
        .filter(|question| question.expected.len() > 1);
    assert!(several.count() > 1);
}

#[test]
fn about_15_percent_are_unanswerable_and_both_languages_are_asked() {
    let questions = questions();
    let total = questions.len();
    let unanswerable = questions
        .iter()
        .filter(|question| !question.answerable)
        .count();
    // Between 10 % and 20 % of the questions.
    assert!(
        unanswerable * 10 >= total && unanswerable * 5 <= total,
        "{unanswerable} of {total} unanswerable"
    );
    for language in [Language::Fr, Language::En] {
        let asked = questions
            .iter()
            .filter(|question| question.language == language)
            .count();
        assert!(asked * 3 >= total, "{language:?}: {asked} of {total}");
    }
}

#[test]
fn some_questions_are_answered_in_the_other_language_both_ways() {
    let documents = documents();
    let questions = questions();
    for (asked, written) in [(Language::Fr, Language::En), (Language::En, Language::Fr)] {
        let crossing = questions.iter().any(|question| {
            question.language == asked && answered_in(&documents, question).contains(&written)
        });
        assert!(
            crossing,
            "no question asked in {asked:?} is answered in {written:?}"
        );
    }
}
