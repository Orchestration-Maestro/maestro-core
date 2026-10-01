//! Owner-approved offline HTML5 link reader and its recorded data contract.
use super::n13_durably_enumerate_public_links_and_bounded_partitions::{
    capture, link_fixture, partition, run, scope,
};
use maestro_acquisition::discovery::{
    links::{DomLinks, LINK_SELECTORS, LinkExtractor},
    partition::discover,
};
use maestro_kernel::{
    acquisition::{Captures, Partitions, SafeIdentity},
    artifact::Digest,
};
use std::fmt::Write as _;

#[test]
fn n13_dom_query_reads_hidden_anchors_images_and_every_declared_resource() {
    let reader = DomLinks::new().unwrap();
    let body = b"<!doctype html><html><head><base href='/docs/root/'>
        <base href='/ignored/'></head><body>
        <a aria-hidden='true' href='hidden'>same text</a><a href='A'>upper</a><a href='a'>lower</a>
        <area href='area'><link href='style.css'><script src='app.js'></script><img src='image.png'>
        <iframe src='frame'></iframe><source src='media'><embed src='embed'>
        <a href='hidden'>duplicate</a>
        </body></html>";
    let first = run(reader.extract("https://garden.example/docs/start", body)).unwrap();
    let replay = run(reader.extract("https://garden.example/docs/start", body)).unwrap();
    let expected = [
        "A",
        "a",
        "app.js",
        "area",
        "embed",
        "frame",
        "hidden",
        "image.png",
        "media",
        "style.css",
    ]
    .map(|path| format!("https://garden.example/docs/root/{path}"));
    assert_eq!(first.links, expected);
    assert_eq!(first, replay);
    assert!(!first.limit_hit);
    assert!(first.contract.contains("dom_query 0.28.0"));
    assert!(
        first
            .contract
            .contains(Digest::of(LINK_SELECTORS.as_bytes()).as_str())
    );
}
#[test]
fn n13_dom_query_checkpoint_replay_keeps_canonical_inventory() {
    let mut fixture = link_fixture();
    let reader = DomLinks::new().unwrap();
    let handle = capture(
        &mut fixture,
        b"<!doctype html><a href='/docs/z'>z</a><img src='/docs/a.png'><a
        aria-hidden='true' href='/docs/hidden'>same</a>",
    );
    let descriptor = partition();
    let first = run(discover(
        &fixture.db,
        &reader,
        (&fixture.context, handle),
        &fixture.policy,
        descriptor.clone(),
    ))
    .unwrap();
    let replay = run(discover(
        &fixture.db,
        &reader,
        (&fixture.context, handle),
        &fixture.policy,
        descriptor,
    ))
    .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.items.len(), 3);
    assert_eq!(
        fixture
            .db
            .partition(&scope(), first.partition.id)
            .unwrap()
            .unwrap()
            .batches,
        vec![first]
    );
}
#[test]
fn n13_dom_query_limit_and_invalid_url_do_not_prove_complete_coverage() {
    let reader = DomLinks::new().unwrap();
    let mut links = String::new();
    for index in 0..1001 {
        write!(&mut links, "<a href='/docs/{index}'>item</a>").unwrap();
    }
    let result =
        run(reader.extract("https://garden.example/docs/start", links.as_bytes())).unwrap();
    assert!(result.limit_hit);
    assert_eq!(result.links.len(), 1000);
    let malformed = run(reader.extract(
        "https://garden.example/docs/start",
        b"<base href='https://['><a href='page'>page</a>",
    ))
    .unwrap();
    assert!(malformed.limit_hit);
    let invalid_href = run(reader.extract(
        "https://garden.example/docs/start",
        b"<a href='https://['>bad</a>",
    ))
    .unwrap();
    assert!(invalid_href.limit_hit);
    assert!(run(reader.extract("https://garden.example/docs/start", &[255])).is_err());
}

#[test]
fn n13_redacted_query_self_links_and_explicit_html_base_resolve_exactly() {
    let reader = DomLinks::new().unwrap();
    for base in [None, Some(""), Some("#base")] {
        let mut fixture = link_fixture();
        fixture.envelope.final_identity =
            SafeIdentity::new("https://garden.example/docs/start?version=2").unwrap();
        let html_base = base.map_or_else(String::new, |href| format!("<base href='{href}'>"));
        let body = format!(
            "<!doctype html>{html_base}<a href='#a'>self</a><a href=''>self</a><a
            href='?x=1'>query</a><a href='other'>sibling</a>"
        );
        let handle = capture(&mut fixture, body.as_bytes());
        let (envelope, bytes) = fixture
            .db
            .read_capture(&fixture.context, handle, 10000)
            .unwrap();
        let encoded = serde_json::to_value(&envelope.final_identity).unwrap();
        assert!(!encoded.get("query_digest").unwrap().is_null());
        let output =
            run(reader.extract(envelope.final_identity.as_str(), &bytes.unwrap())).unwrap();
        assert_eq!(
            output.links,
            [
                "https://garden.example/docs/other",
                "https://garden.example/docs/start?x=1"
            ]
        );
    }
    let mut fixture = link_fixture();
    fixture.envelope.final_identity =
        SafeIdentity::new("https://garden.example/docs/start?version=2").unwrap();
    let handle = capture(
        &mut fixture,
        b"<!doctype html><base href='https://ex.test/doc?v=3'><a href='#a'>base</a><a
        href=''>base</a>",
    );
    let (envelope, bytes) = fixture
        .db
        .read_capture(&fixture.context, handle, 10000)
        .unwrap();
    let output = run(reader.extract(envelope.final_identity.as_str(), &bytes.unwrap())).unwrap();
    assert_eq!(output.links, ["https://ex.test/doc?v=3"]);
}
