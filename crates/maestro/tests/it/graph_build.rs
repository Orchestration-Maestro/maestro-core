//! `knowledge graph build`: the frozen synthetic defaults table (plan A0),
//! imported under its own temporary declaration and manifest, then built
//! with its standalone rule into `DEFAULTS_TO` claims the kernel admits,
//! each quoting its row's original bytes. The home has no model router, no
//! search service and no graph service: the build needs none. A rebuild
//! records nothing new, a changed rule changes the frozen inputs, and the
//! test envelope, a rule for other bytes and an unknown collection are
//! refused with exit code 2.

use super::support::{Ended, Home, local};
use maestro_kernel::artifact::Digest;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// How the document printed under `--json` starts: with its schema.
const SCHEMA_FIRST: &str = r#"{"schema":"maestro-cli/knowledge-graph-build/1","#;

/// The collection the temporary declaration names.
const COLLECTION: &str = "synthetic-graph";

/// The frozen fixture directory, `tests/fixtures/synthetic/graph`.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/synthetic/graph")
}

/// The frozen envelope: the standalone rule and its test oracle.
fn envelope() -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures().join("defaults.json")).unwrap()).unwrap()
}

/// Writes `json` to `name` in `home`'s own directory and returns its path.
fn write(home: &Home, name: &str, json: &Value) -> PathBuf {
    let path = home.root().join(name);
    fs::write(&path, json.to_string()).unwrap();
    path
}

/// The path as the command line takes it.
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// Asserts `ended` exited with `code`.
fn exited(ended: &Ended, code: i32) {
    assert_eq!(ended.code, Some(code), "{ended:?}");
}

/// Writes `markdown` as the pilot's one document under `root`, with its
/// manifest line: plan A0's fixed metadata, and the digest and size of
/// `markdown`, which are A0's for the frozen Markdown.
fn corpus(root: &Path, markdown: &[u8]) {
    fs::create_dir_all(root.join("corpus/graph")).unwrap();
    fs::write(root.join("corpus/graph/defaults.md"), markdown).unwrap();
    let manifest = json!({
        "schema": "maestro-corpus/1",
        "path": "graph/defaults.md",
        "sha256": Digest::of(markdown).as_str(),
        "bytes": markdown.len(),
        "source_ref": "corpus-path:graph/defaults.md",
        "title": "Lantern controller parameters",
        "version": "1.0",
        "set": "graph",
        "source_kind": "reference"
    });
    fs::write(
        root.join("corpus/maestro-corpus.jsonl"),
        format!("{manifest}\n"),
    )
    .unwrap();
}

/// Imports the frozen Markdown in `home` as plan A0's temporary collection
/// `synthetic-graph`, gives it its quality disposition when `gate`, and
/// returns the path of the standalone rule.
fn pilot_gated(home: &Home, gate: bool) -> PathBuf {
    let root = home.root().join("graph-root");
    let markdown = fs::read(fixtures().join("defaults.md")).unwrap();
    assert_eq!(
        (Digest::of(&markdown).as_str(), markdown.len()),
        (
            "8cfbf93dbaa5dc25bf9c3a6d88f1b698a19c7c79c6bb832320977e95220766a3",
            245
        )
    );
    corpus(&root, &markdown);
    let declaration = json!({
        "schema": "maestro-collection/1",
        "id": COLLECTION,
        "title": "Synthetic graph defaults",
        "visibility": "public",
        "profiles": {
            "extraction": "technical-html/1",
            "chunking": "structural-500-700/1",
            "embedding": "embed:winner",
            "sparse": "bm25-en-fr/1"
        },
        "quality": {"ledger": "quality/ledger.jsonl"},
        "sources": [{
            "id": "defaults", "kind": "import", "sync": "manual",
            "manifest": {"binding": "synthetic_graph_root", "path": "corpus/maestro-corpus.jsonl"}
        }],
        "evals": {"suite": "evals"}
    });
    fs::write(root.join("collection.json"), declaration.to_string()).unwrap();
    fs::write(
        home.config().join("bindings.toml"),
        format!("synthetic_graph_root = '{}'\n", root.display()),
    )
    .unwrap();
    let declaration = root.join("collection.json");
    exited(
        &home.run(&["knowledge", "collection", "add", text(&declaration)]),
        0,
    );
    import(home);
    if gate {
        quality(home);
    }
    write(home, "rule.json", &envelope()["rule"])
}

/// [`pilot_gated`], its quality disposition given.
fn pilot(home: &Home) -> PathBuf {
    pilot_gated(home, true)
}

