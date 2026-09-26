//! Refusals: a line that is no `maestro-corpus/1` entry, a document that
//! does not hold what its line declares or that canonicalization cannot
//! read, and bytes or a document the kernel holds otherwise are each refused
//! with their line and reason, and the import goes on with the next line.

use super::support::{
    Scratch, declaration_of, everything, import, import_declared, line, markdown, page,
};
use maestro_kernel::{artifact::Digest, store::Database};
use maestro_knowledge::import::{Reason, Refusal, Report};
use serde_json::json;

/// The original digests of the revisions of `collection`, in the order they
/// were recorded.
fn originals(database: &Database, collection: &str) -> Vec<Digest> {
    database
        .eligible_revisions(&everything(database), collection)
        .unwrap()
        .into_iter()
        .map(|revision| revision.original_digest)
        .collect()
}

/// The counts of `report`: imported, unchanged, held and refused.
fn counts(report: &Report) -> [u64; 4] {
    [
        report.imported,
        report.unchanged,
        report.held,
        report.refused,
    ]
}

/// The line numbers of the refusals of `report`, in order.
fn refused_lines(report: &Report) -> Vec<u64> {
    report.refusals.iter().map(|refusal| refusal.line).collect()
}

#[test]
fn a_digest_mismatch_refuses_its_entry_with_both_digests_and_the_rest_continue() {
    let scratch = Scratch::new();
    let [compost, mulch, seeds] = ["compost", "mulch", "seeds"].map(markdown);
    scratch.put("compost.md", &compost);
    scratch.put("mulch.md", &mulch);
    scratch.put("seeds.md", &seeds);
    let edited = markdown("mulch, as it read before an edit");
    scratch.manifest(&[
        line("compost.md", &compost, &page("compost")),
        line("mulch.md", &edited, &page("mulch")),
        line("seeds.md", &seeds, &page("seeds")),
    ]);
    let database = scratch.database();
    let report = import(&scratch, &database, "garden");
    assert_eq!(counts(&report), [2, 0, 0, 1]);
    let mismatch = Reason::DigestMismatch {
        declared: Digest::of(&edited),
        found: Digest::of(&mulch),
    };
    assert_eq!(
        report.refusals,
        [Refusal {
            source: "docs".to_owned(),
            line: 2,
            reason: mismatch,
        }]
    );
    assert_eq!(
        originals(&database, "garden"),
        [Digest::of(&compost), Digest::of(&seeds)]
    );
}

#[test]
fn a_malformed_line_is_refused_with_its_number_and_the_rest_continue() {
    let scratch = Scratch::new();
    let [compost, seeds] = ["compost", "seeds"].map(markdown);
    scratch.put("compost.md", &compost);
    scratch.put("seeds.md", &seeds);
    let mut climbing = line("seeds.md", &seeds, &page("climbing"));
    climbing["path"] = json!("../seeds.md");
    let mut coloured = line("seeds.md", &seeds, &page("coloured"));
    coloured["colour"] = json!("green");
    let lines = [
        line("compost.md", &compost, &page("compost")).to_string(),
        r#"{"schema":"maestro-corpus/1","path":"#.to_owned(),
        climbing.to_string(),
        String::new(),
        coloured.to_string(),
        line("seeds.md", &seeds, &page("seeds")).to_string(),
    ];
    let mut manifest: Vec<u8> = lines.map(|text| text + "\n").concat().into_bytes();
    manifest.extend(b"{\"title\":\"\xff\"}\n");
    scratch.put("manifest.jsonl", &manifest);
    let database = scratch.database();
    let report = import(&scratch, &database, "garden");
    assert_eq!(counts(&report), [2, 0, 0, 5]);
    assert_eq!(refused_lines(&report), [2, 3, 4, 5, 7]);
    let messages: Vec<&str> = report
        .refusals
        .iter()
        .map(|refusal| match &refusal.reason {
            Reason::Malformed { message } => message.as_str(),
            other => panic!("line {}: {other:?}", refusal.line),
        })
        .collect();
    let [truncated, climbs, empty, unknown, binary] = messages.as_slice() else {
        panic!("{messages:?}");
    };
    assert!(truncated.contains("maestro-corpus/1"), "{truncated}");
    assert!(climbs.contains("climbs out of its directory"), "{climbs}");
    assert!(empty.contains("maestro-corpus/1"), "{empty}");
    assert!(unknown.contains("colour"), "{unknown}");
    assert!(binary.contains("UTF-8"), "{binary}");
    assert_eq!(
        originals(&database, "garden"),
        [Digest::of(&compost), Digest::of(&seeds)]
    );
}

