//! The collection this machine names, imported for real into the kernel's
//! own data directory and gated (T020, steps 3 and 4): an explicit local run,
//! never a pass when ignored.
//!
//! It reads `MAESTRO_COLLECTION`, the path of the collection's declaration,
//! and, when it is set, `MAESTRO_DISPOSITION_REPORT`, the file it writes the
//! disposition report to. Everything else is the kernel's own
//! (`maestro_kernel::paths`): the database and the artifact store in the
//! data directory, which it creates beside whatever else that directory
//! holds and touches nothing else there; and, in the configuration
//! directory, `bindings.toml`, which binds the corpus the declaration names,
//! and `config.toml`, which grants the local principal read on the
//! collection. The ledger is the one the declaration names beside it; a
//! missing one reads as empty.
//!
//! It prints the import's and the gate's counts as JSON, with the wall time
//! and rate of each, and the documents by the outcome of their latest
//! revision, with how many are eligible; and it checks that each manifest
//! line is counted once and that each revision has a disposition.

use super::dispositions::{Documents, Manifest, Run, markdown, per_second};
use maestro_kernel::{
    binding::Bindings,
    paths::{self, Environment},
    scope::{Config, LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    collection::Declaration,
    corpus::Entry,
    import,
    quality::{self, Ledger},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
    str,
    time::Instant,
};

/// The value of `variable`, without which the run cannot start.
fn required(variable: &str) -> String {
    env::var(variable)
        .unwrap_or_else(|_| panic!("set {variable}; see tests/it/local_collection/real_import.rs"))
}

/// What the manifests of `declaration`, found through `bindings`, say of
/// each of their lines.
fn manifest(declaration: &Declaration, bindings: &Bindings) -> Manifest {
    let mut manifest = Manifest::default();
    for (source, path) in declaration.manifest_paths(bindings).unwrap() {
        let mut reader = BufReader::new(File::open(path).unwrap());
        let (mut buffer, mut number) = (Vec::new(), 0);
        while reader.read_until(b'\n', &mut buffer).unwrap() > 0 {
            number += 1;
            let line = (source.id.clone(), number);
            let entry = str::from_utf8(&buffer)
                .ok()
                .and_then(|text| text.trim_end_matches('\n').parse::<Entry>().ok());
            if let Some(entry) = entry {
                let set = entry.set.unwrap_or_else(|| "-".to_owned());
                manifest
                    .kinds
                    .insert(line.clone(), (entry.source_kind, set));
                manifest
                    .by_source_ref
                    .entry(entry.source_ref)
                    .or_default()
                    .insert(line);
            }
            buffer.clear();
        }
        manifest.lines += number;
    }
    manifest
}

/// What the kernel holds of each document of `collection`, as `scopes` reads
/// it, once gated.
fn documents(database: &Database, scopes: &ScopeSet, collection: &str) -> Documents {
    let latest: BTreeMap<String, String> = database
        .revisions(scopes, collection)
        .unwrap()
        .into_iter()
        .map(|revision| (revision.document_id, revision.id))
        .collect();
    let mut documents = Documents::default();
    for revision in latest.into_values() {
        let outcome = database
            .disposition(scopes, &revision)
            .unwrap()
            .map_or_else(|| "none".to_owned(), |decided| decided.outcome.to_string());
        *documents.outcomes.entry(outcome).or_default() += 1;
        documents.latest.insert(revision);
    }
    let eligible = quality::eligible(database, scopes, collection).unwrap();
    documents.eligible = u64::try_from(eligible.len()).unwrap();
    documents
}

#[test]
#[ignore = "imports into this machine's kernel: MAESTRO_COLLECTION=<collection.json> \
            cargo test --release -p maestro-knowledge --test it local_collection -- \
            --ignored --nocapture"]
fn imports_and_gates_the_collection_this_machine_names() {
    let path = required("MAESTRO_COLLECTION");
    let declaration: Declaration = fs::read_to_string(&path).unwrap().parse().unwrap();
    let beside = Path::new(&path).parent().unwrap();
    let ledger = Ledger::load(&declaration.quality.ledger.under(beside)).unwrap();
    let environment = Environment::current();
    let configuration = paths::config_dir(&environment).unwrap();
    let bindings = Bindings::load(&configuration).unwrap();
    let access = Config::load(&configuration).unwrap();
    let database = Database::open_in(&paths::data_dir(&environment).unwrap()).unwrap();
    database.apply_config(&access).unwrap();
    let scopes = database.visible(LOCAL).unwrap();
    let manifest = manifest(&declaration, &bindings);
    let lines = manifest.lines;

    let started = Instant::now();
    let imported = import::import(&database, &scopes, &declaration, &bindings).unwrap();
    let importing = started.elapsed();
    let counted = imported.imported + imported.unchanged + imported.held + imported.refused;
    assert_eq!(counted, lines, "each manifest line is counted once");

    let started = Instant::now();
    let gated = quality::gate(&database, &scopes, &declaration.id, &ledger).unwrap();
    let gating = started.elapsed();
    let outcomes = gated.outcomes;
    let decided = outcomes.accepted
        + outcomes.accepted_with_warnings
        + outcomes.needs_reextraction
        + outcomes.quarantined
        + outcomes.excluded;
    assert_eq!(decided, gated.revisions, "each revision has a disposition");
    assert_eq!(gated.decided + gated.kept, gated.revisions);

    let documents = documents(&database, &scopes, &declaration.id);
    let run = Run {
        manifest: &manifest,
        documents: &documents,
        import: &imported,
        gate: &gated,
        importing,
        gating,
    };
    println!(
        "{:#}",
        json!({
            "manifest_lines": lines,
            "import": {
                "imported": imported.imported,
                "unchanged": imported.unchanged,
                "held": imported.held,
                "refused": imported.refused,
                "seconds": importing.as_secs_f64(),
                "lines_per_second": per_second(lines, importing),
            },
            "gate": {
                "revisions": gated.revisions,
                "decided": gated.decided,
                "kept": gated.kept,
                "outcomes": gated.outcomes,
                "rules": gated.rules,
                "held": gated.held.len(),
                "seconds": gating.as_secs_f64(),
                "revisions_per_second": per_second(gated.revisions, gating),
            },
            "documents": {
                "count": documents.latest.len(),
                "latest_outcomes": documents.outcomes,
                "eligible": documents.eligible,
            },
        })
    );
    if let Some(report) = env::var_os("MAESTRO_DISPOSITION_REPORT") {
        fs::write(report, markdown(&declaration.id, &run)).unwrap();
    }
}
