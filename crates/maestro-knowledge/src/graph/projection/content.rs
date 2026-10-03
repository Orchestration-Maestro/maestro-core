//! Frozen application-ID encodings for projection content and receipt names.
#![allow(
    dead_code,
    reason = "the future engine adapter consumes this shared byte contract"
)]

use super::port::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope};
use maestro_filesystem::is_receipt_basename;
use maestro_kernel::{
    artifact::Digest,
    facts::{Object, ReviewState, Validity},
};

/// Version domain separating canonical projection content from other hashes.
const CONTENT_VERSION: &[u8] = b"maestro-projection-content/2";
/// Version domain included in every stable receipt-name identity.
const NAME_VERSION: &str = "maestro-projection-name/1";

/// Digest sorted tagged rows; malformed facts and oversized fields fail closed.
pub(super) fn digest(edges: &[ProjectionEdge], facts: &[EntityFact]) -> Result<Digest, String> {
    let mut rows = edges
        .iter()
        .map(|edge| {
            encode_row(
                b'E',
                &[
                    match edge.family {
                        EdgeFamily::KnowledgeClaim => "knowledge_claim",
                        EdgeFamily::CatalogDependency => "catalog_dependency",
                    },
                    &edge.relation,
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.id.as_str(),
                    &edge.scope.collection_id,
                    &edge.scope.generation_id.to_string(),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for fact in facts {
        rows.push(encode_fact(fact)?);
    }
    rows.sort();

    let mut bytes = CONTENT_VERSION.to_vec();
    bytes.extend_from_slice(
        &u64::try_from(rows.len())
            .map_err(|_| "projection row count exceeds u64".to_owned())?
            .to_be_bytes(),
    );
    for row in rows {
        bytes.extend(row);
    }
    Ok(Digest::of(&bytes))
}

/// Encode every field of a literal fact, including its complete kernel claim record.
pub(super) fn encode_fact(fact: &EntityFact) -> Result<Vec<u8>, String> {
    let claim = &fact.claim.claim;
    let (kind, lexeme) = match &claim.object {
        Object::Literal(literal) => (literal.kind.as_str(), literal.lexeme.as_str()),
        Object::Entity(_) => return Err("entity object in literal fact rows".to_owned()),
    };
    let mut row = encode_row(
        b'F',
        &[
            fact.claim.id.as_str(),
            fact.subject.as_str(),
            claim.predicate.as_str(),
            kind,
            lexeme,
            &fact.scope.collection_id,
            &fact.scope.generation_id.to_string(),
            &fact.claim.collection_id,
            claim.subject.kind.as_str(),
            &claim.subject.name,
        ],
    )?;
    row.extend(encode_count(claim.conditions.len())?);
    for (key, value) in &claim.conditions {
        row.extend(encode_fields(&[key, value])?);
    }
    row.extend(encode_validity(&claim.version)?);
    row.extend(encode_validity(&claim.world)?);
    row.extend(encode_fields(&[
        &claim.provenance.extractor,
        claim.provenance.profile.as_str(),
    ])?);
    row.extend(encode_count(claim.supports.len())?);
    for support in &claim.supports {
        row.extend(encode_fields(&[
            &support.revision_id,
            &support.block_id,
            &support.span.start.to_string(),
            &support.span.end.to_string(),
            support.quote_digest.as_str(),
        ])?);
    }
    row.extend(encode_fields(&[
        match fact.claim.review {
            ReviewState::Unreviewed => "unreviewed",
            ReviewState::Accepted => "accepted",
            ReviewState::Rejected => "rejected",
            ReviewState::Flagged => "flagged",
        },
        &fact.claim.recorded_at,
    ])?);
    Ok(row)
}

/// Encode an explicit unknown/bounded tag, then independently optional bounds.
fn encode_validity(validity: &Validity) -> Result<Vec<u8>, String> {
    match validity {
        Validity::Unknown => Ok(vec![0]),
        Validity::Bounded { start, end } => {
            let mut bytes = vec![1];
            bytes.extend(encode_bound(start.as_deref())?);
            bytes.extend(encode_bound(end.as_deref())?);
            Ok(bytes)
        }
    }
}

/// Preserve absent versus present-empty qualifier bounds without a sentinel string.
fn encode_bound(bound: Option<&str>) -> Result<Vec<u8>, String> {
    match bound {
        None => Ok(vec![0]),
        Some(value) => {
            let mut bytes = vec![1];
            bytes.extend(encode_fields(&[value])?);
            Ok(bytes)
        }
    }
}

/// Encode the checked size of a variable-length collection as u32 big-endian.
fn encode_count(count: usize) -> Result<[u8; 4], String> {
    u32::try_from(count)
        .map(u32::to_be_bytes)
        .map_err(|_| "projection collection exceeds u32 entries".to_owned())
}

/// Return the receipt basename for one collection generation and claim set.
pub(super) fn basename(scope: &ProjectionScope, claim_set_id: &Digest) -> Result<String, String> {
    let identity = encode_fields(&[
        NAME_VERSION,
        &scope.collection_id,
        &scope.generation_id.to_string(),
        claim_set_id.as_str(),
    ])?;
    Ok(format!("g{}.lbdb", Digest::of(&identity).as_str()))
}

/// Whether a receipt name is canonical and safe from companion-name aliasing.
pub(super) fn is_canonical_basename(name: &str) -> bool {
    is_receipt_basename(name)
}

/// Encode a record tag followed by UTF-8 fields with u32 big-endian byte lengths.
fn encode_row(tag: u8, fields: &[&str]) -> Result<Vec<u8>, String> {
    let mut row = vec![tag];
    row.extend(encode_fields(fields)?);
    Ok(row)
}

/// Encode UTF-8 fields with checked u32 big-endian byte lengths.
fn encode_fields(fields: &[&str]) -> Result<Vec<u8>, String> {
    let mut encoded = Vec::new();
    for field in fields {
        let bytes = field.as_bytes();
        encoded.extend_from_slice(
            &u32::try_from(bytes.len())
                .map_err(|_| "projection field exceeds u32 bytes".to_owned())?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(bytes);
    }
    Ok(encoded)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::graph::projection::{EdgeFamily, EntityFact, ProjectionEdge};
    use maestro_kernel::facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        Provenance, ReviewState, Validity,
    };
    use std::{collections::BTreeMap, slice};

    fn edge(collection: &str, generation: i64, relation: &str) -> ProjectionEdge {
        ProjectionEdge {
            id: Digest::of(b"edge-id"),
            scope: ProjectionScope {
                collection_id: collection.to_owned(),
                generation_id: generation,
            },
            family: EdgeFamily::KnowledgeClaim,
            source: Digest::of(b"source"),
            target: Digest::of(b"target"),
            relation: relation.to_owned(),
        }
    }

    pub(in crate::graph::projection) fn fact() -> EntityFact {
        let scope = ProjectionScope {
            collection_id: "c".to_owned(),
            generation_id: 1,
        };
        EntityFact {
            claim: ClaimRecord {
                id: Digest::of(b"claim"),
                collection_id: "c".to_owned(),
                claim: Claim {
                    subject: EntityName {
                        kind: EntityKind::Parameter,
                        name: "mode".to_owned(),
                    },
                    predicate: Predicate::DefaultsTo,
                    object: Object::Literal(Literal {
                        kind: LiteralKind::Text,
                        lexeme: "fast".to_owned(),
                    }),
                    conditions: BTreeMap::new(),
                    version: Validity::Unknown,
                    world: Validity::Unknown,
                    provenance: Provenance {
                        extractor: "test".to_owned(),
                        profile: Digest::of(b"profile"),
                    },
                    supports: Vec::new(),
                },
                review: ReviewState::Unreviewed,
                recorded_at: "2026-01-01T00:00:00Z".to_owned(),
            },
            subject: Digest::of(b"subject"),
            scope,
        }
    }

    #[test]
    fn basename_encoding_is_frozen_and_companion_safe() {
        let scope = ProjectionScope {
            collection_id: "c".to_owned(),
            generation_id: 7,
        };
        let name = basename(&scope, &Digest::of(b"set")).unwrap();
        assert_eq!(
            name,
            "g0a60f3beabe6af12fd9f8df8aec78c3003385202982f3bb76f3546155d7e949c.lbdb"
        );
        assert_eq!(name.len(), 70);
        let stem = name.strip_suffix(".lbdb").unwrap();
        assert!(!stem.contains('.'));
        let other = basename(
            &ProjectionScope {
                generation_id: 8,
                ..scope
            },
            &Digest::of(b"set"),
        )
        .unwrap();
        let other_stem = other.strip_suffix(".lbdb").unwrap();
        assert_eq!(stem.len(), other_stem.len());
        for invalid in [
            "projection.db",
            "g.short.lbdb",
            &format!("g{}.lbdb", "a".repeat(63)),
            &format!("g{}..lbdb", "a".repeat(63)),
            &format!("g{}.lbdb", "A".repeat(64)),
            &format!("{stem}.LBDB"),
        ] {
            assert!(!is_canonical_basename(invalid));
        }
        assert!(is_canonical_basename(&name));
        assert!(!other.starts_with(&format!("{stem}.")));
        assert!(!name.starts_with(&format!("{other_stem}.")));
    }

    #[test]
    fn content_digest_golden_vectors_and_sorting() {
        let first = edge("c", 1, "requires");
        let second = edge("c", 1, "provides");
        let only_fact = fact();
        assert_eq!(
            digest(slice::from_ref(&first), &[]).unwrap().as_str(),
            "e2f151d0d7f1cdeaa6e405e21e080ce13c5cf7a4621ee6ff38e5b32c28203121"
        );
        assert_eq!(
            digest(&[], slice::from_ref(&only_fact)).unwrap().as_str(),
            "ade1c37f3c52bdccb5fe3d585b56a9c53a0162088e38c88aed338f61894d9ea3"
        );
        assert_eq!(
            digest(
                &[first.clone(), second.clone()],
                slice::from_ref(&only_fact)
            )
            .unwrap()
            .as_str(),
            "eeabd74ffef0facf49293251cc46a02d7e8111a7d6e9a9742b2a351d04acd30d"
        );
        assert_eq!(
            digest(&[first.clone(), second.clone()], &[]).unwrap(),
            digest(&[second, first], &[]).unwrap()
        );
        assert_ne!(
            encode_row(b'E', &["same"]).unwrap(),
            encode_row(b'F', &["same"]).unwrap()
        );
    }

    #[test]
    fn edge_digest_changes_with_each_field() {
        let first = edge("c", 1, "requires");
        let mut changed_id = first.clone();
        changed_id.id = Digest::of(b"different-id");
        let mut changed_source = first.clone();
        changed_source.source = Digest::of(b"different-source");
        let mut changed_target = first.clone();
        changed_target.target = Digest::of(b"different-target");
        let mut changed_family = first.clone();
        changed_family.family = EdgeFamily::CatalogDependency;
        for changed in [
            edge("c", 1, "different"),
            edge("other", 1, "requires"),
            edge("c", 2, "requires"),
            changed_id,
            changed_source,
            changed_target,
            changed_family,
        ] {
            assert_ne!(
                digest(slice::from_ref(&first), &[]).unwrap(),
                digest(&[changed], &[]).unwrap()
            );
        }
    }

    #[test]
    fn fact_digest_changes_with_each_field() {
        let baseline = fact();
        let baseline_digest = digest(&[], slice::from_ref(&baseline)).unwrap();
        let mut changed_claim = baseline.clone();
        changed_claim.claim.id = Digest::of(b"other-claim");
        let mut changed_subject = baseline.clone();
        changed_subject.subject = Digest::of(b"other-subject");
        let mut changed_predicate = baseline.clone();
        changed_predicate.claim.claim.predicate = Predicate::Configures;
        let mut changed_kind = baseline.clone();
        if let Object::Literal(literal) = &mut changed_kind.claim.claim.object {
            literal.kind = LiteralKind::Boolean;
        }
        let mut changed_lexeme = baseline.clone();
        if let Object::Literal(literal) = &mut changed_lexeme.claim.claim.object {
            literal.lexeme = "slow".to_owned();
        }
        let mut changed_collection = baseline.clone();
        changed_collection.scope.collection_id = "another".to_owned();
        let mut changed_generation = baseline;
        changed_generation.scope.generation_id = 2;
        for changed in [
            changed_claim,
            changed_subject,
            changed_predicate,
            changed_kind,
            changed_lexeme,
            changed_collection,
            changed_generation,
        ] {
            assert_ne!(digest(&[], &[changed]).unwrap(), baseline_digest);
        }
    }
}
