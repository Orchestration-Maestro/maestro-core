//! C00's exact S3 inventory: design dispositions, never delivery evidence.
#![cfg(test)]

use maestro_conventions::root;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};

/// Exact architecture 08 keys approved by the owner, 2026-09-28 (OA7).
/// Combined keys stay intact.
pub(super) const ROW_KEYS: &[&str] = &[
    "owner.catalog",
    "owner.m001.manifest",
    "owner.m001.team",
    "owner.m001.cli",
    "owner.m001.load",
    "owner.m001.laptop",
    "owner.m001.guardrails",
    "owner.m024",
    "owner.m028",
    "owner.m032",
    "chat.M006 layers",
    "chat.M006 layout, M023 layout",
    "chat.M006 compiler",
    "chat.M006 explain",
    "chat.M006 lockfile",
    "chat.M006 project file",
    "chat.M006 transparency",
    "chat.M006 release",
    "chat.M019 maturity, M023 step 4",
    "chat.M019 descriptor",
    "chat.M019 ownership",
    "chat.M027 detached",
    "chat.M027 validation split",
    "chat.M027 invariants",
    "chat.M027 contract crate",
    "chat.M027 versions",
    "chat.M027 pipelines",
    "chat.M027 TUF",
    "chat.M036 classes, M039 policy",
    "chat.M036 defaults",
    "chat.M006 roles, M023 step 12",
    "chat.M006 change workflow",
    "chat.M019 machine contracts",
    "chat.M039 superpowers",
    "delivery.R08",
    "chat.M036 models",
    "chat.M059 model profile",
    "chat.M006 broker",
    "chat.M006, M023 step 8",
    "chat.M006 destructive",
    "chat.M006 approvals",
    "chat.M023 step 7",
    "chat.M031 principle",
    "chat.M031 workflow first",
    "chat.M031 API",
    "chat.M031 cards",
    "chat.M031 hybrid",
    "chat.M031 optimal",
    "chat.M031 separation",
    "chat.M031 offline",
    "chat.M031 publication",
    "chat.M031 security",
    "chat.M031 delivery",
    "chat.M031 service",
    "chat.M019 bootstrap, M023 step 13",
    "chat.M019 overlays",
    "chat.M006 native",
    "product.GD2, GD4, GD5",
    "chat.M048 real controls",
    "chat.M057 CI",
    "chat.M059 provenance",
    "delivery.U02",
    "delivery.U05",
    "delivery.U09",
    "delivery.U13",
    "delivery.U17",
    "core storage",
    "core verification",
    "delivery.§1.5",
    "chat.M006 CLI",
    "chat.M036 objects",
    "delivery.§2.2",
    "chat.M019 roles",
    "chat.M023 step 13",
    "delivery.C01 transparent platform, minimal plumbing",
    "delivery.C02 detached two-repository releases",
    "delivery.C04 readiness, owners, dependencies without invalid frontmatter",
    "delivery.C06 closure, merge, explanation, one override class",
    "delivery.C08 explicit instructions and skills with provenance",
    "delivery.C12 preview/apply bootstrap",
    "delivery.C13 native projection",
    "delivery.C14–C16 catalog MCP, shared Qdrant, separate knowledge ACLs",
    "delivery.C17 signed releases, freshness, revocation, transparency",
    "delivery.R01–R11 corrections",
    "product.GD1–GD5",
];

/// Read the real fixture at run time so a missing inventory fails the check.
pub(super) fn inventory() -> Value {
    let path = root().join("specs/003-catalog/traceability.json");
    let text = fs::read_to_string(path).expect("C00 traceability inventory must exist");
    serde_json::from_str(&text).expect("C00 traceability inventory must be valid JSON")
}

/// Require a named, nonempty portion rather than a null or whitespace marker.
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value[field]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing {field}"))
}

/// Read table cells without splitting or normalizing combined source keys.
pub(super) fn table_rows(markdown: &str) -> Vec<Vec<&str>> {
    markdown
        .lines()
        .filter_map(|line| line.strip_prefix("| "))
        .map(|line| line.split('|').map(str::trim).collect())
        .collect()
}

/// An exact source row must exist once; a substring or combined-key alias is not it.
fn exact_row<'a>(
    rows: &'a [Vec<&str>],
    key: &str,
    source: &str,
) -> Result<&'a Vec<&'a str>, String> {
    let matches: Vec<_> = rows
        .iter()
        .filter(|row| row[0].trim_matches('`') == key)
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "{source} row {key}: expected one, found {}",
            matches.len()
        ));
    }
    Ok(matches[0])
}

