//! Chunking profiles: which blocks share a chunk and what its prepared input leaves out.
use crate::prepared_inputs::PREPARATION_PROFILE;
use crate::source_units::CHUNKER_VERSION;
use serde::{Deserialize, Serialize};

/// How a document's mapped units become chunks. Its two names enter every chunk and
/// prepared-input identity, so the chunks of two profiles never share an identity, and the
/// chunks a profile once gave stay reproducible while another profile is tried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChunkProfile {
    /// `mapped-structural-chunks/2` with `canonical-context-parts/v1`: the blocks of one section
    /// and one container packed together, every rendered unit indexed.
    #[default]
    Structural,
    /// `mapped-structural-chunks/3` with `canonical-context-parts/v2`: page chrome left out of
    /// the indexed text, and a section's blocks packed together across their containers, a
    /// chunk ending only between whole steps and whole rows.
    CompleteIdeas,
}

impl ChunkProfile {
    /// Every profile, the default first.
    pub const ALL: [Self; 2] = [Self::Structural, Self::CompleteIdeas];

    /// The chunker version: its packing and chunk identity rules.
    #[must_use]
    pub const fn chunker_version(self) -> &'static str {
        match self {
            Self::Structural => CHUNKER_VERSION,
            Self::CompleteIdeas => "mapped-structural-chunks/3",
        }
    }

    /// The preparation profile: the context and formatting of each prepared input.
    #[must_use]
    pub const fn preparation_profile(self) -> &'static str {
        match self {
            Self::Structural => PREPARATION_PROFILE,
            Self::CompleteIdeas => "canonical-context-parts/v2",
        }
    }

    /// The profile whose chunker version is `name`; none for another name.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.chunker_version() == name)
    }

    /// The rules the chunker follows under this profile; it reads them and never the profile.
    pub(crate) const fn rules(self) -> ChunkRules {
        match self {
            Self::Structural => ChunkRules {
                packing: Packing::Container,
                min_tokens: None,
                chrome: None,
            },
            Self::CompleteIdeas => ChunkRules {
                packing: Packing::Section,
                min_tokens: Some(150),
                chrome: Some(Chrome {
                    heading_suffixes: &[" Link copied to clipboard"],
                    paragraph_suffixes: &[" Copy Copied to clipboard"],
                    labels: &["Copy Copied to clipboard"],
                    labels_before_image: &["Closed"],
                    image_placeholders: &["<!-- image -->"],
                    markup_elements: Some(&[
                        "a", "br", "img", "div", "span", "p", "hr", "table", "thead", "tbody",
                        "tfoot", "tr", "th", "td", "caption", "colgroup", "col",
                    ]),
                }),
            },
        }
    }
}

/// What a profile's chunker does: which blocks share a chunk, how small a chunk may stay and
/// which page chrome leaves the indexed text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChunkRules {
    /// Which blocks may share a chunk.
    pub(crate) packing: Packing,
    /// A chunk smaller than this joins the chunk before it in its section, when the two fit the
    /// maximum together; none keeps every chunk as packed.
    pub(crate) min_tokens: Option<usize>,
    /// The page chrome left out of the indexed text; none indexes every rendered unit whole.
    pub(crate) chrome: Option<Chrome>,
}

/// Which blocks may share a chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Packing {
    /// Blocks of one section inside the same containers; table rows only with rows of their table.
    Container,
    /// Blocks of one section across their containers: a whole step, with its substeps, packs as
    /// one idea, and table rows join the blocks around them, one table per chunk.
    Section,
}

/// Page chrome: interface labels an HTML export keeps around a page's content. Its bytes stay
/// in the source; only the indexed text leaves them out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Chrome {
    /// A heading ending with one of these keeps the text before it: a copy-link button's label.
    pub(crate) heading_suffixes: &'static [&'static str],
    /// A paragraph ending with one of these keeps the text before it: a copy button's label.
    pub(crate) paragraph_suffixes: &'static [&'static str],
    /// A paragraph whose whole text is one of these is chrome: a copy button's label.
    pub(crate) labels: &'static [&'static str],
    /// A paragraph whose whole text is one of these is chrome when the next block is an image
    /// placeholder: the state label of a collapsed image.
    pub(crate) labels_before_image: &'static [&'static str],
    /// The whole text of an image placeholder block.
    pub(crate) image_placeholders: &'static [&'static str],
    /// A paragraph or HTML block holding only comments and tags of these HTML elements is
    /// chrome: an image placeholder, an empty anchor, line breaks. A tag of any other name keeps
    /// the block. None leaves every such block indexed.
    pub(crate) markup_elements: Option<&'static [&'static str]>,
}

/// The chrome rule that left a unit's text, or part of it, out of the indexed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChromeRule {
    /// A label ending a heading.
    HeadingSuffix,
    /// A label ending a paragraph.
    ParagraphSuffix,
    /// A paragraph that is a label alone.
    Label,
    /// A paragraph that is a label alone, before an image placeholder.
    LabelBeforeImage,
    /// A paragraph or HTML block of markup alone.
    Markup,
}
