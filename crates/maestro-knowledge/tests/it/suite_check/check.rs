//! The check itself: each suite of a directory read under its contract, each
//! name its questions give resolved in the document a corpus manifest
//! declares for its `source_ref`, read from the file of that manifest line
//! and canonicalized, and each unanswerable question probed for leads in the
//! files of every manifest line.

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::{
    corpus::Entry,
    lexical,
    suite::{ExpectedSection, Question, Resolved, Suite},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// What a check found.
#[derive(Debug, Default)]
pub(super) struct Checked {
    /// The counts of each suite read, by name.
    pub(super) suites: BTreeMap<String, Counts>,
    /// The `source_ref` of each document read whose file no longer holds the
    /// bytes its manifest line declares: the importer refuses it until the
    /// manifest is written again, so no generation holds it yet.
    pub(super) changed: BTreeSet<String>,
    /// Each name that gives no one section, nor a document without sections,
    /// each question that names one twice, and each file that cannot be read
    /// as it should, in words, sorted.
    pub(super) problems: Vec<String>,
    /// For each unanswerable question with identifier-like terms, by
    /// `<suite>: <id>`, when documents of the manifest hold them all: its
    /// terms and those documents. A lead is no failure: such a document may
    /// answer the question, so a person reads it before the suite is frozen.
    pub(super) unanswerable_leads: BTreeMap<String, Lead>,
    /// The unanswerable questions without an identifier-like term, by
    /// `<suite>: <id>`, which no term leads to a document.
    pub(super) not_probed: Vec<String>,
}

/// The documents that may answer an unanswerable question: those that hold
/// every one of its identifier-like terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Lead {
    /// The question's identifier-like terms, as the lexical analyzer gives
    /// them.
    pub(super) terms: BTreeSet<String>,
    /// The `source_ref` of each document of the manifest that holds them all.
    pub(super) documents: BTreeSet<String>,
}

/// The counts of one suite.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Counts {
    /// Its questions.
    pub(super) questions: usize,
    /// Its unanswerable questions, which name nothing.
    pub(super) unanswerable: usize,
    /// The names its questions give that resolve to one section.
    pub(super) sections: usize,
    /// The names its questions give that resolve to a document without
    /// sections, whole.
    pub(super) documents: usize,
}

/// A name a question gives: its suite's name, the question, and the name.
type Name<'suite> = (&'suite str, &'suite Question, &'suite ExpectedSection);

/// What a name resolved to: its document's `source_ref`, and the section's
/// ID unless the document is named whole.
type Target = (String, Option<String>);

/// Checks every `<name>.jsonl` of the directory `suites` against the corpus
/// manifest `manifest`, whose lines name their files relative to its own
/// directory.
pub(super) fn check(suites: &Path, manifest: &Path) -> Checked {
    let mut checked = Checked::default();
    let read = read_suites(suites, &mut checked);
    let entries = read_entries(manifest, &mut checked.problems);
    let root = manifest.parent().unwrap_or(Path::new(""));
    let mut named: BTreeMap<&str, Vec<Name<'_>>> = BTreeMap::new();
    for (suite, questions) in &read {
        for question in &questions.questions {
            for name in &question.expected {
                let names = named.entry(name.source_ref.as_str()).or_default();
                names.push((suite.as_str(), question, name));
            }
        }
    }
    probe(&read, &entries, root, &mut checked);
    let mut targets: BTreeMap<(&str, &str), Vec<Target>> = BTreeMap::new();
    for (source_ref, names) in named {
        let document = document(root, entries.get(source_ref), source_ref, &mut checked);
        for (suite, question, name) in names {
            let place = format!("{suite}: {}", question.id);
            let resolved = document
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|document| {
                    let path = &name.heading_path;
                    let refusal = |refusal| format!("{source_ref} {path:?}: {refusal}");
                    name.resolve(document).map_err(refusal)
                });
            let counts = checked.suites.entry(suite.to_owned()).or_default();
            let section_id = match resolved {
                Ok(Resolved::Section(section)) => {
                    counts.sections += 1;
                    Some(section.section_id.clone())
                }
                Ok(Resolved::Document(_)) => {
                    counts.documents += 1;
                    None
                }
                Err(problem) => {
                    checked.problems.push(format!("{place}: {problem}"));
                    continue;
                }
            };
            let target = (source_ref.to_owned(), section_id);
            let question_targets = targets.entry((suite, &question.id)).or_default();
            if question_targets.contains(&target) {
                let path = &name.heading_path;
                let twice = format!("{place}: names {source_ref} {path:?} twice");
                checked.problems.push(twice);
            }
            question_targets.push(target);
        }
    }
    checked.problems.sort();
    checked
}

