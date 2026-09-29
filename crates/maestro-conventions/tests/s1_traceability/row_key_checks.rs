//! The spec and 08 §20.4 carry exactly the approved keys, each an 08 row.

use super::keys::ROW_KEYS;
use maestro_conventions::root;
use std::{collections::BTreeSet, fs};

/// The architecture document whose rows the keys name.
const TRACEABILITY: &str = "docs/architecture/08-traceability.md";

/// The S1 spec, whose Traceability section lists the keys.
const SPEC: &str = "specs/001-knowledge-kernel/spec.md";

/// The S1 tasks, whose headings name the task IDs a mapping may cite.
const TASKS: &str = "specs/001-knowledge-kernel/tasks.md";

/// The repository roots a cited test or check path may start with.
const EVIDENCE_ROOTS: &[&str] = &[
    "crates/",
    "tests/",
    ".github/",
    ".cargo/",
    "maestro-quality.toml",
];

/// Read a repository file as text.
fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The lines from `heading` up to the next heading of its level or above.
fn section<'a>(markdown: &'a str, heading: &str) -> Vec<&'a str> {
    let level = heading
        .chars()
        .take_while(|&character| character == '#')
        .count();
    markdown
        .lines()
        .skip_while(|line| *line != heading)
        .skip(1)
        .take_while(|line| {
            let hashes = line
                .chars()
                .take_while(|&character| character == '#')
                .count();
            hashes == 0 || hashes > level || !line[hashes..].starts_with(' ')
        })
        .collect()
}

/// The trimmed cells of each table row, without splitting on escaped pipes.
fn table_rows<'a>(lines: &[&'a str]) -> Vec<Vec<&'a str>> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix('|')?.strip_suffix('|'))
        .map(|line| line.split('|').map(str::trim).collect())
        .collect()
}

/// The key a section cell and a row cell spell, backticks removed.
fn key(section_cell: &str, row_cell: &str) -> String {
    format!("{section_cell} | {}", row_cell.trim_matches('`'))
}

/// Every missing, duplicate or extra key of `listed` against the approved list.
fn key_problems(listed: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for listed_key in listed {
        if !seen.insert(listed_key.as_str()) {
            problems.push(format!("duplicate: {listed_key}"));
        }
        if !ROW_KEYS.contains(&listed_key.as_str()) {
            problems.push(format!("extra: {listed_key}"));
        }
    }
    for approved in ROW_KEYS {
        if !seen.contains(approved) {
            problems.push(format!("missing: {approved}"));
        }
    }
    problems
}

/// Every 08 row as a key: its numbered section and its first cell.
fn traceability_rows(markdown: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut current = String::new();
    for line in markdown.lines() {
        if let Some(heading) = line
            .strip_prefix("## ")
            .or_else(|| line.strip_prefix("### "))
        {
            let number = heading.split(' ').next().unwrap_or_default();
            current = format!("§{}", number.trim_end_matches('.'));
        } else if let Some(row) = table_rows(&[line]).first() {
            rows.push(key(&current, row[0]));
        }
    }
    rows
}

/// The §20.4 rows of 08, each with its section cell first.
fn mapping_rows(markdown: &str) -> Vec<Vec<&str>> {
    table_rows(&section(markdown, "### 20.4 Approved S1 row keys"))
        .into_iter()
        .filter(|cells| cells[0].starts_with('§'))
        .collect()
}

/// The task IDs `cell` names, such as `T019` in `T019–T037`.
fn task_ids(cell: &str) -> Vec<&str> {
    cell.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| {
            word.len() == 4
                && word.starts_with('T')
                && word[1..]
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
        .collect()
}

/// The repository paths `cell` cites in backticks.
fn cited_paths(cell: &str) -> Vec<&str> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .filter(|quoted| EVIDENCE_ROOTS.iter().any(|root| quoted.starts_with(root)))
        .collect()
}

/// A placeholder is not delivery evidence or a remaining disposition.
fn is_placeholder(cell: &str) -> bool {
    matches!(cell.trim(), "" | "None" | "TBD")
}

/// A private receipt is named without copying its contents into the repository.
fn has_private_receipt(cell: &str) -> bool {
    cell.split_once("Private:")
        .is_some_and(|(_, receipt)| !is_placeholder(receipt))
}