/// Validate the task references and explicit slice ownership of unfinished work.
fn check_disposition(row: &Value, tasks: &BTreeSet<&str>) -> Result<(), String> {
    text(row, "s3_portion")?;
    let task_ids = row["tasks"]
        .as_array()
        .filter(|ids| !ids.is_empty())
        .ok_or("missing tasks")?;
    let mut seen = BTreeSet::new();
    for task in task_ids {
        let id = task.as_str().ok_or("invalid task")?;
        if !seen.insert(id) || !tasks.contains(id) {
            return Err(format!("invalid task {id}"));
        }
    }
    let mut remaining_slices = BTreeSet::new();
    for remainder in text(row, "remaining")?.split(';') {
        let (slice, portion) = remainder
            .trim()
            .split_once(':')
            .ok_or("unnamed remaining slice")?;
        if !["S0", "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8"].contains(&slice)
            || portion.trim().is_empty()
        {
            return Err(format!("unnamed remaining slice: {remainder}"));
        }
        remaining_slices.insert(slice);
    }
    for slice in slice_names(text(row, "s3_portion")?) {
        if !remaining_slices.contains(slice) {
            return Err(format!("missing remaining slice {slice}"));
        }
    }
    Ok(())
}

/// Recognize named slices without mistaking S30 or identifiers for S3.
fn slice_names(text: &str) -> BTreeSet<&str> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| matches!(word.as_bytes(), [b'S', b'0'..=b'8']))
        .collect()
}

/// Derive candidates from source table columns, not from the frozen key copies.
pub(super) fn source_candidates(markdown: &str) -> BTreeSet<&str> {
    let mut candidates = BTreeSet::new();
    let mut columns = None;
    for line in markdown.lines() {
        let Some(line) = line.strip_prefix("| ") else {
            columns = None;
            continue;
        };
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        let (location, status) = *columns.get_or_insert_with(|| {
            (
                cells
                    .iter()
                    .position(|cell| matches!(*cell, "Where" | "Where now")),
                cells
                    .iter()
                    .position(|cell| matches!(*cell, "Status" | "Slice now")),
            )
        });
        let location = location.and_then(|i| cells.get(i)).copied().unwrap_or("");
        let status = status.and_then(|i| cells.get(i)).copied().unwrap_or("");
        if location.contains("03-agent-orchestration.md#1")
            || location.contains("06-roadmap.md#s3-catalog--m3")
            || slice_names(status).contains("S3")
        {
            candidates.insert(cells[0].trim_matches('`'));
        }
    }
    candidates
}

/// Every derived candidate is included or explicitly excluded with a reason.
fn check_candidates(
    inventory: &Value,
    architecture: &str,
    included: &BTreeSet<&str>,
) -> Result<(), String> {
    let candidates = source_candidates(architecture);
    let rows = table_rows(architecture);
    let mut covered = included.clone();
    for exclusion in inventory["excluded"]
        .as_array()
        .ok_or("missing exclusions")?
    {
        let key = text(exclusion, "source_row")?;
        text(exclusion, "reason")?;
        if !candidates.contains(key) {
            return Err(format!("extra exclusion {key}"));
        }
        if !covered.insert(key) {
            return Err(format!("duplicate disposition {key}"));
        }
        exact_row(&rows, key, "source")?;
    }
    if let Some(candidate) = candidates.difference(&covered).next() {
        return Err(format!("unmapped S3 candidate {candidate}"));
    }
    Ok(())
}

/// Check inventory integrity against an independent key list and the real documents.
pub(super) fn check_inventory(
    inventory: &Value,
    architecture: &str,
    spec: &str,
    tasks: &str,
) -> Result<(), String> {
    if inventory["status"] != "planned" {
        return Err("C00 is not delivery evidence".into());
    }
    let rows = inventory["rows"].as_array().ok_or("missing rows")?;
    let task_ids = tasks
        .lines()
        .filter_map(|line| line.strip_prefix("### ")?.split_whitespace().next())
        .filter(|id| {
            matches!(
                id.as_bytes(),
                [b'C', b'0'..=b'9', b'0'..=b'9'] | [b'C', b'0'..=b'9', b'0'..=b'9', b'a'..=b'z']
            )
        })
        .collect();
    let mut seen = BTreeSet::new();
    for row in rows {
        let key = text(row, "source_row")?;
        if !ROW_KEYS.contains(&key) {
            return Err(format!("extra key {key}"));
        }
        if !seen.insert(key) {
            return Err(format!("duplicate key {key}"));
        }
        check_disposition(row, &task_ids)?;
    }
    let expected = ROW_KEYS.iter().copied().collect::<BTreeSet<_>>();
    if seen != expected {
        return Err(format!(
            "missing keys: {:?}",
            expected.difference(&seen).collect::<Vec<_>>()
        ));
    }
    let architecture_rows = table_rows(architecture);
    let spec = table_rows(spec);
    for row in rows {
        let key = text(row, "source_row")?;
        exact_row(&architecture_rows, key, "source")?;
        let spec_row = exact_row(&spec, key, "spec")?;
        if spec_row.get(1) != Some(&text(row, "s3_portion")?) {
            return Err(format!("S3 portion differs for {key}"));
        }
        let spec_tasks: Vec<_> = spec_row
            .get(2)
            .ok_or("missing spec tasks")?
            .split(", ")
            .collect();
        if row["tasks"] != json!(spec_tasks) {
            return Err(format!("tasks differ for {key}"));
        }
    }
    // A simultaneous spec/fixture deletion or an extra spec row must also fail.
    let spec_keys: BTreeSet<_> = spec
        .iter()
        .filter_map(|row| row[0].strip_prefix('`'))
        .map(|key| key.trim_end_matches('`'))
        .collect();
    if spec_keys != expected {
        return Err("spec inventory differs".into());
    }
    check_candidates(inventory, architecture, &seen)
}

