//! Packing a document's ideas into drafts: combined up to the target, broken into smaller ideas,
//! refined or split past the maximum.
use super::ideas::{Piece, absorb_small, pieces};
use super::limits::{MAX_TOKENS, TARGET_TOKENS};
use super::refusal::structure_error;
use super::structure::{Body, Layout};
use crate::{error::Error, prepared_inputs::ChunkContent};
use std::collections::{BTreeMap, VecDeque};

/// Pack a document's pieces into drafts: combine compatible pieces up to the target; an idea over
/// the maximum continues atom by atom, an atom over it is refined or split; then number each
/// unit's parts. A refinement that hands a body back unchanged would never end, so it refuses the
/// body's first unit by name. Under a profile with a minimum, a small draft then joins the draft
/// before it.
pub(crate) fn build_drafts(
    layout: &Layout<'_>,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<Vec<ChunkContent>, Error> {
    let mut pending: VecDeque<_> = pieces(layout, layout.atoms()?).into();
    let mut current: Option<(Body, ChunkContent)> = None;
    let mut result = Vec::new();
    while let Some(piece) = pending.pop_front() {
        let body = piece.body();
        if let Some((open, prepared)) = current.take() {
            if let Some((combined, candidate)) = layout.grown(&open, &body, count)? {
                current = settle(&mut result, combined, candidate);
                continue;
            }
            if let Piece::Idea(atoms) = &piece
                && layout.prepare(&body, count)?.token_count > MAX_TOKENS
            {
                // Too large for any chunk: its atoms may still join the open draft.
                requeue(&mut pending, atoms.clone());
                current = Some((open, prepared));
                continue;
            }
            result.push(prepared);
        }
        let prepared = layout.prepare(&body, count)?;
        if prepared.token_count <= MAX_TOKENS {
            current = settle(&mut result, body, prepared);
            continue;
        }
        match piece {
            Piece::Idea(atoms) => requeue(&mut pending, atoms),
            Piece::Atom(atom) => match layout.refine(&atom)? {
                Some(refined) if refined.contains(&atom) => {
                    let first = atom.fragments.first().ok_or_else(structure_error)?;
                    return Err(layout.oversized(first.contribution.unit_index));
                }
                Some(refined) => requeue(&mut pending, refined),
                None => result.extend(layout.split_unit(&atom, count)?),
            },
        }
    }
    if let Some((_, prepared)) = current {
        result.push(prepared);
    }
    if let Some(min_tokens) = layout.rules().min_tokens {
        result = absorb_small(layout, result, min_tokens, count)?;
    }
    let mut ordinals = BTreeMap::new();
    for chunk in &mut result {
        for fragment in &mut chunk.fragments {
            let ordinal = ordinals
                .entry(fragment.contribution.unit_index)
                .or_insert(0);
            fragment.part_ordinal = *ordinal;
            *ordinal += 1;
        }
    }
    Ok(result)
}

/// Put `atoms` back at the front of the pending pieces, in order, each a piece alone.
fn requeue(pending: &mut VecDeque<Piece>, atoms: Vec<Body>) {
    for atom in atoms.into_iter().rev() {
        pending.push_front(Piece::Atom(atom));
    }
}

/// A draft that reached the target is complete and joins the result; a smaller one stays open.
fn settle(
    result: &mut Vec<ChunkContent>,
    body: Body,
    prepared: ChunkContent,
) -> Option<(Body, ChunkContent)> {
    if prepared.token_count >= TARGET_TOKENS {
        result.push(prepared);
        None
    } else {
        Some((body, prepared))
    }
}
