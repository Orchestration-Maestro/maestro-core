//! Strict native rows and the inverse of E05's complete binary fact contract.

use super::schema::{self, FACT_VERSION};
use crate::graph::projection::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope, content, schema::SCHEMA_VERSION,
    writer::BuildVerification,
};
use lbug::{Connection, LogicalType, Value};
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{
        Claim, ClaimRecord, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate,
        Provenance, ReviewState, Support, Validity,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::{self, FromStr},
};

/// Validated canonical application rows read from the native database.
pub(super) struct Rows {
    /// Edges ordered by application ID, independent of native IDs.
    pub(super) edges: Vec<ProjectionEdge>,
    /// Subject facts ordered by claim application ID.
    pub(super) facts: Vec<EntityFact>,
    /// Logical paths proven by the native catalog, never guessed.
    indexes: BTreeSet<String>,
}
impl Rows {
    /// Derive counts and digest only from the validated durable rows.
    pub(super) fn verification(&self) -> Result<BuildVerification, String> {
        let mut family_counts = BTreeMap::new();
        for edge in &self.edges {
            *family_counts.entry(edge.family).or_default() += 1;
        }
        Ok(BuildVerification {
            schema: SCHEMA_VERSION.into(),
            family_counts,
            fact_count: self.facts.len(),
            content_digest: content::digest(&self.edges, &self.facts)?,
            indexes: self.indexes.clone(),
        })
    }
}

/// Read real catalog evidence and every row; malformed and duplicate content refuses.
pub(super) fn read(connection: &Connection<'_>, scope: &ProjectionScope) -> Result<Rows, String> {
    let indexes = schema::verify(connection, scope)?;
    let mut edges = Vec::new();
    for row in connection
        .query(
            "MATCH (s:Entity)-[e:Edge]->(t:Entity)
            RETURN e.id, e.family, e.relation, s.id, t.id, e.collection, e.generation",
        )
        .map_err(|error| error.to_string())?
    {
        edges.push(edge_row(&row)?);
    }
    let mut facts = Vec::new();
    let mut entities = BTreeSet::new();
    for row in connection
        .query("MATCH (e:Entity) RETURN e.id, e.facts")
        .map_err(|error| error.to_string())?
    {
        let [Value::String(id), Value::List(LogicalType::Blob, values)] = row.as_slice() else {
            return Err("malformed native entity properties".into());
        };
        let subject = Digest::parse(id).map_err(|error| error.to_string())?;
        if !entities.insert(subject.clone()) {
            return Err("duplicate native entity ID".into());
        }
        for value in values {
            let Value::Blob(bytes) = value else {
                return Err("non-blob fact property".into());
            };
            let fact = decode_fact(bytes)?;
            if fact.subject != subject {
                return Err("fact property is attached to the wrong subject".into());
            }
            facts.push(fact);
        }
    }
    if validate(scope, &edges, &facts)? != entities {
        return Err("unreferenced native entity rows".into());
    }
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    facts.sort_by(|left, right| left.claim.id.cmp(&right.claim.id));
    Ok(Rows {
        edges,
        facts,
        indexes,
    })
}

/// Parse an exact typed relationship row, preserving its explicit family and scope.
fn edge_row(row: &[Value]) -> Result<ProjectionEdge, String> {
    let [
        Value::String(id),
        Value::String(family),
        Value::String(relation),
        Value::String(source),
        Value::String(target),
        Value::String(collection),
        Value::Int64(generation),
    ] = row
    else {
        return Err("malformed native edge row".into());
    };
    Ok(ProjectionEdge {
        id: Digest::parse(id).map_err(|error| error.to_string())?,
        family: match family.as_str() {
            "knowledge_claim" => EdgeFamily::KnowledgeClaim,
            "catalog_dependency" => EdgeFamily::CatalogDependency,
            _ => return Err("unknown native edge family".into()),
        },
        relation: relation.clone(),
        source: Digest::parse(source).map_err(|error| error.to_string())?,
        target: Digest::parse(target).map_err(|error| error.to_string())?,
        scope: ProjectionScope {
            collection_id: collection.clone(),
            generation_id: *generation,
        },
    })
}

/// Validate storage shape and global row IDs; catalog authority stays in `ProjectionWriter`.
pub(super) fn validate(
    scope: &ProjectionScope,
    edges: &[ProjectionEdge],
    facts: &[EntityFact],
) -> Result<BTreeSet<Digest>, String> {
    let mut ids = BTreeSet::new();
    let mut entities = BTreeSet::new();
    for edge in edges {
        if edge.scope != *scope || edge.relation.is_empty() || !ids.insert(edge.id.clone()) {
            return Err("invalid scope or duplicate edge ID".into());
        }
        if edge.family == EdgeFamily::KnowledgeClaim
            && !Predicate::parse(&edge.relation)
                .is_some_and(|predicate| predicate.is_claimable() && !predicate.takes_literal())
        {
            return Err("invalid knowledge-claim relation".into());
        }
        entities.insert(edge.source.clone());
        entities.insert(edge.target.clone());
    }
    for fact in facts {
        if fact.scope != *scope
            || fact.claim.collection_id != scope.collection_id
            || !ids.insert(fact.claim.id.clone())
        {
            return Err("invalid scope or duplicate fact ID".into());
        }
        encode_fact(fact)?;
        entities.insert(fact.subject.clone());
    }
    Ok(entities)
}

