//! A canonical document's body as the checks count it, and the two checks of
//! what it holds: near-empty, and navigation-heavy.
//!
//! The body is every innermost block, one without a block inside it, but
//! headings, front matter, thematic breaks and link reference definitions. A
//! word is a run of characters between whitespace that holds a letter or a
//! digit, in the text of a block's inline content: its code included, the
//! alternative text of an image and raw HTML left out. The words inside a
//! link are its label.

use super::flag::{Flag, counted, number};
use maestro_canonicalization::{
    Block, BlockType, CanonicalDocument, ContentNode, Inline, InlineKind,
};
use maestro_kernel::document::Outcome;
use std::collections::HashMap;

/// A body with fewer words than this outside link labels, and no code block,
/// list item or table cell holding one, is near-empty: less than a sentence.
const NEAR_EMPTY_WORDS: u64 = 8;

/// A body whose link labels are this percentage of its words or more…
const NAVIGATION_PERCENT: u64 = 90;

/// …over this many links or more, is navigation rather than content.
const NAVIGATION_LINKS: u64 = 10;

/// What the checks count in a document's body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Body {
    /// Every word.
    pub(super) words: u64,
    /// The words of link labels.
    link_words: u64,
    /// The links, images apart.
    links: u64,
    /// The words outside link labels of the blocks that are neither code,
    /// nor in a list item, nor a table cell: prose.
    prose: u64,
    /// Whether a code block, a block in a list item or a table cell holds a
    /// word outside a link label: an entry, whole on its own.
    entries: bool,
}

impl Body {
    /// The body of `document`.
    pub(super) fn of(document: &CanonicalDocument) -> Self {
        let by_id: HashMap<&str, &Block> = document
            .blocks
            .iter()
            .map(|block| (block.block_id.as_str(), block))
            .collect();
        let mut body = Self::default();
        for block in document.blocks.iter().filter(|block| in_body(block)) {
            let count = Count::of(block);
            body.words += count.words;
            body.link_words += count.link_words;
            body.links += count.links;
            let own = count.words - count.link_words;
            if block.block_type == BlockType::Code
                || block.block_type == BlockType::TableCell
                || in_list(block, &by_id)
            {
                body.entries |= own > 0;
            } else {
                body.prose += own;
            }
        }
        body
    }
}

/// What one block's inline content holds.
#[derive(Debug, Clone, Copy, Default)]
struct Count {
    /// Its words.
    words: u64,
    /// The words of its link labels.
    link_words: u64,
    /// Its links.
    links: u64,
}

impl Count {
    /// What the inline content of `block` holds.
    fn of(block: &Block) -> Self {
        let mut count = Self::default();
        for node in &block.structured_content.children {
            if let ContentNode::Inline { inline } = node {
                count.add(inline, false);
            }
        }
        count
    }

    /// Adds the words and links of `inline`, inside a link if `in_link`.
    fn add(&mut self, inline: &Inline, in_link: bool) {
        match &inline.content {
            InlineKind::Text { text }
            | InlineKind::Code { text }
            | InlineKind::Math { text, .. } => {
                let words = words(text);
                self.words += words;
                if in_link {
                    self.link_words += words;
                }
            }
            InlineKind::Link { .. } => self.links += 1,
            InlineKind::Image { .. } => return,
            _ => {}
        }
        let in_link = in_link || matches!(inline.content, InlineKind::Link { .. });
        for child in &inline.children {
            self.add(child, in_link);
        }
    }
}

/// The words of `text`: its runs between whitespace that hold a letter or a
/// digit.
fn words(text: &str) -> u64 {
    number(
        text.split_whitespace()
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .count(),
    )
}

/// Whether `block` holds no block: an innermost one, whose text is its own.
pub(super) fn innermost(block: &Block) -> bool {
    block
        .structured_content
        .children
        .iter()
        .all(|node| matches!(node, ContentNode::Inline { .. }))
}

/// Whether `block` is one of the body's.
fn in_body(block: &Block) -> bool {
    innermost(block)
        && !matches!(
            block.block_type,
            BlockType::Heading
                | BlockType::Metadata
                | BlockType::ThematicBreak
                | BlockType::ReferenceDefinition
        )
}

/// Whether `block` is a list item or lies in one, its parents found in
/// `by_id`.
fn in_list(block: &Block, by_id: &HashMap<&str, &Block>) -> bool {
    let mut current = Some(block);
    while let Some(block) = current {
        if block.block_type == BlockType::ListItem {
            return true;
        }
        current = block
            .parent_block_id
            .as_deref()
            .and_then(|parent| by_id.get(parent).copied());
    }
    false
}

/// `body.near-empty`: fewer than [`NEAR_EMPTY_WORDS`] words of prose outside
/// link labels, and no entry. Format-aware: a list item, a code block or a
/// table cell is an entry on its own, so a short changelog or a two-line
/// configuration file is not near-empty, while a page whose body is a link,
/// a label or nothing is.
pub(super) fn near_empty(body: &Body) -> Option<Flag> {
    (!body.entries && body.prose < NEAR_EMPTY_WORDS).then(|| Flag {
        rule: "body.near-empty",
        outcome: Outcome::NeedsReextraction,
        reason: format!(
            "its body holds {} outside link labels, and no code block, list item or table \
             cell holds one: fewer than {NEAR_EMPTY_WORDS} is near-empty",
            counted(body.prose, "word")
        ),
    })
}

/// `body.navigation-heavy`: link labels are [`NAVIGATION_PERCENT`] % of the
/// body's words or more, over [`NAVIGATION_LINKS`] links or more. An index
/// that says what each link holds is content; a few links alone are left to
/// `body.near-empty`.
pub(super) fn navigation_heavy(body: &Body) -> Option<Flag> {
    let percent = (body.link_words * 100)
        .checked_div(body.words)
        .unwrap_or(100);
    (body.links >= NAVIGATION_LINKS && percent >= NAVIGATION_PERCENT).then(|| Flag {
        rule: "body.navigation-heavy",
        outcome: Outcome::NeedsReextraction,
        reason: format!(
            "link labels are {percent} % of its body's {}, over {}: from \
             {NAVIGATION_PERCENT} % over {NAVIGATION_LINKS} links, a page is navigation",
            counted(body.words, "word"),
            counted(body.links, "link")
        ),
    })
}
