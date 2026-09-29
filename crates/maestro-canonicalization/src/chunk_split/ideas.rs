//! Ideas: the atoms that belong in one chunk. Under the complete-ideas profile a whole step, with
//! its substeps, is one idea, and a small chunk joins the chunk before it in its section; under
//! the structural profile every atom is a piece alone.
use super::structure::{Body, Layout};
use crate::{
    chunk_profile::Packing, content::BlockType, error::Error, prepared_inputs::ChunkContent,
};

/// What packing places: an idea of several atoms, packed whole while it fits one chunk, or a
/// single atom.
pub(super) enum Piece {
    /// Atoms that belong together: a whole step with its substeps.
    Idea(Vec<Body>),
    /// One atom.
    Atom(Body),
}

impl Piece {
    /// The piece as one body.
    pub(super) fn body(&self) -> Body {
        match self {
            Self::Atom(atom) => atom.clone(),
            Self::Idea(atoms) => {
                let mut body = Body {
                    fragments: Vec::new(),
                    windows: Vec::new(),
                };
                for atom in atoms {
                    body = body.combined(atom);
                }
                body
            }
        }
    }
}

/// The pieces of a document's atoms: when a section packs as a whole, consecutive atoms under one
/// outermost list item, in one section, make an idea; every other atom is a piece alone.
pub(super) fn pieces(layout: &Layout<'_>, atoms: Vec<Body>) -> Vec<Piece> {
    let mut groups: Vec<Vec<Body>> = Vec::new();
    let mut previous = None;
    for atom in atoms {
        let key = step(layout, &atom);
        match groups.last_mut() {
            Some(group) if key.is_some() && key == previous => group.push(atom),
            _ => groups.push(vec![atom]),
        }
        previous = key;
    }
    groups
        .into_iter()
        .map(|group| match <[Body; 1]>::try_from(group) {
            Ok([atom]) => Piece::Atom(atom),
            Err(atoms) => Piece::Idea(atoms),
        })
        .collect()
}

/// The step an atom belongs to when a section packs as a whole: its section and the outermost
/// list item around it; none outside lists.
fn step(layout: &Layout<'_>, atom: &Body) -> Option<(Option<String>, String)> {
    if layout.rules().packing != Packing::Section {
        return None;
    }
    let unit = atom.fragments.first()?.contribution.unit_index;
    let item = layout
        .ancestors(unit)
        .iter()
        .find(|block| block.block_type == BlockType::ListItem)?;
    Some((layout.section(unit), item.block_id.clone()))
}

/// The drafts with each one under `min_tokens` joined to the draft before it, when the two are
/// compatible and fit the maximum together.
pub(super) fn absorb_small(
    layout: &Layout<'_>,
    drafts: Vec<ChunkContent>,
    min_tokens: usize,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<Vec<ChunkContent>, Error> {
    let mut result: Vec<ChunkContent> = Vec::new();
    for draft in drafts {
        if draft.token_count < min_tokens
            && let Some(previous) = result.last_mut()
            && let Some(joined) = joined(layout, previous, &draft, count)?
        {
            *previous = joined;
            continue;
        }
        result.push(draft);
    }
    Ok(result)
}

/// Two drafts as one chunk, when they may share one.
fn joined(
    layout: &Layout<'_>,
    previous: &ChunkContent,
    small: &ChunkContent,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<Option<ChunkContent>, Error> {
    let body = |draft: &ChunkContent| Body {
        fragments: draft.fragments.clone(),
        windows: draft.table_windows.clone(),
    };
    Ok(layout
        .grown(&body(previous), &body(small), count)?
        .map(|(_, joined)| joined))
}
