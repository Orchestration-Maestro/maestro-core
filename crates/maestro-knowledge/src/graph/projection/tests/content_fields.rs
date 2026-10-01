//! Each full-record field must affect durable projection verification.

use super::super::content::{digest, encode_fact, tests::fact};
use crate::graph::projection::EntityFact;
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{EntityKind, ReviewState, Support, Validity},
};
use std::{collections::BTreeMap, fmt::Write, slice};

/// A full record, including bounded qualifiers and two ordered support references.
fn complete_fact() -> EntityFact {
    let mut record = fact();
    record.claim.claim.conditions = BTreeMap::from([("mode".to_owned(), "safe".to_owned())]);
    record.claim.claim.version = Validity::Bounded {
        start: Some("1".to_owned()),
        end: Some("2".to_owned()),
    };
    record.claim.claim.world = Validity::Bounded {
        start: Some("2026-01-01".to_owned()),
        end: Some("2027-01-01".to_owned()),
    };
    record.claim.claim.supports = vec![
        Support {
            revision_id: "revision-a".to_owned(),
            block_id: "block-a".to_owned(),
            span: Span { start: 3, end: 7 },
            quote_digest: Digest::of(b"fast"),
        },
        Support {
            revision_id: "revision-b".to_owned(),
            block_id: "block-b".to_owned(),
            span: Span { start: 9, end: 13 },
            quote_digest: Digest::of(b"mode"),
        },
    ];
    record
}

/// Compare records differing only in the field changed by this case.
fn assert_field_changes_digest(change: fn(&mut EntityFact)) {
    let baseline = complete_fact();
    let mut changed = baseline.clone();
    change(&mut changed);
    assert_ne!(baseline, changed, "the case must actually alter the record");
    assert_ne!(
        digest(&[], slice::from_ref(&baseline)).unwrap(),
        digest(&[], slice::from_ref(&changed)).unwrap(),
        "loss or corruption of this fact field must invalidate verification"
    );
}

#[test]
fn full_record_and_empty_content_have_frozen_v2_digests() {
    assert_eq!(
        digest(&[], &[complete_fact()]).unwrap().as_str(),
        "1e0732d8e12575d4ac9590ce6e6a6cb51dae8d0ac2e0fc9e5d7336cf697d62ca"
    );
    assert_eq!(
        digest(&[], &[]).unwrap().as_str(),
        "a3b2abe4237b84ac08917ce65e45f14ec2b9df6cd0426c18d27225c53e742a35"
    );
}

#[test]
fn digest_covers_claim_collection() {
    assert_field_changes_digest(|fact| fact.claim.collection_id = "other".to_owned());
}

#[test]
fn digest_covers_subject_kind() {
    assert_field_changes_digest(|fact| fact.claim.claim.subject.kind = EntityKind::Component);
}

#[test]
fn digest_covers_subject_name() {
    assert_field_changes_digest(|fact| fact.claim.claim.subject.name = "other".to_owned());
}

#[test]
fn digest_covers_condition_keys() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.conditions = BTreeMap::from([("other".to_owned(), "safe".to_owned())]);
    });
}

#[test]
fn digest_covers_condition_values() {
    assert_field_changes_digest(|fact| {
        fact.claim
            .claim
            .conditions
            .insert("mode".to_owned(), "other".to_owned());
    });
}

#[test]
fn digest_covers_condition_presence() {
    assert_field_changes_digest(|fact| fact.claim.claim.conditions.clear());
}

#[test]
fn digest_covers_version_kind() {
    assert_field_changes_digest(|fact| fact.claim.claim.version = Validity::Unknown);
}

#[test]
fn digest_covers_version_start() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.version = Validity::Bounded {
            start: Some("other".to_owned()),
            end: Some("2".to_owned()),
        };
    });
}

#[test]
fn digest_covers_version_end() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.version = Validity::Bounded {
            start: Some("1".to_owned()),
            end: Some("other".to_owned()),
        };
    });
}

#[test]
fn digest_covers_world_kind() {
    assert_field_changes_digest(|fact| fact.claim.claim.world = Validity::Unknown);
}

#[test]
fn digest_covers_world_start() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.world = Validity::Bounded {
            start: Some("other".to_owned()),
            end: Some("2027-01-01".to_owned()),
        };
    });
}

#[test]
fn digest_covers_world_end() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.world = Validity::Bounded {
            start: Some("2026-01-01".to_owned()),
            end: Some("other".to_owned()),
        };
    });
}

