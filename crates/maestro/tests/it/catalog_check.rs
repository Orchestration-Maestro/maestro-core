//! `catalog check --catalog-dir DIR`: the strict source checker as its
//! authors run it, on the synthetic catalog and on a refused neighbour.

use super::support::Home;
use serde_json::json;
use std::{fs, path::PathBuf};

/// The synthetic source fixtures, where each file of the valid catalog lives.
const VALID: [(&str, &str); 6] = [
    ("core/agents/valid.agent.md", "valid.agent.md"),
    ("core/agents/valid.maestro.toml", "valid.maestro.toml"),
    ("skills/valid-skill/SKILL.md", "valid-skill/SKILL.md"),
    (
        "core/instructions/valid.instructions.md",
        "valid.instructions.md",
    ),
    (
        "core/instructions/valid.maestro.toml",
        "valid.instructions.maestro.toml",
    ),
    ("presets/knowledge-client.toml", "preset.toml"),
];

/// Writes the valid synthetic catalog under `home`, and returns its root.
fn valid_catalog(home: &Home) -> PathBuf {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog/source");
    let root = home.root().join("catalog");
    for (file, fixture) in VALID {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = fs::read_to_string(fixtures.join(fixture)).unwrap();
        let text = match fixture {
            "valid.agent.md" => text.replace("\"maestro/knowledge_search\", ", ""),
            "preset.toml" => format!(
                "name = \"knowledge-client\"\n{}",
                text.replace(", \"mcp:maestro\"", "")
            ),
            _ => text,
        };
        fs::write(path, text).unwrap();
    }
    root
}

#[test]
fn catalog_check_passes_the_valid_catalog() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let result = home.run(&["catalog", "check", "--catalog-dir", root.to_str().unwrap()]);
    assert_eq!(
        (result.code, result.stdout.as_str(), result.stderr.as_str()),
        (Some(0), "catalog check passed: 4 resources\n", ""),
    );
}

#[test]
fn catalog_check_reports_each_resource_under_json() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let result = home.run(&[
        "--json",
        "catalog",
        "check",
        "--catalog-dir",
        root.to_str().unwrap(),
    ]);
    assert_eq!((result.code, result.stderr.as_str()), (Some(0), ""));
    let document = result.json();
    assert_eq!(document["schema"], "maestro-cli/catalog-check/1");
    assert_eq!(document["status"], "passed");
    assert_eq!(
        document["resources"][0],
        json!({
            "id": "agent:valid",
            "path": "core/agents/valid.agent.md",
            "owner": "@synthetic/knowledge",
            "maturity": "reviewed",
        })
    );
    assert_eq!(document["diagnostics"], json!([]));
}

#[test]
fn catalog_check_refuses_an_invalid_agent_with_exit_2() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog/source");
    fs::copy(
        fixtures.join("invalid.agent.md"),
        root.join("core/agents/valid.agent.md"),
    )
    .unwrap();
    let result = home.run(&["catalog", "check", "--catalog-dir", root.to_str().unwrap()]);
    assert_eq!((result.code, result.stdout.as_str()), (Some(2), ""));
    assert!(
        result
            .stderr
            .contains("core/agents/valid.agent.md: metadata: unknown key\n"),
        "{result:?}"
    );
}

#[test]
fn catalog_check_fails_with_exit_1_on_a_missing_directory() {
    let home = Home::bare();
    let root = home.root().join("absent");
    let result = home.run(&["catalog", "check", "--catalog-dir", root.to_str().unwrap()]);
    assert_eq!(result.code, Some(1), "{result:?}");
    assert!(
        result
            .stderr
            .starts_with("catalog: cannot list the catalog directory:"),
        "{result:?}"
    );
}

#[test]
fn catalog_check_reports_an_unreadable_directory_as_failed_under_json() {
    let home = Home::bare();
    let root = home.root().join("absent");
    let result = home.run(&[
        "--json",
        "catalog",
        "check",
        "--catalog-dir",
        root.to_str().unwrap(),
    ]);
    assert_eq!(result.code, Some(1), "{result:?}");
    let document = result.json();
    assert_eq!(document["status"], "failed");
    assert!(
        document["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("cannot list the catalog directory:"),
        "{document}"
    );
}
