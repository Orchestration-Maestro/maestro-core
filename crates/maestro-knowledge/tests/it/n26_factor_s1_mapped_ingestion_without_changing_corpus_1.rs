//! N26 mapped ingestion: immutable assets, preserved mappings and corpus/1 parity.
use super::n26_support::{
    ASSETS, CORPUS_REVISION, DOCUMENT_ID, Fixture, MARKDOWN, SOURCE_REF, line, prepared,
};
use maestro_canonicalization::{AssetStatus, SourceSpan};
use maestro_kernel::{artifact::Digest, document::Outcome};
use maestro_knowledge::import::{AssetRecord, Imported, Target, ingest_mapped};
use serde_json::{Value, json};

#[test]
fn n26_corpus_golden_ids_and_source_ref_bytes_hold() {
    let fixture = Fixture::new();
    assert_eq!(fixture.corpus(&[line()]).imported, 1);
    let revision = fixture.revisions().remove(0);
    assert_eq!(revision.document_id, DOCUMENT_ID);
    assert_eq!(revision.id, CORPUS_REVISION);
    let doc = fixture.canonical(&revision);
    assert!(!doc.input_metadata.extra.contains_key(ASSETS));
    assert!(doc.extractor_blocks.is_empty());
    assert!(doc.asset_inventory.is_empty());
    assert_eq!(doc.source_reference.as_deref(), Some(SOURCE_REF));
    assert_eq!(
        fixture
            .db
            .document(&fixture.scopes, DOCUMENT_ID)
            .unwrap()
            .unwrap()
            .source_ref
            .as_bytes(),
        SOURCE_REF.as_bytes()
    );
    assert_eq!(fixture.corpus(&[line()]).unchanged, 1);
}

#[test]
fn n26_mapped_pdf_retains_page_cell_asset_and_unknown_coordinates() {
    let fixture = Fixture::new();
    let input = fixture.input();
    let blocks = input.extractor_blocks.clone();
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    let doc = fixture.canonical(&revision);
    assert_eq!(doc.extractor_blocks, blocks);
    assert_eq!(doc.asset_inventory["diagram.png"], AssetStatus::Missing);
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.extractor_block_ids.contains(&"table-1".into()))
    );
    assert_eq!(doc.extractor_blocks[0].original_locations[0].page, Some(3));
    assert_eq!(
        doc.extractor_blocks[0].original_locations[0]
            .locator
            .as_ref()
            .unwrap()["cell"],
        "A2"
    );
    assert!(doc.extractor_blocks[1].original_locations.is_empty());
    assert_eq!(revision.original_digest, Digest::of(MARKDOWN.as_bytes()));
    assert!(!prepared(&doc).is_empty());
}

#[test]
fn n26_inventory_digest_and_bidirectional_link_resolve() {
    let fixture = Fixture::new();
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    let doc = fixture.canonical(&revision);
    let links = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].capture, fixture.evidence.capture);
    assert_eq!(links[0].fidelity, fixture.evidence.fidelity);
    assert_eq!(
        fixture
            .db
            .artifact(&links[0].inventory)
            .unwrap()
            .unwrap()
            .pins,
        1,
        "inventory is pinned with its link"
    );
    let inventory = fixture.db.get(&links[0].inventory).unwrap();
    assert_eq!(
        doc.input_metadata.extra[ASSETS],
        json!(Digest::of(&inventory).as_str())
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&inventory).unwrap(),
        json!([ASSETS, [["diagram.png", "missing", null, null]]])
    );
    assert_eq!(
        fixture
            .db
            .capture_revisions(&fixture.scopes, fixture.evidence.capture)
            .unwrap(),
        links
    );
}

