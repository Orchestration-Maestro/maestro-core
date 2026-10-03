//! Declaring a collection (`declare`), as `maestro knowledge collection add`
//! does before any import: the collection and each of its sources recorded
//! as an import records them, and nothing for a caller who cannot read a
//! source.

use super::support::{Scratch, declaration_of, everything};
use maestro_kernel::scope::Right;
use maestro_knowledge::import::{self, Error};

#[test]
fn declaring_a_collection_records_it_and_its_sources_and_no_revision() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let declaration = declaration_of(
        "garden",
        &[("docs", "docs.jsonl"), ("notes", "notes.jsonl")],
    );
    import::declare(&database, &scopes, &declaration).unwrap();
    let collection = database.collection(&scopes, "garden").unwrap().unwrap();
    assert_eq!(
        (collection.title.as_str(), collection.visibility.as_str()),
        ("The garden collection", "public")
    );
    for source in ["docs", "notes"] {
        let recorded = database.source(&scopes, "garden", source).unwrap().unwrap();
        assert_eq!(recorded.reference, format!("corpus_root:{source}.jsonl"));
    }
    assert_eq!(database.eligible_revisions(&scopes, "garden").unwrap(), []);
}

#[test]
fn a_collection_whose_source_the_caller_cannot_read_is_not_declared() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let docs = "workspace/default/collection/garden/source/docs"
        .parse()
        .unwrap();
    database
        .grant("docs-reader", &docs, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("docs-reader").unwrap();
    let declaration = declaration_of(
        "garden",
        &[("docs", "docs.jsonl"), ("notes", "notes.jsonl")],
    );
    let refusal = import::declare(&database, &scopes, &declaration).unwrap_err();
    assert!(
        matches!(&refusal, Error::NotVisible(scope)
            if scope == "workspace/default/collection/garden/source/notes"),
        "{refusal:?}"
    );
    assert!(
        database
            .collection(&everything(&database), "garden")
            .unwrap()
            .is_none()
    );
}

#[test]
fn n03_review_large_v1_collection_still_imports() {
    use super::support::{counts, import_declared, line, markdown, page};
    use maestro_knowledge::collection::Declaration;
    use serde_json::json;
    let scratch = Scratch::new();
    let database = scratch.database();
    let sources: Vec<_> = (0..128)
        .map(|index| (format!("source-{index}"), format!("manifest-{index}.jsonl")))
        .collect();
    let names: Vec<_> = sources
        .iter()
        .map(|(id, path)| (id.as_str(), path.as_str()))
        .collect();
    let mut value = serde_json::to_value(declaration_of("garden", &names)).unwrap();
    value["title"] = json!("Garden documentation ".repeat(500));
    let declared: Declaration = value.to_string().parse().unwrap();
    for (index, (_, manifest)) in sources.iter().enumerate() {
        let path = format!("notes/{index}.md");
        let bytes = markdown(&format!("Garden plant {index}"));
        scratch.put(&path, &bytes);
        scratch.manifest_at(manifest, &[line(&path, &bytes, &page(&index.to_string()))]);
    }
    let report = import_declared(&scratch, &database, &declared);
    assert_eq!(counts(&report), [128, 0, 0, 0]);
}
