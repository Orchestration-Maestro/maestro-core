//! The public synthetic collection, `tests/fixtures/synthetic`, which stands in
//! for the private corpus in public CI (ADR-0009): its declaration, its corpus
//! manifest and its suites hold under their contracts, every document
//! canonicalizes without a blocking finding, and every section the suite
//! `synthetic` expects is exactly one section of its document.
#![cfg(test)]

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, Severity, canonicalize};
use maestro_kernel::{artifact::Digest, binding::Bindings};
use maestro_knowledge::{
    collection::{Declaration, Visibility},
    corpus::Entry,
    suite::{ExpectedSection, Language, Question, Resolved, Suite},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// What surrounds a word in prose or code without being part of it.
const PUNCTUATION: &[char] = &[
    '?', '!', ',', ';', ':', '.', '"', '\'', '(', ')', '[', ']', '{', '}', '«', '»',
];

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

/// The Markdown of the document `entry` declares.
fn markdown(entry: &Entry) -> String {
    fs::read_to_string(entry.path.under(&corpus())).unwrap()
}

/// Every document of the manifest, canonicalized with the provenance its line
/// gives.
fn documents() -> Vec<(Entry, CanonicalDocument)> {
    entries()
        .into_iter()
        .map(|entry| {
            let markdown = markdown(&entry);
            let mut input = CanonicalizeInput::new(&markdown, entry.path.as_str());
            input.metadata.source_reference = Some(entry.source_ref.clone());
            input.metadata.title = Some(entry.title.clone());
            let document = canonicalize(input).unwrap();
            (entry, document)
        })
        .collect()
}

/// Every suite of the directory the declaration's `evals.suite` names: each
/// `<name>.jsonl` in it is the suite `<name>`.
fn suites() -> BTreeMap<String, Suite> {
    let directory = declaration().evals.suite.under(&fixture());
    fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|file| file.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let text = fs::read_to_string(&path).unwrap();
            let suite = text
                .parse()
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            (name, suite)
        })
        .collect()
}

/// The questions of the suite `synthetic`.
fn questions() -> Vec<Question> {
    let mut suites = suites();
    let suite = suites.remove("synthetic");
    suite
        .unwrap_or_else(|| panic!("no suite synthetic among {:?}", suites.keys()))
        .questions
}

/// The ID of the section `expected` names, in the document of `documents`
/// whose `source_ref` it gives, or why it names none.
fn section_id(
    documents: &[(Entry, CanonicalDocument)],
    expected: &ExpectedSection,
) -> Result<String, String> {
    let (_, document) = documents
        .iter()
        .find(|(entry, _)| entry.source_ref == expected.source_ref)
        .ok_or_else(|| format!("no document is {}", expected.source_ref))?;
    let path = &expected.heading_path;
    match expected.resolve(document) {
        Ok(Resolved::Section(section)) => Ok(section.section_id.clone()),
        Ok(Resolved::Document(_)) => Err(format!("{} is expected whole", expected.source_ref)),
        Err(refusal) => Err(format!("{} {path:?}: {refusal}", expected.source_ref)),
    }
}

/// The language a document is written in: its path's first directory.
fn written_in(entry: &Entry) -> Language {
    match entry.path.as_str().split('/').next() {
        Some("fr") => Language::Fr,
        Some("en") => Language::En,
        _ => panic!("{} is under neither fr/ nor en/", entry.path.as_str()),
    }
}

/// The documents whose sections `question` expects.
fn answered_in<'entries>(
    documents: &'entries [(Entry, CanonicalDocument)],
    question: &Question,
) -> Vec<&'entries Entry> {
    documents
        .iter()
        .map(|(entry, _)| entry)
        .filter(|entry| {
            question
                .expected
                .iter()
                .any(|expected| expected.source_ref == entry.source_ref)
        })
        .collect()
}

/// The words a Markdown text writes as code: in its fenced blocks and in its
/// code spans.
fn code_words(markdown: &str) -> BTreeSet<String> {
    markdown
        .split("```")
        .enumerate()
        .flat_map(|(index, part)| {
            if index % 2 == 1 {
                vec![part]
            } else {
                part.split('`').skip(1).step_by(2).collect()
            }
        })
        .flat_map(str::split_whitespace)
        .map(|word| word.trim_matches(PUNCTUATION).to_owned())
        .collect()
}

/// Whether `character` is a lowercase ASCII letter or a digit.
fn lowercase_or_digit(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}

/// The style of identifier `word` is written in, if it is one: `snake_case`,
/// lowercase kebab-case or dotted, `camelCase`, or an error code such as
/// `TLS-017`.
fn identifier_style(word: &str) -> Option<&'static str> {
    let only = |allowed: fn(char) -> bool| word.chars().all(allowed);
    let lowercase_start = word.starts_with(|character: char| character.is_ascii_lowercase());
    let uppercase_start = word.starts_with(|character: char| character.is_ascii_uppercase());
    let has_uppercase = word.contains(|character: char| character.is_ascii_uppercase());
    let has_digit = word.contains(|character: char| character.is_ascii_digit());
    [
        (
            "snake_case",
            lowercase_start
                && word.contains('_')
                && only(|character| lowercase_or_digit(character) || character == '_'),
        ),
        (
            "kebab-case or dotted",
            lowercase_start
                && word.contains(['-', '.'])
                && only(|character| lowercase_or_digit(character) || "-.".contains(character)),
        ),
        (
            "camelCase",
            lowercase_start && has_uppercase && only(|character| character.is_ascii_alphanumeric()),
        ),
        (
            "error code",
            uppercase_start
                && has_digit
                && word.contains('-')
                && only(|character| {
                    character.is_ascii_uppercase() || character.is_ascii_digit() || character == '-'
                }),
        ),
    ]
    .into_iter()
    .find_map(|(style, written)| written.then_some(style))
}

#[test]
fn the_declaration_is_a_public_collection_of_one_source_bound_to_the_fixture() {
    let declaration = declaration();
    assert_eq!(declaration.id, "synthetic");
    assert_eq!(declaration.visibility, Visibility::Public);
    let manifest = manifest();
    assert!(manifest.starts_with(fixture()), "{}", manifest.display());
    assert!(manifest.is_file(), "{}", manifest.display());
    let evals = declaration.evals.suite.under(&fixture());
    assert!(evals.is_dir(), "{}", evals.display());
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
fn the_suite_synthetic_holds_40_to_60_questions_under_its_contract() {
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
            match section_id(&documents, expected) {
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
            question.language == asked
                && answered_in(&documents, question)
                    .into_iter()
                    .any(|entry| written_in(entry) == written)
        });
        assert!(
            crossing,
            "no question asked in {asked:?} is answered in {written:?}"
        );
    }
}

#[test]
fn some_question_looks_up_each_style_of_identifier_its_answer_writes_as_code() {
    let documents = documents();
    let mut looked_up = BTreeSet::new();
    for question in questions() {
        let code: BTreeSet<String> = answered_in(&documents, &question)
            .into_iter()
            .flat_map(|entry| code_words(&markdown(entry)))
            .collect();
        let words = question.question.split_whitespace();
        looked_up.extend(
            words
                .map(|word| word.trim_matches(PUNCTUATION))
                .filter(|word| code.contains(*word))
                .filter_map(identifier_style),
        );
    }
    let styles = [
        "camelCase",
        "error code",
        "kebab-case or dotted",
        "snake_case",
    ];
    assert_eq!(looked_up, BTreeSet::from(styles));
}