#[test]
fn n26_asset_only_revisions_are_immutable_and_equal_inventory_is_unchanged() {
    let fixture = Fixture::new();
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let mut input = fixture.input();
    input.assets = vec![fixture.available(b"first image")];
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::New
    );
    let before = fixture.revisions();
    let old_links = fixture
        .db
        .revision_links(&fixture.scopes, &before[0].id)
        .unwrap();
    let old_inventory = fixture.db.get(&old_links[0].inventory).unwrap();
    let mut input = fixture.input();
    input.assets = vec![fixture.available(b"second image")];
    assert_eq!(
        ingest_mapped(&fixture.target(), input.clone()).unwrap(),
        Imported::New
    );
    input
        .operational_metadata
        .insert("attempt".into(), json!("another run"));
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::Unchanged
    );
    let revisions = fixture.revisions();
    assert_eq!(revisions.len(), 3);
    assert_eq!(revisions[..2], before);
    assert_ne!(revisions[0].id, revisions[1].id);
    assert_ne!(revisions[1].id, revisions[2].id);
    assert_eq!(
        prepared(&fixture.canonical(&revisions[0])),
        prepared(&fixture.canonical(&revisions[2]))
    );
    assert_eq!(
        fixture.canonical(&revisions[0]).asset_inventory["diagram.png"],
        AssetStatus::Missing
    );
    assert_eq!(
        fixture.canonical(&revisions[1]).asset_inventory["diagram.png"],
        AssetStatus::Available
    );
    assert_eq!(
        fixture.db.get(&old_links[0].inventory).unwrap(),
        old_inventory
    );
    assert_eq!(
        fixture
            .db
            .revision_links(&fixture.scopes, &revisions[0].id)
            .unwrap(),
        old_links
    );
    assert!(
        revisions
            .iter()
            .all(|revision| revision.original_digest == revisions[0].original_digest)
    );
}

#[test]
fn n26_inventory_order_and_identical_duplicates_do_not_churn() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.assets.push(AssetRecord {
        destination: "required.bin".into(),
        status: AssetStatus::Unchecked,
        digest: None,
        length: None,
    });
    assert_eq!(
        ingest_mapped(&fixture.target(), input.clone()).unwrap(),
        Imported::New
    );
    input.assets.reverse();
    input.assets.push(input.assets[0].clone());
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::Unchanged
    );
    assert_eq!(fixture.revisions().len(), 1);
}

#[test]
fn n26_forged_inventory_conflicting_destinations_and_invalid_assets_refuse() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.metadata.extra.insert(ASSETS.into(), json!("forged"));
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "reserved metadata collision"
    );
    let mut input = fixture.input();
    input.assets.push(fixture.available(b"conflicting"));
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "conflicting destinations"
    );
    let mut input = fixture.input();
    input.assets[0].status = AssetStatus::Available;
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "available asset needs verified bytes"
    );
    for length in [0, 999] {
        let mut input = fixture.input();
        input.assets = vec![fixture.available(b"image")];
        input.assets[0].length = Some(length);
        assert!(
            ingest_mapped(&fixture.target(), input).is_err(),
            "asset length mismatch"
        );
    }
    let mut input = fixture.input();
    input.assets = vec![AssetRecord {
        destination: "diagram.png".into(),
        status: AssetStatus::Available,
        digest: Some(Digest::of(b"not stored")),
        length: Some(10),
    }];
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "missing asset artifact"
    );
    assert!(fixture.revisions().is_empty());
}

#[test]
fn n26_markdown_integrity_and_bad_mappings_refuse_before_recording() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.digest = Digest::of(b"wrong");
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "Markdown digest mismatch"
    );
    let mut input = fixture.input();
    input.length += 1;
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "Markdown length mismatch"
    );
    let mut input = fixture.input();
    input.markdown = b"\xff";
    input.digest = Digest::of(input.markdown);
    input.length = 1;
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "not UTF-8"
    );
    let mut input = fixture.input();
    input.extractor_blocks[0].markdown_spans[0].end = MARKDOWN.len() + 1;
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "out-of-bounds mapping"
    );
    let mut input = fixture.input();
    input.extractor_blocks[0].original_locations[0].source_reference = Some("other.pdf".into());
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "substituted original source"
    );
    let mut input = fixture.input();
    input.extractor_blocks[0].original_locations[0].page = Some(0);
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "zero page"
    );
    let mut input = fixture.input();
    input.markdown = "# é\n".as_bytes();
    input.digest = Digest::of(input.markdown);
    input.length = input.markdown.len() as u64;
    input.extractor_blocks[0].markdown_spans = vec![SourceSpan { start: 3, end: 4 }];
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "mapping must not split UTF-8"
    );
    assert!(fixture.revisions().is_empty());
}

