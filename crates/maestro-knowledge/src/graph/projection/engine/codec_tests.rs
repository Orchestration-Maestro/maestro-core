//! Synthetic canonical fact-vector and malformed-byte checks.

use super::{rows, schema::FACT_VERSION, tests::full_fact};
use crate::graph::projection::content;
use maestro_kernel::facts::{Object, ReviewState, Validity};

/// Replace one of the ten leading length-delimited fields without using the decoder.
fn replace_field(bytes: &[u8], index: usize, value: &[u8]) -> Vec<u8> {
    let mut start = FACT_VERSION.len() + 1;
    for _ in 0..index {
        let count = u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap()) as usize;
        start += 4 + count;
    }
    let count = u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap()) as usize;
    let mut changed = bytes[..start].to_vec();
    changed.extend(u32::try_from(value.len()).unwrap().to_be_bytes());
    changed.extend(value);
    changed.extend(&bytes[start + 4 + count..]);
    changed
}

#[test]
fn fact_decoder_refuses_unknown_fields_noncanonical_numbers_and_duplicate_conditions() {
    let fact = full_fact();
    let bytes = rows::encode_fact(&fact).unwrap();
    for (index, value) in [
        (0, b"invalid".as_slice()),
        (1, b"INVALID"),
        (2, b"UNKNOWN"),
        (2, b"REQUIRES"),
        (3, b"float"),
        (6, b"01"),
        (6, b"+1"),
        (6, b"nan"),
        (8, b"parameter"),
        (9, b"\xff"),
    ] {
        assert!(
            rows::decode_fact(&replace_field(&bytes, index, value)).is_err(),
            "accepted {index}/{value:?}"
        );
    }
    let mut count_at = FACT_VERSION.len() + 1;
    for _ in 0..10 {
        let count = u32::from_be_bytes(bytes[count_at..count_at + 4].try_into().unwrap()) as usize;
        count_at += 4 + count;
    }
    let mut cursor = count_at + 4;
    let pair_start = cursor;
    for _ in 0..2 {
        let count = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4 + count;
    }
    let first_pair = bytes[pair_start..cursor].to_vec();
    let mut duplicate = bytes.clone();
    duplicate[count_at..count_at + 4].copy_from_slice(&3u32.to_be_bytes());
    duplicate.splice(cursor..cursor, first_pair);
    assert!(rows::decode_fact(&duplicate).is_err());
    let second_pair_start = cursor;
    for _ in 0..2 {
        let count = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4 + count;
    }
    let mut reversed = bytes[..pair_start].to_vec();
    reversed.extend(&bytes[second_pair_start..cursor]);
    reversed.extend(&bytes[pair_start..second_pair_start]);
    reversed.extend(&bytes[cursor..]);
    assert!(rows::decode_fact(&reversed).is_err());
    for position in [cursor, cursor + 1] {
        let mut unknown_tag = bytes.clone();
        unknown_tag[position] = 2;
        assert!(rows::decode_fact(&unknown_tag).is_err());
    }
    let mut unknown_review = bytes.clone();
    let review_at = bytes
        .windows(7)
        .position(|window| window == b"flagged")
        .unwrap();
    unknown_review[review_at..review_at + 7].copy_from_slice(b"unknown");
    assert!(rows::decode_fact(&unknown_review).is_err());
    let mut overlarge = bytes;
    overlarge[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(rows::decode_fact(&overlarge).is_err());
}

#[test]
fn every_validity_review_and_literal_lexeme_round_trips_without_normalization() {
    for review in [
        ReviewState::Unreviewed,
        ReviewState::Accepted,
        ReviewState::Rejected,
        ReviewState::Flagged,
    ] {
        for validity in [
            Validity::Unknown,
            Validity::Bounded {
                start: None,
                end: None,
            },
            Validity::Bounded {
                start: Some("1".into()),
                end: Some(String::new()),
            },
        ] {
            let mut fact = full_fact();
            fact.claim.review = review;
            fact.claim.claim.version = validity.clone();
            fact.claim.claim.world = validity;
            let bytes = rows::encode_fact(&fact).unwrap();
            assert_eq!(rows::decode_fact(&bytes).unwrap(), fact);
            assert_eq!(
                &bytes[FACT_VERSION.len()..],
                content::encode_fact(&fact).unwrap()
            );
        }
    }
    let mut fact = full_fact();
    if let Object::Literal(literal) = &mut fact.claim.claim.object {
        literal.lexeme = String::new();
    }
    assert_eq!(
        rows::decode_fact(&rows::encode_fact(&fact).unwrap()).unwrap(),
        fact
    );
}

#[test]
fn ordered_support_records_are_not_sorted_or_dropped_by_property_encoding() {
    let mut fact = full_fact();
    let mut second = fact.claim.claim.supports[0].clone();
    second.revision_id = "a-revision".into();
    second.span.start = 8;
    second.span.end = 12;
    fact.claim.claim.supports.push(second);
    let bytes = rows::encode_fact(&fact).unwrap();
    assert_eq!(rows::decode_fact(&bytes).unwrap(), fact);
    let forward = content::digest(&[], &[fact.clone()]).unwrap();
    fact.claim.claim.supports.reverse();
    assert_ne!(content::digest(&[], &[fact.clone()]).unwrap(), forward);
    assert_eq!(
        rows::decode_fact(&rows::encode_fact(&fact).unwrap()).unwrap(),
        fact
    );
}
