//! Deterministic text construction from a frozen source view.

use super::{
    qualifiers::{DescriptorQualifiers, DescriptorValidity},
    types::{Descriptor, DescriptorError, DescriptorPin, SourcePointer},
};
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
use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
};

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
/// Context candidates are full verified claim supports, never caller subspans.
/// Select the first defining support in lexicographic order of
/// `(revision_id, block_id, span.start, span.end, quote_digest)`.
/// Defining text must contain the exact endpoint name at word boundaries and
/// more than the name after trimming; no case folding or expansion is applied.
///
/// # Errors
/// Refuses unsourced endpoint context or inconsistent frozen inputs.
pub fn build(input: &DescriptorInput) -> Result<Vec<Descriptor>, DescriptorError> {
    let entities =
        resolve_snapshot(&input.snapshot).map_err(|_| refused("invalid identity snapshot"))?;
    let attached: BTreeSet<_> = input
        .claims
        .iter()
        .flat_map(|record| {
            let subject = Mention {
                claim: record.id.clone(),
                endpoint: Endpoint::Subject,
            };
            let object = matches!(record.claim.object, Object::Entity(_)).then(|| Mention {
                claim: record.id.clone(),
                endpoint: Endpoint::Object,
            });
            once(subject).chain(object)
        })
        .collect();
    if let Some(held) = entities.iter().find(|entity| {
        !entity.colliding.is_empty()
            && entity
                .mentions
                .iter()
                .any(|mention| attached.contains(mention))
    }) {
        return Err(DescriptorError::HeldForReview(held.id.clone()));
    }
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
            let (support, text) = select_context(input, record, name)?;
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

/// Select only full frozen supports, independent of claim support ordering.
fn select_context<'a>(
    input: &DescriptorInput,
    record: &'a ClaimRecord,
    name: &EntityName,
) -> Result<(&'a Support, String), DescriptorError> {
    let mut supports: Vec<_> = record.claim.supports.iter().collect();
    supports.sort_by_key(|support| {
        (
            &support.revision_id,
            &support.block_id,
            support.span.start,
            support.span.end,
            &support.quote_digest,
        )
    });
    for support in supports {
        if let Some(text) = context(input, support, name)? {
            return Ok((support, text));
        }
    }
    Err(refused("missing endpoint context"))
}

/// Verify original support bytes before considering them as defining context.
fn context(
    input: &DescriptorInput,
    support: &Support,
    name: &EntityName,
) -> Result<Option<String>, DescriptorError> {
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
    if text.trim() == name.name || !contains_name(text, &name.name) {
        return Ok(None);
    }
    Ok(Some(format!(
        "{}\n{}\n{text}",
        name.name,
        name.kind.as_str()
    )))
}

/// Exact Unicode word boundaries include letters, numbers and underscore.
fn contains_name(text: &str, name: &str) -> bool {
    let word = |character: char| character.is_alphanumeric() || character == '_';
    !name.is_empty()
        && text.match_indices(name).any(|(start, matched)| {
            !text[..start].chars().next_back().is_some_and(word)
                && !text[start + matched.len()..]
                    .chars()
                    .next()
                    .is_some_and(word)
        })
}

/// Qualifier representation preserves unknown and half-open source bounds.
fn validity(value: &Validity) -> DescriptorValidity {
    match value {
        Validity::Unknown => DescriptorValidity::Unknown { known: false },
        Validity::Bounded { start, end } => DescriptorValidity::Bounded {
            end: end.clone(),
            known: true,
            start: start.clone(),
        },
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
    let qualifiers = DescriptorQualifiers {
        conditions: record.claim.conditions.clone(),
        version: validity(&record.claim.version),
        world: validity(&record.claim.world),
    };
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
    DescriptorError::Refused(message.into())
}