/// Use only the traceability table, not unrelated Markdown tables in the specification.
pub(super) fn documents() -> (String, String, String) {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let spec = read("specs/003-catalog/spec.md");
    let traceability = spec
        .split_once("## Traceability\n")
        .unwrap()
        .1
        .split_once("## Assumptions\n")
        .unwrap()
        .0
        .to_owned();
    (
        read("docs/architecture/08-traceability.md"),
        traceability,
        read("specs/003-catalog/tasks.md"),
    )
}

#[test]
fn catalog_traceability_inventory_matches_exact_rows_and_dispositions() {
    let (architecture, spec, tasks) = documents();
    assert_eq!(
        check_inventory(&inventory(), &architecture, &spec, &tasks),
        Ok(())
    );
}

#[test]
fn catalog_traceability_rejects_missing_duplicate_and_extra_keys() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for index in 0..ROW_KEYS.len() {
        let mut missing = original.clone();
        missing["rows"].as_array_mut().unwrap().remove(index);
        let mut duplicate = original.clone();
        let repeated = duplicate["rows"][index].clone();
        duplicate["rows"].as_array_mut().unwrap().push(repeated);
        let mut extra = original.clone();
        extra["rows"][index]["source_row"] = json!("owner.not-in-s3");
        for (name, mutated) in [
            ("missing keys", missing),
            ("duplicate key", duplicate),
            ("extra key", extra),
        ] {
            let error = check_inventory(&mutated, &architecture, &spec, &tasks).unwrap_err();
            assert!(error.starts_with(name), "{index}: {error}");
        }
    }
}

/// Assert the intended guard, not a later comparison that happens to reject too.
pub(super) fn rejected(result: Result<(), String>, prefix: &str) {
    let error = result.expect_err(prefix);
    assert!(error.starts_with(prefix), "expected {prefix}, got {error}");
}

#[test]
fn catalog_traceability_rejects_incomplete_dispositions() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for index in 0..ROW_KEYS.len() {
        for (field, invalid, prefix) in [
            ("s3_portion", json!(" "), "missing s3_portion"),
            ("tasks", json!([]), "missing tasks"),
            ("tasks", json!(["C99"]), "invalid task C99"),
            ("remaining", json!(""), "missing remaining"),
            ("remaining", json!("later"), "unnamed remaining slice"),
            (
                "remaining",
                json!("S99: unspecified"),
                "unnamed remaining slice",
            ),
            ("remaining", json!("S4: "), "unnamed remaining slice"),
        ] {
            let mut mutated = original.clone();
            mutated["rows"][index][field] = invalid;
            rejected(
                check_inventory(&mutated, &architecture, &spec, &tasks),
                prefix,
            );
            mutated["rows"][index]
                .as_object_mut()
                .unwrap()
                .remove(field);
            rejected(
                check_inventory(&mutated, &architecture, &spec, &tasks),
                &format!("missing {field}"),
            );
        }
    }
}

#[test]
fn catalog_traceability_rejects_changed_source_keys_and_delivery_claims() {
    let original = inventory();
    let (architecture, spec, tasks) = documents();
    for key in ROW_KEYS {
        let needle = format!("| {key} |");
        let renamed = architecture.replace(&needle, "| unrelated |");
        rejected(
            check_inventory(&original, &renamed, &spec, &tasks),
            "source row",
        );
        let duplicate = format!("{architecture}\n{needle} duplicate |\n");
        rejected(
            check_inventory(&original, &duplicate, &spec, &tasks),
            "source row",
        );
    }
    let mut delivered = original;
    delivered["status"] = json!("delivered");
    rejected(
        check_inventory(&delivered, &architecture, &spec, &tasks),
        "C00 is not delivery evidence",
    );
}
