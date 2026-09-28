//! Review regressions for source completeness and independent field guards.

use super::inventory_checks::{
    ROW_KEYS, check_inventory, documents, inventory, rejected, source_candidates, table_rows,
};
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn catalog_traceability_rejects_unmapped_architecture_candidates() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for (status, location) in [
        (
            "Kept",
            "[catalog](03-agent-orchestration.md#13-check-compile-release-install)",
        ),
        ("Kept", "[catalog](06-roadmap.md#s3-catalog--m3)"),
        ("Kept in S3", "elsewhere"),
    ] {
        let added = format!(
            "{architecture}\n| ID | Requirement | Status | Where |\n\
            | --- | --- | --- | --- |\n\
            | chat.new-s3-row | synthetic | {status} | {location} |\n"
        );
        rejected(
            check_inventory(&original, &added, &spec, &tasks),
            "unmapped S3 candidate chat.new-s3-row",
        );
    }
}

#[test]
fn catalog_traceability_candidates_use_source_columns_and_exact_slice_tokens() {
    let source = "| ID | Requirement | Status | Where |\n| --- | --- | --- | --- |\n\
        | chat.status | synthetic | Kept in S3 | elsewhere |\n\
        | chat.not-status | S3 mentioned in requirement | S4 | elsewhere |\n\
        | chat.not-slice | synthetic | S30 | elsewhere |\n\
        | chat.root | synthetic | Kept | \
        [catalog](03-agent-orchestration.md#1-the-catalog) |\n\
        | chat.innersource | synthetic | S5 | \
        [flow](03-agent-orchestration.md#10-innersource) |\n\n\
        | Earlier disposition | Where now |\n| --- | --- |\n\
        | delivery.link | [catalog](06-roadmap.md#s3-catalog--m3) |\n\n\
        | Earlier task | Content | Slice now |\n| --- | --- | --- |\n\
        | delivery.slice | synthetic | S1, S3 |\n\n\
        | Contract | Disposition |\n| --- | --- |\n| Not a source row | S3 |\n";
    assert_eq!(
        source_candidates(source),
        BTreeSet::from([
            "chat.status",
            "chat.root",
            "chat.innersource",
            "delivery.link",
            "delivery.slice"
        ])
    );
}

#[test]
fn catalog_traceability_rejects_invalid_exclusions() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for index in 0..original["excluded"].as_array().unwrap().len() {
        let mut missing = original.clone();
        missing["excluded"].as_array_mut().unwrap().remove(index);
        rejected(
            check_inventory(&missing, &architecture, &spec, &tasks),
            "unmapped S3 candidate",
        );
        let mut duplicate = original.clone();
        let repeated = duplicate["excluded"][index].clone();
        duplicate["excluded"].as_array_mut().unwrap().push(repeated);
        rejected(
            check_inventory(&duplicate, &architecture, &spec, &tasks),
            "duplicate disposition",
        );
        for reason in [json!(" "), json!(null)] {
            let mut incomplete = original.clone();
            incomplete["excluded"][index]["reason"] = reason;
            rejected(
                check_inventory(&incomplete, &architecture, &spec, &tasks),
                "missing reason",
            );
        }
    }
    let mut extra = original.clone();
    extra["excluded"][0]["source_row"] = json!("chat.not-a-candidate");
    rejected(
        check_inventory(&extra, &architecture, &spec, &tasks),
        "extra exclusion",
    );
    let mut overlap = original.clone();
    overlap["excluded"][0]["source_row"] = json!("owner.catalog");
    rejected(
        check_inventory(&overlap, &architecture, &spec, &tasks),
        "duplicate disposition",
    );
}

#[test]
fn catalog_traceability_rejects_spec_mutations() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for key in ROW_KEYS {
        let needle = format!("| `{key}` |");
        let line = spec.lines().find(|line| line.starts_with(&needle)).unwrap();
        let cells = table_rows(line);
        for (replacement, prefix) in [
            (
                line.replace(&needle, "| `chat.renamed` |"),
                format!("spec row {key}:"),
            ),
            (
                format!("{needle} different portion | {} |", cells[0][2]),
                format!("S3 portion differs for {key}"),
            ),
            (
                format!("{needle} {} | C99 |", cells[0][1]),
                format!("tasks differ for {key}"),
            ),
            (String::new(), format!("spec row {key}:")),
            (format!("{line}\n{line}"), format!("spec row {key}:")),
        ] {
            rejected(
                check_inventory(
                    &original,
                    &architecture,
                    &spec.replace(line, &replacement),
                    &tasks,
                ),
                &prefix,
            );
        }
    }
    let extra = format!("{spec}\n| `chat.extra` | synthetic | C00 |\n");
    rejected(
        check_inventory(&original, &architecture, &extra, &tasks),
        "spec inventory differs",
    );
}

#[test]
fn catalog_traceability_rejects_unknown_and_non_task_headings() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for id in [
        "C99",
        "Paths",
        "Requirements",
        "C9",
        "C999",
        "C0a",
        "C01ab",
        "C01A",
    ] {
        let mut mutated = original.clone();
        mutated["rows"][0]["tasks"] = json!([id]);
        let changed_spec = spec.replacen("C02, C21, C21b, C29", id, 1);
        let changed_tasks = if id == "C99" {
            tasks.clone()
        } else {
            format!("{tasks}\n### {id} not a task\n")
        };
        rejected(
            check_inventory(&mutated, &architecture, &changed_spec, &changed_tasks),
            &format!("invalid task {id}"),
        );
    }
}

#[test]
fn catalog_traceability_rejects_lost_remaining_slices() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for index in 0..ROW_KEYS.len() {
        let portion = original["rows"][index]["s3_portion"].as_str().unwrap();
        for slice in ["S0", "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8"] {
            if !portion
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|word| word == slice)
            {
                continue;
            }
            let mut mutated = original.clone();
            let remainder = original["rows"][index]["remaining"].as_str().unwrap();
            let kept = remainder
                .split(';')
                .filter(|part| !part.trim().starts_with(&format!("{slice}:")))
                .collect::<Vec<_>>()
                .join(";");
            // Keep syntax valid so only the lost-slice guard can reject this case.
            mutated["rows"][index]["remaining"] = json!(if kept.is_empty() {
                format!(
                    "{}: synthetic unrelated remainder",
                    if slice == "S0" { "S1" } else { "S0" }
                )
            } else {
                kept
            });
            rejected(
                check_inventory(&mutated, &architecture, &spec, &tasks),
                &format!("missing remaining slice {slice}"),
            );
        }
    }
}