/// Imports the pilot collection in `home`.
fn import(home: &Home) {
    exited(
        &home.run(&["knowledge", "import", "--collection", COLLECTION]),
        0,
    );
}

/// Gives the pilot collection's revisions in `home` their disposition.
fn quality(home: &Home) {
    exited(
        &home.run(&["knowledge", "quality", "--collection", COLLECTION]),
        0,
    );
}

/// Runs `knowledge graph build` of the pilot collection with `rule`.
fn build(home: &Home, rule: &Path) -> Ended {
    home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(rule),
        "--json",
    ])
}

/// Asserts that `built`, the printed build, holds the oracle's `expected`
/// defaults, unreviewed, and four unambiguous `Parameter` entities.
fn printed(built: &Value, expected: &[Value]) {
    assert_eq!(built["schema"], "maestro-cli/knowledge-graph-build/1");
    assert_eq!(built["collection"], COLLECTION);
    assert_eq!(built["extractor"]["id"], "synthetic-defaults/1");
    assert_eq!(built["extractor"]["profile"], built["claims"][0]["profile"]);
    assert_eq!(built["rejections"], json!([]));
    let claims = built["claims"].as_array().unwrap();
    assert_eq!(claims.len(), expected.len());
    for (claim, expected) in claims.iter().zip(expected) {
        printed_claim(claim, expected);
    }
    let entities = built["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 4);
    for entity in entities {
        assert_eq!(entity["kind"], "Parameter");
        assert_eq!(entity["colliding"], json!([]));
    }
}

/// Asserts that `claim`, as printed, is the oracle's `expected` default,
/// unreviewed, with its one support.
fn printed_claim(claim: &Value, expected: &Value) {
    assert_eq!(
        claim["subject"],
        json!({"kind": "Parameter", "name": expected["subject"]})
    );
    assert_eq!(claim["predicate"], "DEFAULTS_TO");
    assert_eq!(claim["object"], expected["object"]);
    assert_eq!(claim["review"], "unreviewed");
    assert_eq!(claim["supports"].as_array().unwrap().len(), 1);
    assert_eq!(claim["supports"][0]["span"], expected["span"]);
    assert_eq!(
        claim["supports"][0]["quote_sha256"],
        expected["quote_sha256"]
    );
}

/// Asserts that the kernel of `home` holds the set `built` printed, each
/// support quoting the original's bytes exactly as the oracle `expected`.
fn recorded(home: &Home, built: &Value, expected: &[Value]) {
    let database = home.database();
    let id = Digest::parse(built["claim_set"].as_str().unwrap()).unwrap();
    let set = database.claim_set(&local(&database), &id).unwrap().unwrap();
    assert_eq!(set.claims.len(), expected.len());
    let original = fs::read(fixtures().join("defaults.md")).unwrap();
    for (record, expected) in set.claims.iter().zip(expected) {
        let support = &record.claim.supports[0];
        let quote = &original[support.span.start..support.span.end];
        assert_eq!(quote, expected["quote"].as_str().unwrap().as_bytes());
        assert_eq!(support.quote_digest, Digest::of(quote));
        assert_eq!(record.claim.object.lexeme, expected["object"]["lexeme"]);
    }
}

#[test]
fn the_rule_admits_the_four_oracle_defaults_through_the_kernel() {
    let home = Home::new();
    let rule = pilot(&home);
    let ended = build(&home, &rule);
    exited(&ended, 0);
    assert!(ended.stdout.starts_with(SCHEMA_FIRST), "{ended:?}");
    let expected = envelope()["expected"].as_array().unwrap().clone();
    printed(&ended.json(), &expected);
    recorded(&home, &ended.json(), &expected);
}

#[test]
fn a_rebuild_records_nothing_new_and_a_changed_rule_changes_the_inputs() {
    let home = Home::new();
    let rule = pilot(&home);
    let first = build(&home, &rule);
    exited(&first, 0);
    let again = build(&home, &rule);
    exited(&again, 0);
    assert_eq!(first.json()["claim_set"], again.json()["claim_set"]);
    let mut changed = envelope()["rule"].clone();
    changed["id"] = json!("synthetic-defaults/2");
    let other = build(&home, &write(&home, "changed.json", &changed));
    exited(&other, 0);
    assert_ne!(
        first.json()["extractor"]["profile"],
        other.json()["extractor"]["profile"]
    );
    assert_ne!(first.json()["claim_set"], other.json()["claim_set"]);
}

#[test]
fn the_envelope_a_rule_for_other_bytes_and_an_unknown_collection_are_refused() {
    let home = Home::new();
    pilot(&home);
    let refused = build(&home, &write(&home, "envelope.json", &envelope()));
    exited(&refused, 2);
    assert!(refused.stderr.contains("unknown field"), "{refused:?}");
    let mut other = envelope()["rule"].clone();
    other["source_sha256"] = json!(Digest::of(b"other bytes").as_str());
    let refused = build(&home, &write(&home, "other.json", &other));
    exited(&refused, 2);
    assert!(
        refused.stderr.contains("no eligible revision"),
        "{refused:?}"
    );
    let rule = write(&home, "rule.json", &envelope()["rule"]);
    let unknown = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        "missing",
        "--rule",
        text(&rule),
    ]);
    exited(&unknown, 2);
}