#[test]
fn digest_covers_extractor() {
    assert_field_changes_digest(|fact| fact.claim.claim.provenance.extractor = "other".to_owned());
}

#[test]
fn digest_covers_profile() {
    assert_field_changes_digest(|fact| fact.claim.claim.provenance.profile = Digest::of(b"other"));
}

#[test]
fn digest_covers_support_revision() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.supports[0].revision_id = "other".to_owned();
    });
}

#[test]
fn digest_covers_support_block() {
    assert_field_changes_digest(|fact| fact.claim.claim.supports[0].block_id = "other".to_owned());
}

#[test]
fn digest_covers_support_start() {
    assert_field_changes_digest(|fact| fact.claim.claim.supports[0].span.start = 2);
}

#[test]
fn digest_covers_support_end() {
    assert_field_changes_digest(|fact| fact.claim.claim.supports[0].span.end = 8);
}

#[test]
fn digest_covers_support_quote() {
    assert_field_changes_digest(|fact| {
        fact.claim.claim.supports[0].quote_digest = Digest::of(b"other");
    });
}

#[test]
fn digest_covers_support_presence() {
    assert_field_changes_digest(|fact| fact.claim.claim.supports.clear());
}

#[test]
fn digest_covers_support_order() {
    assert_field_changes_digest(|fact| fact.claim.claim.supports.reverse());
}

#[test]
fn digest_covers_review_state() {
    for review in [
        ReviewState::Accepted,
        ReviewState::Rejected,
        ReviewState::Flagged,
    ] {
        let baseline = complete_fact();
        let mut changed = baseline.clone();
        changed.claim.review = review;
        assert_ne!(
            digest(&[], &[baseline]).unwrap(),
            digest(&[], &[changed]).unwrap()
        );
    }
}

#[test]
fn digest_covers_recorded_at() {
    assert_field_changes_digest(|fact| fact.claim.recorded_at = "2027-01-01T00:00:00Z".to_owned());
}

#[test]
fn digest_distinguishes_unknown_open_and_empty_validity_bounds() {
    let baseline = complete_fact();
    for world in [false, true] {
        let mut seen = Vec::new();
        for validity in [
            Validity::Unknown,
            Validity::Bounded {
                start: None,
                end: None,
            },
            Validity::Bounded {
                start: Some(String::new()),
                end: None,
            },
            Validity::Bounded {
                start: None,
                end: Some(String::new()),
            },
            Validity::Bounded {
                start: Some(String::new()),
                end: Some(String::new()),
            },
        ] {
            let mut changed = baseline.clone();
            if world {
                changed.claim.claim.world = validity;
            } else {
                changed.claim.claim.version = validity;
            }
            let found = digest(&[], &[changed]).unwrap();
            assert!(
                !seen.contains(&found),
                "validity tags and optional bounds must be distinct"
            );
            seen.push(found);
        }
    }
}

#[test]
fn full_fact_encoding_is_frozen() {
    let encoded = encode_fact(&complete_fact()).unwrap();
    let mut hex = String::new();
    for byte in encoded {
        write!(hex, "{byte:02x}").unwrap();
    }
    assert_eq!(
        hex,
        concat!(
            "460000004064643162336333313263663764383136313330333534343532653936323963",
            "653339333535623063353334313239646432366130386364396134353032656465000000",
            "406139343931663463316266376230636666626164636261326462386630323865346233",
            "66323836376362353965316633613062633139363866336335313234320000000b444546",
            "41554c54535f544f00000004746578740000000466617374000000016300000001310000",
            "00016300000009506172616d65746572000000046d6f646500000001000000046d6f6465",
            "00000004736166650101000000013101000000013201010000000a323032362d30312d30",
            "31010000000a323032372d30312d30310000000474657374000000403139303065616236",
            "633032383438336437313236353939656536663530646530643237393037623563363566",
            "6139303532343538306234623066393835326230000000020000000a7265766973696f6e",
            "2d6100000007626c6f636b2d610000000133000000013700000040313135646333363036",
            "666266383639316662363966326165666563383666326563643330323336326130353032",
            "623361393634386266326334646338323930660000000a7265766973696f6e2d62000000",
            "07626c6f636b2d6200000001390000000231330000004065363432623132393031613665",
            "653531343536663635346334386364306161366539306166643634653033356166626339",
            "3763666335343232303963373066390000000a756e726576696577656400000014323032",
            "362d30312d30315430303a30303a30305a",
        )
    );
}