/// Each suite of `directory`, by name: every `<name>.jsonl` in it, read
/// under `maestro-suite/1`, with its questions counted in `checked`; a file
/// that is not a suite, and a directory without one, are problems.
fn read_suites(directory: &Path, checked: &mut Checked) -> BTreeMap<String, Suite> {
    let mut suites = BTreeMap::new();
    let files = match fs::read_dir(directory) {
        Ok(files) => files,
        Err(error) => {
            let shown = directory.display();
            checked
                .problems
                .push(format!("{shown} cannot be read: {error}"));
            return suites;
        }
    };
    let mut paths: Vec<PathBuf> = files
        .filter_map(|file| Some(file.ok()?.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        let shown = directory.display();
        checked.problems.push(format!("{shown} holds no suite"));
    }
    for path in paths {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        let read = fs::read_to_string(&path).map_err(|error| error.to_string());
        match read.and_then(|text| text.parse::<Suite>().map_err(|error| error.to_string())) {
            Ok(suite) => {
                let questions = &suite.questions;
                let unanswerable = questions.iter().filter(|question| !question.answerable);
                let counts = Counts {
                    questions: questions.len(),
                    unanswerable: unanswerable.count(),
                    ..Counts::default()
                };
                checked.suites.insert(name.to_string(), counts);
                suites.insert(name.to_string(), suite);
            }
            Err(error) => checked.problems.push(format!("suite {name}: {error}")),
        }
    }
    suites
}

/// The lines of `manifest`, by `source_ref`; a line that is not a corpus
/// entry is a problem.
fn read_entries(manifest: &Path, problems: &mut Vec<String>) -> BTreeMap<String, Vec<Entry>> {
    let mut entries: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    let text = match fs::read_to_string(manifest) {
        Ok(text) => text,
        Err(error) => {
            let shown = manifest.display();
            problems.push(format!("{shown} cannot be read: {error}"));
            return entries;
        }
    };
    for (number, line) in (1..).zip(text.lines()) {
        match line.parse::<Entry>() {
            Ok(entry) => entries
                .entry(entry.source_ref.clone())
                .or_default()
                .push(entry),
            Err(error) => problems.push(format!("manifest line {number}: {error}")),
        }
    }
    entries
}

/// Records in `checked` a lead for each unanswerable question of `suites`
/// with identifier-like terms, to each document of `entries`, read from its
/// file under `root`, whose terms hold them all, and each question without
/// such a term as not probed. A file that cannot be read is a problem, since
/// a lead in it would go unseen.
fn probe(
    suites: &BTreeMap<String, Suite>,
    entries: &BTreeMap<String, Vec<Entry>>,
    root: &Path,
    checked: &mut Checked,
) {
    let mut probed = Vec::new();
    for (suite, questions) in suites {
        let unanswerable = questions.questions.iter().filter(|asked| !asked.answerable);
        for question in unanswerable {
            let place = format!("{suite}: {}", question.id);
            let terms = identifier_terms(&question.question);
            if terms.is_empty() {
                checked.not_probed.push(place);
            } else {
                probed.push((place, terms));
            }
        }
    }
    if probed.is_empty() {
        return;
    }
    for entry in entries.values().flatten() {
        let bytes = match fs::read(entry.path.under(root)) {
            Ok(bytes) => bytes,
            Err(error) => {
                let path = entry.path.as_str();
                let unread = format!("{path} cannot be read, so it gives no lead: {error}");
                checked.problems.push(unread);
                continue;
            }
        };
        let held: HashSet<String> = lexical::terms(&String::from_utf8_lossy(&bytes))
            .into_iter()
            .collect();
        for (place, terms) in &probed {
            if terms.iter().all(|term| held.contains(term)) {
                let lead = checked
                    .unanswerable_leads
                    .entry(place.clone())
                    .or_insert_with(|| Lead {
                        terms: terms.clone(),
                        documents: BTreeSet::new(),
                    });
                lead.documents.insert(entry.source_ref.clone());
            }
        }
    }
}

/// The identifier-like terms of `text`: of each run of letters, digits and
/// the lexical analyzer's joiners that holds a digit, a joiner or a
/// camelCase part, the term the analyzer gives its whole form.
fn identifier_terms(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !(character.is_alphanumeric() || is_joiner(character)))
        .map(|run| run.trim_matches(is_joiner))
        .filter(|run| is_identifier_like(run))
        .filter_map(|run| lexical::terms(run).into_iter().next())
        .collect()
}

