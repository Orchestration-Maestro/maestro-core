//! Exact parent provenance and acknowledged-child change-key binding.
use super::{
    Captures, Enumeration, Handle, Partitions, ReceiptError,
    partition_captures::bound,
    partition_mutation_support::{Fixture, batch, discovered, keys, now},
};
use crate::artifact::Digest;

#[test]
fn k1_parent_capture_independent_claims() {
    let fixture = Fixture::new();
    let parent = discovered(0);
    let (context, envelope) = fixture.capture(&parent.request);
    let handle = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    let mut valid = batch();
    valid.partition.kind = Enumeration::Links;
    valid.capture = Some(handle);
    valid.extractor = Some("links/1".into());
    let mut parent_keys = keys();
    parent_keys.links = Some(Digest::of(b"links"));
    parent_keys.representation = Some(envelope.artifact.clone());
    parent_keys.validator = Some(Digest::of(&serde_json::to_vec(&envelope.headers).unwrap()));
    parent_keys.metadata = Some(Digest::of(
        &serde_json::to_vec(&(&envelope.declared_media, &envelope.detected_media)).unwrap(),
    ));
    valid.parent_keys = Some(parent_keys.clone());
    valid.items = vec![discovered(1)];
    let mut cases = vec![];
    let mut changed = valid.clone();
    changed.capture = Some(Handle::new());
    cases.push(changed);
    for index in 0..5 {
        let mut changed = valid.clone();
        let claims = changed.parent_keys.as_mut().unwrap();
        match index {
            0 => claims.links = None,
            1 => claims.representation = Some(Digest::of(b"wrong")),
            2 => claims.validator = Some(Digest::of(b"wrong")),
            3 => claims.metadata = Some(Digest::of(b"wrong")),
            _ => claims.permissions = Digest::of(b"wrong"),
        }
        cases.push(changed);
    }
    for index in 0..6 {
        let mut changed = valid.clone();
        let child = &mut changed.items[0];
        match index {
            0 => {
                child.request.authorization_context = Digest::of(b"wrong");
                child.keys.permissions = child.request.authorization_context.clone();
            }
            1 => child.request.representation_profile = Digest::of(b"wrong"),
            2 => child.keys.representation = Some(Digest::of(b"claim")),
            3 => child.keys.validator = Some(Digest::of(b"claim")),
            4 => child.keys.metadata = Some(Digest::of(b"claim")),
            _ => child.keys.links = Some(Digest::of(b"claim")),
        }
        cases.push(changed);
    }
    for (index, changed) in cases.iter().enumerate() {
        assert!(
            matches!(
                fixture.db.checkpoint(&fixture.writer, changed, now()),
                Err(ReceiptError::Invalid)
            ),
            "parent case {index}"
        );
    }
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &valid, now())
            .is_ok()
    );
    assert_eq!(
        fixture
            .db
            .partition(&fixture.scope, valid.partition.id)
            .unwrap()
            .unwrap()
            .batches,
        vec![valid]
    );
}

#[test]
fn k1_link_extractor_limits() {
    let fixture = Fixture::new();
    let (context, envelope) = fixture.capture(&discovered(0).request);
    let handle = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    let mut valid = batch();
    valid.partition.kind = Enumeration::Links;
    valid.capture = Some(handle);
    let mut claims = keys();
    claims.links = Some(Digest::of(b"links"));
    claims.representation = Some(envelope.artifact.clone());
    claims.validator = Some(Digest::of(&serde_json::to_vec(&envelope.headers).unwrap()));
    claims.metadata = Some(Digest::of(
        &serde_json::to_vec(&(envelope.declared_media, envelope.detected_media)).unwrap(),
    ));
    valid.parent_keys = Some(claims);
    for extractor in [None, Some(String::new()), Some("a".repeat(513))] {
        valid.extractor = extractor;
        assert!(matches!(
            fixture.db.checkpoint(&fixture.writer, &valid, now()),
            Err(ReceiptError::Invalid)
        ));
    }
    valid.extractor = Some("a".repeat(512));
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &valid, now())
            .is_ok()
    );
}

#[test]
fn k1_bound_capture_exact_change_keys_and_page_ceiling() {
    let fixture = Fixture::new();
    let mut child = discovered(0);
    let (context, envelope) = fixture.capture(&child.request);
    let handle = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    fixture.db.acknowledge_capture(&context, handle).unwrap();
    child.keys.validator = Some(Digest::of(&serde_json::to_vec(&envelope.headers).unwrap()));
    child.keys.representation = Some(envelope.artifact.clone());
    assert_eq!(
        bound(&fixture.db, &fixture.scope, "docs", &[child.clone()]).unwrap(),
        vec![Some(handle)]
    );
    for index in 0..2 {
        let mut wrong = child.clone();
        if index == 0 {
            wrong.keys.validator = Some(Digest::of(b"wrong"));
        } else {
            wrong.keys.representation = Some(Digest::of(b"wrong"));
        }
        assert_eq!(
            bound(&fixture.db, &fixture.scope, "docs", &[wrong]).unwrap(),
            vec![None]
        );
    }
    assert!(
        bound(
            &fixture.db,
            &fixture.scope,
            "docs",
            &vec![child.clone(); 1000]
        )
        .is_ok_and(|handles| handles == vec![Some(handle); 1000])
    );
    assert!(matches!(
        bound(&fixture.db, &fixture.scope, "docs", &vec![child; 1001]),
        Err(ReceiptError::Invalid)
    ));
}