#[test]
fn n26_corpus_shared_source_conflicts_still_hold_and_native_source_conflicts_refuse() {
    let fixture = Fixture::new();
    let mut other = line();
    other["sha256"] = json!(Digest::of(b"other").as_str());
    let report = fixture.corpus(&[line(), other]);
    assert_eq!([report.imported, report.held, report.refused], [0, 1, 1]);
    let revision = fixture.revisions().remove(0);
    assert_eq!(revision.id, CORPUS_REVISION);
    assert_eq!(
        fixture
            .db
            .disposition(&fixture.scopes, &revision.id)
            .unwrap()
            .unwrap()
            .outcome,
        Outcome::Quarantined
    );
    let target = Target {
        source: "other",
        ..fixture.target()
    };
    assert!(ingest_mapped(&target, fixture.input()).is_err());
    assert_eq!(fixture.revisions().len(), 1);
}

#[test]
fn n26_corrupted_raw_capture_and_available_asset_bytes_refuse_even_on_replay() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.assets = vec![fixture.available(b"verified image")];
    let digest = input.assets[0].digest.clone().unwrap();
    assert_eq!(
        ingest_mapped(&fixture.target(), input.clone()).unwrap(),
        Imported::New
    );
    fixture.corrupt(&digest);
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "corrupted available bytes refuse unchanged replay"
    );
    let raw = Digest::of(b"%PDF-synthetic raw table and image");
    fixture.corrupt(&raw);
    assert!(
        ingest_mapped(&fixture.target(), fixture.input()).is_err(),
        "oversized corrupt raw capture refuses a new revision"
    );
    assert_eq!(fixture.revisions().len(), 1);
}

#[test]
fn n26_source_visibility_reference_and_empty_inventory_are_explicit() {
    let fixture = Fixture::new();
    let denied = fixture.db.visible("ungranted").unwrap();
    let target = Target {
        scopes: &denied,
        ..fixture.target()
    };
    assert!(ingest_mapped(&target, fixture.input()).is_err());
    let mut input = fixture.input();
    input.metadata.source_reference = Some("other".into());
    assert!(ingest_mapped(&fixture.target(), input).is_err());
    let mut input = fixture.input();
    input.assets.push(AssetRecord {
        destination: String::new(),
        status: AssetStatus::Unchecked,
        digest: None,
        length: None,
    });
    assert!(ingest_mapped(&fixture.target(), input).is_err());
    let mut input = fixture.input();
    input.assets.clear();
    input.markdown = b"# Manual\n";
    input.digest = Digest::of(input.markdown);
    input.length = input.markdown.len() as u64;
    input.extractor_blocks.clear();
    assert_eq!(
        ingest_mapped(&fixture.target(), input.clone()).unwrap(),
        Imported::New
    );
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::Unchanged
    );
    let revision = fixture.revisions().remove(0);
    let doc = fixture.canonical(&revision);
    assert_eq!(
        doc.input_metadata.extra[ASSETS],
        json!(Digest::of(br#"["maestro.native_assets/1",[]]"#).as_str())
    );
}

#[test]
fn n26_native_local_asset_references_need_inventory_but_remote_and_fragment_do_not() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.assets.clear();
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "missing local image inventory refuses"
    );
    for markdown in [
        "![remote](https://example.org/image.png)\n",
        "[anchor](#manual)\n",
        "[local](other.md)\n",
    ] {
        let mut input = fixture.input();
        input.markdown = markdown.as_bytes();
        input.digest = Digest::of(input.markdown);
        input.length = input.markdown.len() as u64;
        input.assets.clear();
        input.extractor_blocks.clear();
        let result = ingest_mapped(&fixture.target(), input);
        if markdown.contains("other.md") {
            assert!(result.is_err(), "missing local link inventory refuses");
        } else {
            assert_eq!(result.unwrap(), Imported::New);
        }
    }
}
