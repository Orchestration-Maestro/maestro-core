//! The disposition report of a local run, in Markdown, for the collection's
//! private repository: counts, rule IDs, identities and reasons, never the
//! corpus's text. A document is named by its revision's ID and the manifest
//! lines that give its `source_ref`, never by its path, title or URL, and a
//! reason that names the `source_ref` names it `<source_ref>`.

use maestro_knowledge::{import, quality};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    time::Duration,
};

/// A manifest line: its source and its number, from 1.
pub(super) type Line = (String, u64);

/// What the collection's manifests say of each of their lines.
#[derive(Debug, Default)]
pub(super) struct Manifest {
    /// How many lines they hold.
    pub(super) lines: u64,
    /// The lines of each `source_ref`.
    pub(super) by_source_ref: BTreeMap<String, BTreeSet<Line>>,
    /// The source kind and the set of each line that is an entry.
    pub(super) kinds: BTreeMap<Line, (String, String)>,
}

/// What the kernel holds of each document once gated: its latest revision
/// in record order, the outcome that revision was given, and the documents
/// eligible for chunking.
#[derive(Debug, Default)]
pub(super) struct Documents {
    /// The latest revision of each document.
    pub(super) latest: BTreeSet<String>,
    /// The documents by the outcome of their latest revision, `none` for
    /// one not decided.
    pub(super) outcomes: BTreeMap<String, u64>,
    /// The documents eligible for chunking.
    pub(super) eligible: u64,
}

/// What a local run found, and how long it took.
pub(super) struct Run<'a> {
    /// What the collection's manifests say of their lines.
    pub(super) manifest: &'a Manifest,
    /// What the kernel holds of each document once gated.
    pub(super) documents: &'a Documents,
    /// What the import did.
    pub(super) import: &'a import::Report,
    /// What the gate found.
    pub(super) gate: &'a quality::Report,
    /// The import's wall time.
    pub(super) importing: Duration,
    /// The gate's wall time.
    pub(super) gating: Duration,
}

/// Lines by what they share: a rule or a reason, a source kind and a set.
type Groups<'a> = BTreeMap<(&'a str, &'a str, &'a str), BTreeSet<Line>>;

/// The disposition report of `run` over the collection `collection`.
pub(super) fn markdown(collection: &str, run: &Run<'_>) -> String {
    let mut text = format!(
        "# Quality dispositions of the `{collection}` collection\n\n\
         Written by maestro-core's local collection run (T020): counts, rule IDs,\n\
         identities and reasons, never the corpus's text. A document is named by its\n\
         revision's ID and by the manifest lines that give its `source_ref`, written\n\
         `<source>:<line>`; a reason that names the `source_ref` names it\n\
         `<source_ref>`. The revisions accepted with warnings are counted by rule, not\n\
         listed.\n\n"
    );
    import_section(&mut text, run);
    gate_section(&mut text, run);
    documents_section(&mut text, run);
    groups_section(&mut text, run);
    superseded_section(&mut text, run);
    held_section(&mut text, run);
    text
}

/// The import's counts, and its refusals by reason, source kind and set.
fn import_section(text: &mut String, run: &Run<'_>) {
    let import = run.import;
    let lines = run.manifest.lines;
    let _ = write!(
        text,
        "## Import\n\n\
         | Manifest lines | Imported | Unchanged | Held | Refused | Wall time | Lines per second \
         |\n\
         | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n\
         | {lines} | {} | {} | {} | {} | {:.1} s | {:.1} |\n\n",
        import.imported,
        import.unchanged,
        import.held,
        import.refused,
        run.importing.as_secs_f64(),
        per_second(lines, run.importing),
    );
    if import.refusals.is_empty() {
        text.push_str("No line was refused.\n\n");
        return;
    }
    let reasons: Vec<String> = import
        .refusals
        .iter()
        .map(|refusal| {
            serde_json::to_value(refusal)
                .ok()
                .and_then(|value| value["reason"].as_str().map(str::to_owned))
                .unwrap_or_default()
        })
        .collect();
    let mut groups = Groups::new();
    for (refusal, reason) in import.refusals.iter().zip(&reasons) {
        let line = (refusal.source.clone(), refusal.line);
        let (kind, set) = kind_of(run, &line);
        groups.entry((reason, kind, set)).or_default().insert(line);
    }
    text.push_str(
        "| Reason | Source kind | Set | Lines | Manifest lines |\n\
         | --- | --- | --- | ---: | --- |\n",
    );
    for ((reason, kind, set), lines) in groups {
        let _ = writeln!(
            text,
            "| `{reason}` | {kind} | {set} | {} | {} |",
            lines.len(),
            listed(&lines)
        );
    }
    text.push('\n');
}

/// The gate's counts, by outcome and by rule.
fn gate_section(text: &mut String, run: &Run<'_>) {
    let gate = run.gate;
    let _ = write!(
        text,
        "## Gate\n\n| Revisions | Decided by this run | Kept | Wall time | Revisions per second |\n\
         | ---: | ---: | ---: | ---: | ---: |\n\
         | {} | {} | {} | {:.1} s | {:.1} |\n\n| Outcome | Revisions |\n| --- | ---: |\n",
        gate.revisions,
        gate.decided,
        gate.kept,
        run.gating.as_secs_f64(),
        per_second(gate.revisions, run.gating),
    );
    let outcomes = serde_json::to_value(gate.outcomes).unwrap_or(Value::Null);
    for (outcome, count) in outcomes.as_object().into_iter().flatten() {
        let _ = writeln!(text, "| `{outcome}` | {count} |");
    }
    text.push_str("\n| Rule | Revisions |\n| --- | ---: |\n");
    for (rule, count) in &gate.rules {
        let _ = writeln!(text, "| `{rule}` | {count} |");
    }
    text.push('\n');
}

