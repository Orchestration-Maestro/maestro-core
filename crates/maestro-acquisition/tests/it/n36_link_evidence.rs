//! Unknown link evidence is not a sentinel or proof that a child is unchanged.
use super::{
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{
        capture, link_fixture, partition, run,
    },
};
use maestro_acquisition::{
    discovery::{links::DomLinks, partition::discover},
    lifecycle::full::changed,
};
use maestro_kernel::{
    acquisition::{Batch, Captures, ChangeKeys, Frontier, Handle, Partitions, ReceiptError},
    artifact::Digest,
};

/// A real offline Links parent and its durable, unclaimed child signals.
fn links(fixture: &mut Fixture) -> Batch {
    let handle = capture(fixture, b"<a href='/docs/child'>reply</a>");
    run(discover(
        &fixture.db,
        &DomLinks::new().unwrap(),
        (&fixture.context, handle),
        &fixture.policy,
        (partition(), 0),
    ))
    .unwrap()
}

#[test]
fn n36_stale_links_parent_cannot_enqueue_after_refresh() {
    let mut fixture = link_fixture();
    let batch = links(&mut fixture);
    fixture
        .db
        .acknowledge_capture(&fixture.context, batch.capture.unwrap())
        .unwrap();
    fixture
        .db
        .refresh(
            &fixture.context.writer,
            fixture.context.item.item,
            fixture.context.now,
        )
        .unwrap();
    // Exact replay and historical evidence survive; new discovery must not.
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    let mut stale = batch.clone();
    stale.partition.id = Handle::new();
    stale.items.first_mut().unwrap().request.fetch_identity =
        "https://garden.example/docs/fresh".into();
    assert!(stale.items.first().unwrap().keys.links.is_none());
    let scopes = fixture.db.visible("reader").unwrap();
    let before = fixture.db.page(&scopes, "notes", None, 100).unwrap();
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &stale, fixture.context.now),
        Err(ReceiptError::Invalid),
        "stale generation submitted discovery"
    );
    assert_eq!(
        fixture.db.page(&scopes, "notes", None, 100).unwrap(),
        before
    );
    assert_eq!(
        fixture
            .db
            .partition(
                &"workspace/default/collection/garden".parse().unwrap(),
                batch.partition.id
            )
            .unwrap()
            .unwrap()
            .batches,
        vec![batch]
    );
}

#[test]
fn n36_links_parent_requires_known_link_inventory() {
    let mut fixture = link_fixture();
    let batch = links(&mut fixture);
    assert!(batch.parent_keys.as_ref().unwrap().links.is_some());
    let mut wrong = batch.clone();
    wrong.partition.id = Handle::new();
    wrong.parent_keys.as_mut().unwrap().links = None;
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &wrong, fixture.context.now),
        Err(ReceiptError::Invalid),
        "unknown parent links admitted"
    );
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
}

#[test]
fn n36_links_children_cannot_claim_a_link_inventory() {
    let mut fixture = link_fixture();
    let batch = links(&mut fixture);
    assert_eq!(
        batch.items.first().unwrap().keys.links,
        None,
        "child was assigned a sentinel or parent inventory"
    );
    let mut wrong = batch.clone();
    wrong.partition.id = Handle::new();
    wrong.items.first_mut().unwrap().keys.links = Some(Digest::of(b"unobserved child links"));
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &wrong, fixture.context.now),
        Err(ReceiptError::Invalid),
        "child link claim admitted"
    );
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
}

#[test]
fn n36_unknown_keys_are_not_evidence_of_unchanged_representations() {
    let fixture = Fixture::new();
    let known = super::n13_durably_enumerate_public_links_and_bounded_partitions::batch(&fixture)
        .items
        .remove(0)
        .keys;
    let verification = ChangeKeys {
        revision: None,
        validator: None,
        metadata: None,
        links: None,
        permissions: known.permissions.clone(),
        representation: known.representation.clone(),
    };
    assert!(
        !changed(&known, &verification),
        "unknown verification fields caused false change"
    );
    assert!(
        !changed(&verification, &known),
        "one-sided knowledge changed equal bytes"
    );
    let child = ChangeKeys {
        representation: None,
        ..verification.clone()
    };
    assert!(
        changed(&child, &known),
        "child unknown bytes called unchanged"
    );
    assert!(
        changed(&known, &child),
        "unknown current bytes called unchanged"
    );
    assert!(
        changed(&child, &child),
        "unknown child equality called unchanged"
    );
    let different = ChangeKeys {
        representation: Some(Digest::of(b"edited reply")),
        ..verification
    };
    assert!(changed(&known, &different));
}

#[test]
fn n36_known_link_digest_wire_bytes_stay_identical() {
    let fixture = Fixture::new();
    let mut keys =
        super::n13_durably_enumerate_public_links_and_bounded_partitions::batch(&fixture)
            .items
            .remove(0)
            .keys;
    let digest = keys.links.clone().unwrap();
    let before = serde_json::to_string(&keys).unwrap();
    let field = format!("\"links\":\"{}\"", digest.as_str());
    assert!(before.contains(&field), "known digest wire shape changed");
    keys.links = None;
    assert_eq!(
        serde_json::to_string(&keys).unwrap(),
        before.replace(&field, "\"links\":null"),
        "non-link wire bytes changed"
    );
}