/// Commits name a quoted Git hash, a private receipt or every S1 commit.
fn has_commit_evidence(cell: &str) -> bool {
    has_private_receipt(cell)
        || cell.starts_with("Every S1 commit")
        || cell
            .split_inclusive('`')
            .skip(1)
            .step_by(2)
            .filter_map(|quoted| quoted.strip_suffix('`'))
            .any(|hash| {
                (7..=40).contains(&hash.len()) && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
}

/// A fully deferred row names its later slice, such as S2 or S7-I1.
fn names_slice(cell: &str) -> bool {
    cell.split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|word| word.strip_prefix('S'))
        .any(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
}

/// What one §20.4 row fails to name: portion, tasks, commits, tests, remainder.
fn row_problems(cells: &[&str], tasks: &BTreeSet<&str>) -> Vec<String> {
    let [
        section_cell,
        row_cell,
        portion,
        task_cell,
        commits,
        tests,
        remainder,
    ] = cells
    else {
        return vec![format!("{cells:?}: expected seven cells")];
    };
    let name = key(section_cell, row_cell);
    let mut problems = Vec::new();
    let portion_kind = portion
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches([':', ',']);
    let delivered = matches!(portion_kind, "Whole" | "Part");
    let ids = task_ids(task_cell);
    let paths = cited_paths(tests);
    let remainder_present = match portion_kind {
        "Part" => !is_placeholder(remainder),
        "None" => names_slice(remainder),
        _ => !remainder.is_empty(),
    };
    for (column, cell, valid) in [
        ("portion", portion, true),
        (
            "tasks",
            task_cell,
            !delivered || *task_cell == "All" || !ids.is_empty(),
        ),
        (
            "commits",
            commits,
            !delivered || has_commit_evidence(commits),
        ),
        (
            "tests",
            tests,
            !delivered || has_private_receipt(tests) || !paths.is_empty(),
        ),
        ("remainder", remainder, remainder_present),
    ] {
        if cell.is_empty() || !valid {
            problems.push(format!("{name}: no {column}"));
        }
    }
    for id in ids {
        if !tasks.contains(id) {
            problems.push(format!("{name}: no task {id}"));
        }
    }
    for path in paths {
        if !root().join(path).exists() {
            problems.push(format!("{name}: no path {path}"));
        }
    }
    problems
}

#[test]
fn the_approved_list_holds_94_distinct_keys() {
    let distinct: BTreeSet<_> = ROW_KEYS.iter().collect();
    assert_eq!((ROW_KEYS.len(), distinct.len()), (94, 94));
}

#[test]
fn every_approved_key_is_one_row_of_its_08_section() {
    let rows = traceability_rows(&read(TRACEABILITY));
    let wrong: Vec<String> = ROW_KEYS
        .iter()
        .filter_map(|approved| {
            let found = rows.iter().filter(|row| row == approved).count();
            (found != 1).then(|| format!("{approved}: {found} rows"))
        })
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_spec_lists_exactly_the_approved_keys() {
    let spec = read(SPEC);
    let listed: Vec<String> = table_rows(&section(&spec, "## Traceability"))
        .into_iter()
        .filter(|cells| cells.get(1).is_some_and(|cell| cell.starts_with('§')))
        .map(|cells| key(cells[1], cells.get(2).copied().unwrap_or_default()))
        .collect();
    let problems = key_problems(&listed);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn section_20_4_maps_exactly_the_approved_keys() {
    let traceability = read(TRACEABILITY);
    let listed: Vec<String> = mapping_rows(&traceability)
        .into_iter()
        .map(|cells| key(cells[0], cells.get(1).copied().unwrap_or_default()))
        .collect();
    let problems = key_problems(&listed);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn each_mapping_names_its_portion_tasks_evidence_and_remainder() {
    let tasks_text = read(TASKS);
    let tasks: BTreeSet<&str> = tasks_text
        .lines()
        .filter_map(|line| line.strip_prefix("### "))
        .filter_map(|heading| heading.split(' ').next())
        .collect();
    let traceability = read(TRACEABILITY);
    let problems: Vec<String> = mapping_rows(&traceability)
        .iter()
        .flat_map(|cells| row_problems(cells, &tasks))
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn missing_duplicate_and_extra_keys_are_refused() {
    let mut listed: Vec<String> = ROW_KEYS[1..].iter().map(ToString::to_string).collect();
    listed.push(ROW_KEYS[1].to_owned());
    listed.push("§11.3 | rag.N011 techniques".to_owned());
    assert_eq!(
        key_problems(&listed),
        [
            format!("duplicate: {}", ROW_KEYS[1]),
            "extra: §11.3 | rag.N011 techniques".to_owned(),
            format!("missing: {}", ROW_KEYS[0]),
        ]
    );
}

#[test]
fn a_row_without_evidence_or_with_an_unknown_task_is_refused() {
    let tasks = BTreeSet::from(["T001"]);
    let cells = [
        "§14",
        "`core storage`",
        "Whole",
        "T001, T999",
        "",
        "`crates/none`",
        "None",
    ];
    assert_eq!(
        row_problems(&cells, &tasks),
        [
            "§14 | core storage: no commits",
            "§14 | core storage: no task T999",
            "§14 | core storage: no path crates/none",
        ]
    );
    for portion in ["Whole", "Part: storage"] {
        for placeholder in ["", "None", "TBD"] {
            for (column, name) in [(3, "tasks"), (4, "commits"), (5, "tests")] {
                let mut row = [
                    "§14",
                    "`core storage`",
                    portion,
                    "T001",
                    "`b771872`",
                    "`maestro-quality.toml`",
                    "Other storage: S2",
                ];
                row[column] = placeholder;
                assert!(
                    row_problems(&row, &tasks).contains(&format!("§14 | core storage: no {name}")),
                    "{portion}: {name} accepted {placeholder:?}"
                );
            }
        }
    }
    for portion in ["Part: storage", "None delivered"] {
        for remainder in ["", "None", "TBD"] {
            let row = [
                "§14",
                "`core storage`",
                portion,
                "T001",
                "`b771872`",
                "`maestro-quality.toml`",
                remainder,
            ];
            assert!(
                row_problems(&row, &tasks).contains(&"§14 | core storage: no remainder".to_owned()),
                "{portion}: remainder accepted {remainder:?}"
            );
        }
    }
}

#[test]
fn evidence_requires_a_commit_or_receipt_and_an_existing_path_or_receipt() {
    let tasks = BTreeSet::from(["T001"]);
    for (commits, tests, valid) in [
        ("`b771872`", "`maestro-quality.toml`", true),
        (
            "`b738975eac03180f75cdfaad848847c00e868aa6`",
            "`.cargo/mutants.toml`",
            true,
        ),
        (
            "Private: the mapping receipt",
            "Private: the mapping checks",
            true,
        ),
        (
            "Every S1 commit",
            "`crates/maestro-conventions/tests`",
            true,
        ),
        ("`123456`", "`maestro-quality.toml`", false),
        (
            "`b738975eac03180f75cdfaad848847c00e868aa60`",
            "`maestro-quality.toml`",
            false,
        ),
        ("`not-hex`", "`maestro-quality.toml`", false),
        ("b771872", "`maestro-quality.toml`", false),
        ("`b771872", "`maestro-quality.toml`", false),
        ("Private:", "`maestro-quality.toml`", false),
        ("`b771872`", "Private:", false),
        ("`b771872`", "the tests pass", false),
        ("`b771872`", "`maestro-quality-missing.toml`", false),
    ] {
        let row = [
            "§14",
            "`core storage`",
            "Whole",
            "All",
            commits,
            tests,
            "None",
        ];
        let problems = row_problems(&row, &tasks);
        assert_eq!(
            problems.is_empty(),
            valid,
            "{commits}; {tests}: {problems:?}"
        );
    }
    for (remainder, valid) in [("Research loop: S2", true), ("Research loop", false)] {
        let row = [
            "§11.5",
            "`rag.N016 agent`",
            "None delivered",
            "None",
            "None",
            "None",
            remainder,
        ];
        assert_eq!(row_problems(&row, &tasks).is_empty(), valid, "{remainder}");
    }
}