#[test]
fn a_document_missing_misdeclared_or_unreadable_as_markdown_is_refused() {
    let scratch = Scratch::new();
    let compost = markdown("compost");
    let binary = b"# Seeds\n\n\xff\xfe\n".to_vec();
    let nested = format!("{} too deep\n", ">".repeat(200)).into_bytes();
    let mulch = markdown("mulch");
    for (path, bytes) in [
        ("compost.md", &compost),
        ("seeds.md", &binary),
        ("nested.md", &nested),
        ("mulch.md", &mulch),
    ] {
        scratch.put(path, bytes);
    }
    let mut oversized = line("mulch.md", &mulch, &page("mulch"));
    oversized["bytes"] = json!(mulch.len() + 1);
    scratch.manifest(&[
        line("missing.md", &compost, &page("missing")),
        line("seeds.md", &binary, &page("seeds")),
        line("nested.md", &nested, &page("nested")),
        oversized,
        line("compost.md", &compost, &page("compost")),
    ]);
    let database = scratch.database();
    let report = import(&scratch, &database, "garden");
    assert_eq!(counts(&report), [1, 0, 0, 4]);
    let reasons: Vec<&Reason> = report
        .refusals
        .iter()
        .map(|refusal| &refusal.reason)
        .collect();
    assert_eq!(refused_lines(&report), [1, 2, 3, 4]);
    assert!(
        matches!(reasons[0], Reason::Unreadable { path, message }
            if path == "missing.md" && !message.is_empty()),
        "{reasons:?}"
    );
    assert_eq!(reasons[1], &Reason::NotUtf8);
    assert!(
        matches!(reasons[2], Reason::Canonicalization { message } if !message.is_empty()),
        "{reasons:?}"
    );
    assert_eq!(
        reasons[3],
        &Reason::SizeMismatch {
            declared: u64::try_from(mulch.len()).unwrap() + 1,
            found: u64::try_from(mulch.len()).unwrap(),
        }
    );
    assert_eq!(originals(&database, "garden"), [Digest::of(&compost)]);
}

#[test]
fn bytes_or_a_document_the_kernel_holds_otherwise_are_refused_as_conflicts() {
    let scratch = Scratch::new();
    let [compost, mulch, notes] = ["compost", "mulch", "mulch notes"].map(markdown);
    scratch.put("compost.md", &compost);
    scratch.put("mulch.md", &mulch);
    scratch.put("mulch-notes.md", &notes);
    scratch.manifest_at(
        "docs.jsonl",
        &[
            line("compost.md", &compost, &page("compost")),
            line("mulch.md", &mulch, &page("mulch")),
        ],
    );
    scratch.manifest_at(
        "notes.jsonl",
        &[line("mulch-notes.md", &notes, &page("mulch"))],
    );
    let database = scratch.database();
    database.put(&compost, "text/plain").unwrap();
    let declaration = declaration_of(
        "garden",
        &[("docs", "docs.jsonl"), ("notes", "notes.jsonl")],
    );
    let report = import_declared(&scratch, &database, &declaration);
    assert_eq!(counts(&report), [1, 0, 0, 2]);
    let refused: Vec<(&str, u64)> = report
        .refusals
        .iter()
        .map(|refusal| (refusal.source.as_str(), refusal.line))
        .collect();
    assert_eq!(refused, [("docs", 1), ("notes", 1)]);
    for refusal in &report.refusals {
        assert!(
            matches!(&refusal.reason, Reason::Conflict { message } if !message.is_empty()),
            "{refusal:?}"
        );
    }
    assert_eq!(originals(&database, "garden"), [Digest::of(&mulch)]);
}