/// The documents, by the outcome of their latest revision, and how many are
/// eligible for chunking.
fn documents_section(text: &mut String, run: &Run<'_>) {
    let documents = run.documents;
    let _ = write!(
        text,
        "## Documents\n\n\
         A document counts by its latest revision in record order alone: an older\n\
         revision never stands in for it. Of {} documents, {} are eligible for\n\
         chunking: their latest revision is not failed and is accepted, with or\n\
         without warnings.\n\n\
         | Outcome of the latest revision | Documents |\n| --- | ---: |\n",
        documents.latest.len(),
        documents.eligible,
    );
    for (outcome, count) in &documents.outcomes {
        let _ = writeln!(text, "| `{outcome}` | {count} |");
    }
    text.push('\n');
}

/// Whether `held` is its document's latest revision.
fn latest(run: &Run<'_>, held: &quality::Held) -> bool {
    run.documents.latest.contains(&held.revision)
}

/// The revisions held back that a newer revision of their document
/// superseded, by rule.
fn superseded_section(text: &mut String, run: &Run<'_>) {
    let mut rules: BTreeMap<&str, u64> = BTreeMap::new();
    for held in run.gate.held.iter().filter(|held| !latest(run, held)) {
        for rule in &held.rule_ids {
            *rules.entry(rule).or_default() += 1;
        }
    }
    text.push_str(
        "## Superseded revisions held back, by rule\n\n\
         A newer revision of their document superseded them, so none is ever\n\
         eligible; they are counted, not listed.\n\n\
         | Rule | Revisions |\n| --- | ---: |\n",
    );
    for (rule, count) in rules {
        let _ = writeln!(text, "| `{rule}` | {count} |");
    }
    text.push('\n');
}

/// The latest revisions held back, by rule, then by source kind and set.
fn groups_section(text: &mut String, run: &Run<'_>) {
    let mut groups = Groups::new();
    let mut revisions: BTreeMap<(&str, &str, &str), usize> = BTreeMap::new();
    for held in run.gate.held.iter().filter(|held| latest(run, held)) {
        let kind = held.source_kind.as_deref().unwrap_or("-");
        let set = held.set.as_deref().unwrap_or("-");
        for rule in &held.rule_ids {
            *revisions.entry((rule, kind, set)).or_default() += 1;
            groups
                .entry((rule, kind, set))
                .or_default()
                .extend(lines_of(run, &held.source_ref));
        }
    }
    text.push_str("## Latest revisions held back, by rule, source kind and set\n");
    let mut current = "";
    for ((rule, kind, set), lines) in groups {
        if rule != current {
            let _ = write!(
                text,
                "\n### `{rule}`\n\n| Source kind | Set | Revisions | Manifest lines |\n\
                 | --- | --- | ---: | --- |\n"
            );
            current = rule;
        }
        let count = revisions
            .get(&(rule, kind, set))
            .copied()
            .unwrap_or_default();
        let _ = writeln!(text, "| {kind} | {set} | {count} | {} |", listed(&lines));
    }
    text.push('\n');
}

/// Each latest revision held back, with its lines, rules and reasons.
fn held_section(text: &mut String, run: &Run<'_>) {
    text.push_str(
        "## Each latest revision held back\n\n\
         | Manifest lines | Revision | Outcome | Rules | Decided by | Reasons |\n\
         | --- | --- | --- | --- | --- | --- |\n",
    );
    for held in run.gate.held.iter().filter(|held| latest(run, held)) {
        let outcome = serde_json::to_value(held)
            .ok()
            .and_then(|value| value["outcome"].as_str().map(str::to_owned))
            .unwrap_or_default();
        let reasons: Vec<String> = held
            .reasons
            .iter()
            .map(|reason| cell(&reason.replace(&held.source_ref, "<source_ref>")))
            .collect();
        let rules: Vec<String> = held
            .rule_ids
            .iter()
            .map(|rule| format!("`{rule}`"))
            .collect();
        let _ = writeln!(
            text,
            "| {} | `{}` | `{outcome}` | {} | {} | {} |",
            listed(&lines_of(run, &held.source_ref)),
            held.revision,
            rules.join(", "),
            cell(&held.decided_by),
            reasons.join("; ")
        );
    }
}

/// The manifest lines that give `source_ref`.
fn lines_of(run: &Run<'_>, source_ref: &str) -> BTreeSet<Line> {
    run.manifest
        .by_source_ref
        .get(source_ref)
        .cloned()
        .unwrap_or_default()
}

/// The source kind and the set of `line`, or `-` for what it lacks.
fn kind_of<'a>(run: &'a Run<'_>, line: &Line) -> (&'a str, &'a str) {
    run.manifest
        .kinds
        .get(line)
        .map_or(("-", "-"), |(kind, set)| (kind.as_str(), set.as_str()))
}

/// `lines`, each as `<source>:<line>`, in order.
fn listed(lines: &BTreeSet<Line>) -> String {
    let named: Vec<String> = lines
        .iter()
        .map(|(source, line)| format!("{source}:{line}"))
        .collect();
    named.join(", ")
}

/// `text` as one cell of a Markdown table.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// The rate of `count` items in `elapsed`, per second.
pub(super) fn per_second(count: u64, elapsed: Duration) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a rate for people, from counts far below 2^52"
    )]
    let count = count as f64;
    count / elapsed.as_secs_f64()
}
