//! Frozen application-ID encodings for projection content and receipt names.
#![allow(
    dead_code,
    reason = "the future engine adapter consumes this shared byte contract"
)]

use super::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope};
use maestro_kernel::{artifact::Digest, facts::Object};

/// Version domain separating canonical projection content from other hashes.
const CONTENT_VERSION: &[u8] = b"maestro-projection-content/1";
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
        let (kind, lexeme) = match &fact.claim.claim.object {
            Object::Literal(literal) => (literal.kind.as_str(), literal.lexeme.as_str()),
            Object::Entity(_) => return Err("entity object in literal fact rows".to_owned()),
        };
        rows.push(encode_row(
            b'F',
            &[
                fact.claim.id.as_str(),
                fact.subject.as_str(),
                fact.claim.claim.predicate.as_str(),
                kind,
                lexeme,
                &fact.scope.collection_id,
                &fact.scope.generation_id.to_string(),
            ],
        )?);
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
    let Some(digest) = name
        .strip_prefix('g')
        .and_then(|name| name.strip_suffix(".lbdb"))
    else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
mod tests {
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

    fn fact() -> EntityFact {
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
            "8dff3db5d2ca38349bd4c4c3432a6af9cf88e3d9091810b890db0f6d2215963e"
        );
        assert_eq!(
            digest(&[], slice::from_ref(&only_fact)).unwrap().as_str(),
            "08ad8aaed1176ad3a7cb464d7a802dfac5a925d8a7255ed47a0a4fd32046dc5d"
        );
        assert_eq!(
            digest(
                &[first.clone(), second.clone()],
                slice::from_ref(&only_fact)
            )
            .unwrap()
            .as_str(),
            "fafff101b76184cc449a7236da4afafa16a11361c30772c738853021c9591dbb"
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