/// A versioned property contains precisely the shared E05 fact encoding.
pub(super) fn encode_fact(fact: &EntityFact) -> Result<Vec<u8>, String> {
    let claim = &fact.claim.claim;
    let Object::Literal(literal) = &claim.object else {
        return Err("entity object in literal fact".into());
    };
    if !claim.predicate.takes_literal() || !literal.kind.admits(&literal.lexeme) {
        return Err("malformed literal fact".into());
    }
    let mut supports = BTreeSet::new();
    for support in &claim.supports {
        if support.span.start >= support.span.end
            || !supports.insert((
                &support.revision_id,
                &support.block_id,
                support.span.start,
                support.span.end,
                &support.quote_digest,
            ))
        {
            return Err("malformed or duplicate fact support".into());
        }
    }
    let mut bytes = FACT_VERSION.to_vec();
    bytes.extend(content::encode_fact(fact)?);
    Ok(bytes)
}

/// Decode canonical bytes only: unknown versions, duplicates and trailing bytes refuse.
pub(super) fn decode_fact(bytes: &[u8]) -> Result<EntityFact, String> {
    let mut input = Input(
        bytes
            .strip_prefix(FACT_VERSION)
            .and_then(|encoded| encoded.strip_prefix(b"F"))
            .ok_or("unknown fact encoding version or tag")?,
    );
    let id = input.digest()?;
    let subject = input.digest()?;
    let predicate = Predicate::parse(&input.string()?).ok_or("unknown fact predicate")?;
    let kind = LiteralKind::parse(&input.string()?).ok_or("unknown literal kind")?;
    let lexeme = input.string()?;
    let collection_id = input.string()?;
    let generation_id = input.number()?;
    let claim_collection = input.string()?;
    let subject_kind = EntityKind::parse(&input.string()?).ok_or("unknown subject kind")?;
    let subject_name = input.string()?;
    let mut conditions = BTreeMap::new();
    for _ in 0..input.count()? {
        if conditions
            .insert(input.string()?, input.string()?)
            .is_some()
        {
            return Err("duplicate fact condition".into());
        }
    }
    let version = input.validity()?;
    let world = input.validity()?;
    let provenance = Provenance {
        extractor: input.string()?,
        profile: input.digest()?,
    };
    let mut supports = Vec::new();
    for _ in 0..input.count()? {
        supports.push(Support {
            revision_id: input.string()?,
            block_id: input.string()?,
            span: Span {
                start: input.number()?,
                end: input.number()?,
            },
            quote_digest: input.digest()?,
        });
    }
    let review = match input.string()?.as_str() {
        "unreviewed" => ReviewState::Unreviewed,
        "accepted" => ReviewState::Accepted,
        "rejected" => ReviewState::Rejected,
        "flagged" => ReviewState::Flagged,
        _ => return Err("unknown fact review state".into()),
    };
    let recorded_at = input.string()?;
    let fact = EntityFact {
        subject,
        scope: ProjectionScope {
            collection_id,
            generation_id,
        },
        claim: ClaimRecord {
            id,
            collection_id: claim_collection,
            claim: Claim {
                subject: EntityName {
                    kind: subject_kind,
                    name: subject_name,
                },
                predicate,
                object: Object::Literal(Literal { kind, lexeme }),
                conditions,
                version,
                world,
                provenance,
                supports,
            },
            review,
            recorded_at,
        },
    };
    if !input.0.is_empty() || encode_fact(&fact)? != bytes {
        return Err("noncanonical fact encoding".into());
    }
    Ok(fact)
}

/// Checked cursor into the frozen length-delimited encoding; no untrusted preallocation.
struct Input<'a>(&'a [u8]);
impl Input<'_> {
    /// Consume exactly n bytes, refusing truncation.
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let (head, tail) = self.0.split_at_checked(n).ok_or("truncated fact field")?;
        self.0 = tail;
        Ok(head)
    }
    /// Decode one big-endian length or collection count.
    fn count(&mut self) -> Result<usize, String> {
        let bytes = self
            .take(4)?
            .try_into()
            .map_err(|_| "invalid fact length")?;
        usize::try_from(u32::from_be_bytes(bytes)).map_err(|_| "fact length exceeds usize".into())
    }
    /// Read exact UTF-8 bytes with their u32 length.
    fn string(&mut self) -> Result<String, String> {
        let n = self.count()?;
        str::from_utf8(self.take(n)?)
            .map(str::to_owned)
            .map_err(|_| "non-UTF-8 fact field".into())
    }
    /// Read a canonical application digest.
    fn digest(&mut self) -> Result<Digest, String> {
        Digest::parse(&self.string()?).map_err(|error| error.to_string())
    }
    /// Read a number; the final encode comparison enforces canonical decimal spelling.
    fn number<T: FromStr>(&mut self) -> Result<T, String> {
        self.string()?
            .parse()
            .map_err(|_| "invalid fact number".into())
    }
    /// Read a one-byte tag without unchecked indexing.
    fn tag(&mut self) -> Result<u8, String> {
        self.take(1)?
            .first()
            .copied()
            .ok_or("missing fact tag".into())
    }
    /// Read a strict optional string, preserving present-empty versus absent.
    fn bound(&mut self) -> Result<Option<String>, String> {
        match self.tag()? {
            0 => Ok(None),
            1 => self.string().map(Some),
            _ => Err("unknown fact bound tag".into()),
        }
    }
    /// Read an explicit validity tag and its independently optional bounds.
    fn validity(&mut self) -> Result<Validity, String> {
        match self.tag()? {
            0 => Ok(Validity::Unknown),
            1 => Ok(Validity::Bounded {
                start: self.bound()?,
                end: self.bound()?,
            }),
            _ => Err("unknown fact validity tag".into()),
        }
    }
}