#[test]
fn only_the_latest_accepted_revision_is_read() {
    let home = Home::new();
    let rule = pilot_gated(&home, false);
    let refused = build(&home, &rule);
    exited(&refused, 2);
    assert!(
        refused.stderr.contains("no eligible revision"),
        "{refused:?}"
    );
    quality(&home);
    exited(&build(&home, &rule), 0);
    let changed = fs::read_to_string(fixtures().join("defaults.md"))
        .unwrap()
        .replace("café", "cafe");
    corpus(&home.root().join("graph-root"), changed.as_bytes());
    import(&home);
    quality(&home);
    let refused = build(&home, &rule);
    exited(&refused, 2);
    assert!(
        refused.stderr.contains("no eligible revision"),
        "{refused:?}"
    );
}

#[test]
fn a_build_that_admits_nothing_prints_its_rejections_and_exits_2() {
    let home = Home::new();
    pilot(&home);
    let mut misleading = envelope()["rule"].clone();
    misleading["heading_path"] = json!(["Lantern controller", "Settings"]);
    let rule = write(&home, "misleading.json", &misleading);
    let refused = build(&home, &rule);
    exited(&refused, 2);
    assert!(refused.stdout.starts_with(SCHEMA_FIRST), "{refused:?}");
    let printed = refused.json();
    assert_eq!(printed["claim_set"], Value::Null);
    assert_eq!(printed["claims"], json!([]));
    let rejections = printed["rejections"].as_array().unwrap();
    assert_eq!(rejections.len(), 1, "{printed}");
    assert!(
        rejections[0]["reason"]
            .as_str()
            .unwrap()
            .contains("no table under")
    );
    assert_eq!(rejections[0]["block_id"], Value::Null);
    let text = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule),
    ]);
    exited(&text, 2);
    assert!(text.stdout.is_empty(), "{text:?}");
    assert!(
        text.stderr
            .contains("the rule admitted no claim: 1 rejected\nrejected rev-"),
        "{text:?}"
    );
    assert!(text.stderr.contains(" -: no table under"), "{text:?}");
}

#[test]
fn people_read_the_admitted_claims_one_per_line() {
    let home = Home::new();
    let rule = pilot(&home);
    let ended = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule),
    ]);
    exited(&ended, 0);
    let lines: Vec<&str> = ended.stdout.lines().collect();
    assert_eq!(lines.len(), 5, "{ended:?}");
    assert!(
        lines[0].starts_with("admitted 4 claims as the claim set "),
        "{ended:?}"
    );
    assert!(lines[0].ends_with("; 0 rejected"), "{ended:?}");
    assert_eq!(
        &lines[1..],
        [
            "label DEFAULTS_TO text café",
            "enabled DEFAULTS_TO boolean true",
            "retries DEFAULTS_TO integer 3",
            "ratio DEFAULTS_TO decimal 0.50",
        ]
    );
}

#[test]
fn a_failed_revision_is_not_read() {
    let home = Home::new();
    let rule = pilot(&home);
    let connection = Connection::open(home.data().join("kernel.sqlite3")).unwrap();
    connection
        .execute("UPDATE revisions SET status = 'failed'", [])
        .unwrap();
    drop(connection);
    let refused = build(&home, &rule);
    exited(&refused, 2);
    assert!(
        refused.stderr.contains("no eligible revision"),
        "{refused:?}"
    );
}

#[test]
fn an_unreadable_rule_file_is_refused() {
    let home = Home::new();
    pilot(&home);
    let refused = build(&home, &home.root().join("missing.json"));
    exited(&refused, 2);
    assert!(refused.stderr.contains("cannot be read"), "{refused:?}");
}
