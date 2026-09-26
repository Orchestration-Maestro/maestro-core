//! An observed import (`import_observed`): its caller sees the report after
//! every hundredth line of each source's manifest and after the last line
//! of each, as a job journals its progress, and stops the import when it
//! breaks, before any completion is journaled.

use super::support::{
    Scratch, bindings, data_of, declaration_of, events, everything, line, markdown, page,
};
use maestro_knowledge::import::{self, Error, Report};
use serde_json::Value;
use std::{error, ops::ControlFlow};

/// The type of the event an import journals when it completes.
const COMPLETED: &str = "maestro.knowledge.import.completed.v1";

/// Its counts: imported, unchanged, held, refused.
fn counts(report: &Report) -> [u64; 4] {
    [
        report.imported,
        report.unchanged,
        report.held,
        report.refused,
    ]
}

/// Writes the manifest `manifest` of `lines` lines: at each line number
/// `documents` names, the entry of a document of its own, written beside
/// it; at every other line, text that is no entry.
fn manifest(scratch: &Scratch, manifest: &str, lines: u64, documents: &[u64]) {
    let text: String = (1..=lines)
        .map(|number| {
            if !documents.contains(&number) {
                return format!("line {number} is no entry\n");
            }
            let path = format!("{manifest}-{number}.md");
            let bytes = markdown(&path);
            scratch.put(&path, &bytes);
            line(&path, &bytes, &page(&path)).to_string() + "\n"
        })
        .collect();
    scratch.put(manifest, text.as_bytes());
}

#[test]
fn the_observer_sees_the_report_every_hundred_lines_and_after_each_last_line() {
    let scratch = Scratch::new();
    manifest(&scratch, "long.jsonl", 250, &[1, 2]);
    manifest(&scratch, "round.jsonl", 100, &[1]);
    manifest(&scratch, "empty.jsonl", 0, &[]);
    let declaration = declaration_of(
        "observed",
        &[
            ("long", "long.jsonl"),
            ("round", "round.jsonl"),
            ("empty", "empty.jsonl"),
        ],
    );
    let database = scratch.database();
    let mut seen = Vec::new();
    let report = import::import_observed(
        &database,
        &everything(&database),
        &declaration,
        &bindings(&scratch),
        &mut |report: &Report| {
            seen.push(counts(report));
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    assert_eq!(
        seen,
        [
            [2, 0, 0, 98],
            [2, 0, 0, 198],
            [2, 0, 0, 248],
            [3, 0, 0, 347]
        ],
        "after lines 100, 200 and 250 of the first source, and 100 of the second, which is \
         its last; the empty source has no line to observe"
    );
    assert_eq!(counts(&report), [3, 0, 0, 347]);
    assert_eq!(data_of(&events(&database, "observed"), COMPLETED).len(), 1);
}

#[test]
fn an_observer_that_breaks_stops_the_import_before_its_completion() {
    let scratch = Scratch::new();
    manifest(&scratch, "long.jsonl", 250, &[1, 150]);
    let declaration = declaration_of("observed", &[("long", "long.jsonl")]);
    let database = scratch.database();
    let scopes = everything(&database);
    let mut calls = 0;
    let stopped = import::import_observed(
        &database,
        &scopes,
        &declaration,
        &bindings(&scratch),
        &mut |_: &Report| {
            calls += 1;
            ControlFlow::Break(())
        },
    );
    assert!(
        matches!(stopped, Err(Error::Stopped)),
        "{:?}",
        stopped.as_ref().map(counts)
    );
    assert_eq!(calls, 1, "the import stopped at the first break");
    assert_eq!(
        data_of(&events(&database, "observed"), COMPLETED),
        Vec::<Value>::new(),
        "a stopped import journals no completion"
    );
    let resumed = import::import(&database, &scopes, &declaration, &bindings(&scratch)).unwrap();
    assert_eq!(
        counts(&resumed),
        [1, 1, 0, 248],
        "the rerun finds line 1 recorded and imports line 150, which the stop left alone"
    );
}

#[test]
fn a_stopped_import_says_so() {
    let message = Error::Stopped.to_string();
    assert!(
        message.starts_with("the import was stopped by its caller"),
        "{message}"
    );
    assert!(error::Error::source(&Error::Stopped).is_none());
}