/// Whether `character` joins the runs of an identifier for the lexical
/// analyzer: `-`, `_`, `.`, `/`, `\` or `:`.
fn is_joiner(character: char) -> bool {
    matches!(character, '-' | '_' | '.' | '/' | '\\' | ':')
}

/// Whether `run` holds a digit, a joiner or a camelCase part, which starts
/// where a lowercase letter meets an uppercase one, or before the last of
/// three uppercase letters followed by two lowercase ones.
fn is_identifier_like(run: &str) -> bool {
    let letters: Vec<char> = run.chars().collect();
    let lower_upper = |pair: &[char]| match pair {
        [before, after] => before.is_lowercase() && after.is_uppercase(),
        _ => false,
    };
    let three_upper_two_lower = |window: &[char]| match window {
        [first, second, third, fourth, fifth] => {
            [first, second, third]
                .iter()
                .all(|letter| letter.is_uppercase())
                && fourth.is_lowercase()
                && fifth.is_lowercase()
        }
        _ => false,
    };
    letters
        .iter()
        .any(|&letter| letter.is_numeric() || is_joiner(letter))
        || letters.windows(2).any(lower_upper)
        || letters.windows(5).any(three_upper_two_lower)
}

/// The canonical document of `source_ref`, which `declared`, its manifest
/// lines, must declare, all with the same bytes, read from the file of the
/// first of them under `root`, and recorded in `checked` as changed when the
/// file no longer holds the bytes the line declares; or why there is none.
fn document(
    root: &Path,
    declared: Option<&Vec<Entry>>,
    source_ref: &str,
    checked: &mut Checked,
) -> Result<CanonicalDocument, String> {
    let entry = match declared.map(Vec::as_slice) {
        None | Some([]) => return Err(format!("no manifest line declares {source_ref}")),
        Some([first, others @ ..]) if others.iter().all(|other| other.sha256 == first.sha256) => {
            first
        }
        Some(several) => {
            let lines = several.len();
            return Err(format!(
                "{source_ref} is declared by {lines} manifest lines with different bytes, \
                 which the importer holds"
            ));
        }
    };
    let path = entry.path.as_str();
    let bytes = fs::read(entry.path.under(root))
        .map_err(|error| format!("{path} cannot be read: {error}"))?;
    if Digest::of(&bytes) != entry.sha256 {
        checked.changed.insert(source_ref.to_owned());
    }
    let markdown = String::from_utf8(bytes).map_err(|_| format!("{path} is not UTF-8"))?;
    let mut input = CanonicalizeInput::new(&markdown, source_ref);
    input.metadata.source_reference = Some(source_ref.to_owned());
    input.metadata.title = Some(entry.title.clone());
    canonicalize(input).map_err(|error| format!("{path} does not canonicalize: {error}"))
}
