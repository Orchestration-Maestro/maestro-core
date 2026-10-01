//! Deterministic text construction from a frozen source view.

use super::types::{Descriptor, DescriptorError, DescriptorPin, SourcePointer};
use crate::graph::{
    resolve::resolve_snapshot,
    verify::{Source, check_source},
};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        ClaimRecord, Endpoint, EntityName, Mention, Object, ResolutionSnapshot, ReviewState,
        Support, Validity,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Read-only authority/artifact view; production callers obtain it from `read`.
#[derive(Debug)]
pub struct DescriptorInput {
    /// The generation and source-version selection.
    pub(super) pin: DescriptorPin,
    /// Attached kernel claim-set identity.
    pub(super) claim_set: Digest,
    /// Authorized identity-resolution snapshot.
    pub(super) snapshot: ResolutionSnapshot,
    /// Claims belonging to the attached set, not other alias source sets.
    pub(super) claims: Vec<ClaimRecord>,
    /// Original source artifacts of pinned revisions.
    pub(super) sources: BTreeMap<String, Source>,
}

/// Compose entity and claim documents without accepting prose from a caller.
///
/// # Errors
/// Refuses unsourced endpoint context or inconsistent frozen inputs.
pub fn build(
    input: &DescriptorInput,
    contexts: &BTreeMap<Mention, Support>,
) -> Result<Vec<Descriptor>, DescriptorError> {
    let entities =
        resolve_snapshot(&input.snapshot).map_err(|_| refused("invalid identity snapshot"))?;
    let mut documents = BTreeMap::new();
    for record in &input.claims {
        let claim = &record.claim;
        let mut texts = Vec::new();
        let mut pointers = Vec::new();
        let mut endpoints = vec![(Endpoint::Subject, &claim.subject)];
        if let Object::Entity(name) = &claim.object {
            endpoints.push((Endpoint::Object, name));
        }
        for (endpoint, name) in endpoints {
            let mention = Mention {
                claim: record.id.clone(),
                endpoint,
            };
            let support = contexts
                .get(&mention)
                .ok_or_else(|| refused("missing endpoint context"))?;
            let text = context(input, record, support, name)?;
            let target = entities
                .iter()
                .find(|entity| entity.mentions.contains(&mention))
                .ok_or_else(|| refused("unresolved endpoint"))?;
            let pointer = SourcePointer {
                revision_id: support.revision_id.clone(),
                block_id: support.block_id.clone(),
                span: support.span,
                quote_digest: support.quote_digest.clone(),
            };
            let entity = descriptor(
                input,
                record,
                (target.id.clone(), "entity"),
                text.clone(),
                vec![pointer.clone()],
            );
            documents.insert(entity.id.clone(), entity);
            texts.push(text);
            pointers.push(pointer);
        }
        let object = match &claim.object {
            Object::Entity(_) => texts
                .get(1)
                .cloned()
                .ok_or_else(|| refused("missing object context"))?,
            Object::Literal(literal) => format!("{}\n{}", literal.kind.as_str(), literal.lexeme),
        };
        let text = texts
            .into_iter()
            .take(1)
            .chain([claim.predicate.as_str().to_owned(), object])
            .collect::<Vec<_>>()
            .join("\n");
        let document = descriptor(input, record, (record.id.clone(), "claim"), text, pointers);
        documents.insert(document.id.clone(), document);
    }
    Ok(documents.into_values().collect())
}

/// Locate a context only inside the original verified support, never across gaps.
fn context(
    input: &DescriptorInput,
    record: &ClaimRecord,
    support: &Support,
    name: &EntityName,
) -> Result<String, DescriptorError> {
    if !record.claim.supports.iter().any(|outer| {
        outer.revision_id == support.revision_id
            && outer.block_id == support.block_id
            && outer.span.start <= support.span.start
            && support.span.end <= outer.span.end
    }) {
        return Err(refused("context is outside frozen supports"));
    }
    let source = input
        .sources
        .get(&support.revision_id)
        .ok_or_else(|| refused("source is outside generation"))?;
    check_source(source, &Digest::of(source.markdown().as_bytes()))
        .map_err(|_| refused("inconsistent canonical source"))?;
    let block = source
        .canonical()
        .blocks
        .iter()
        .find(|block| block.block_id == support.block_id)
        .ok_or_else(|| refused("unknown canonical block"))?;
    if block.revision_id != support.revision_id
        || !block
            .source_spans
            .iter()
            .any(|span| span.start <= support.span.start && support.span.end <= span.end)
    {
        return Err(refused("context is outside canonical block"));
    }
    let text = source
        .markdown()
        .get(support.span.start..support.span.end)
        .ok_or_else(|| refused("invalid UTF-8 span"))?;
    if Digest::of(text.as_bytes()) != support.quote_digest {
        return Err(refused("unverified defining text"));
    }
    if !text.contains(&name.name) {
        return Err(refused("context does not name its endpoint"));
    }
    Ok(format!("{}\n{}\n{text}", name.name, name.kind.as_str()))
}

/// Qualifier representation preserves unknown and half-open source bounds.
fn validity(value: &Validity) -> Value {
    match value {
        Validity::Unknown => json!({"known": false}),
        Validity::Bounded { start, end } => json!({"known": true, "start": start, "end": end}),
    }
}

/// Derive a content ID without record time, supporting-copy votes or ANN state.
fn descriptor(
    input: &DescriptorInput,
    record: &ClaimRecord,
    target: (Digest, &str),
    text: String,
    pointers: Vec<SourcePointer>,
) -> Descriptor {
    let (target, kind) = target;
    let qualifiers = json!({
        "conditions": record.claim.conditions,
        "version": validity(&record.claim.version),
        "world": validity(&record.claim.world),
    });
    let eligible = record.review != ReviewState::Rejected && record.review != ReviewState::Flagged;
    let mut document = Descriptor {
        id: Digest::of(b""),
        target,
        kind: kind.into(),
        text,
        pointers,
        pin: input.pin.clone(),
        claim_set: input.claim_set.clone(),
        resolution: input.snapshot.id.clone(),
        qualifiers,
        eligible,
    };
    document.id = document.identity();
    document
}

/// A sanitized refusal; private source text never enters diagnostics.
pub(super) fn refused(message: &str) -> DescriptorError {
    DescriptorError(message.into())
}
