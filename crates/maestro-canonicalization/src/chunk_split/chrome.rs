//! Page chrome: the part of each unit a profile's chrome rules leave out of every indexed text,
//! while its bytes stay in the source.
use super::ranges::normalized_range;
use crate::{
    chunk_profile::{Chrome, ChromeRule},
    content::{Block, BlockType},
    document::CanonicalDocument,
    source_units::{MappedDocument, SourceUnit, TextRange},
};
use std::collections::BTreeMap;

/// What a profile indexes of each unit: the part kept, none for chrome, and the chrome rule that
/// left the rest out.
pub(super) struct Indexed {
    /// For each unit, the part of its text indexed; none for page chrome.
    pub(super) kept: Vec<Option<TextRange>>,
    /// For each unit, the rule that left some of its text out; none when it is indexed whole.
    pub(super) rules: Vec<Option<ChromeRule>>,
}

impl Indexed {
    /// Index a unit as `kept` only, `rule` having left the rest out.
    fn keep(&mut self, unit: usize, kept: Option<TextRange>, rule: ChromeRule) {
        if let (Some(slot), Some(reason)) = (self.kept.get_mut(unit), self.rules.get_mut(unit)) {
            *slot = kept;
            *reason = Some(rule);
        }
    }
}

/// The part of each unit `chrome` indexes: none for a label or markup block, the text before a
/// heading's or a paragraph's trailing label, else the whole text; every unit whole without
/// chrome rules.
pub(super) fn indexed(
    document: &CanonicalDocument,
    mapped: &MappedDocument,
    chrome: Option<&Chrome>,
) -> Indexed {
    let mut result = Indexed {
        kept: mapped.units.iter().map(|unit| Some(whole(unit))).collect(),
        rules: vec![None; mapped.units.len()],
    };
    let Some(chrome) = chrome else {
        return result;
    };
    let mut owned: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, unit) in mapped.units.iter().enumerate() {
        if unit.primary {
            owned.entry(unit.block_id.as_str()).or_default().push(index);
        }
    }
    let owning: Vec<(&Block, &[usize])> = document
        .blocks
        .iter()
        .filter_map(|block| Some((block, owned.get(block.block_id.as_str())?.as_slice())))
        .collect();
    for (position, &(block, units)) in owning.iter().enumerate() {
        match block.block_type {
            BlockType::Heading => {
                let suffixes = (chrome.heading_suffixes, ChromeRule::HeadingSuffix);
                trim_suffix(mapped, units, suffixes, &mut result);
            }
            BlockType::Paragraph | BlockType::Html => {
                let next = owning.get(position + 1).map(|&(_, next)| next);
                paragraph_chrome(chrome, mapped, (block, units), next, &mut result);
            }
            _ => {}
        }
    }
    result
}

/// Leave out a paragraph or HTML block's `units` as a whole when it is chrome, the next block
/// owning `next`; else a paragraph's trailing label.
fn paragraph_chrome(
    chrome: &Chrome,
    mapped: &MappedDocument,
    (block, units): (&Block, &[usize]),
    next: Option<&[usize]>,
    result: &mut Indexed,
) {
    let next = next.map(|next| text(mapped, next));
    if let Some(rule) = whole_chrome(chrome, &text(mapped, units), next.as_deref()) {
        for &unit in units {
            result.keep(unit, None, rule);
        }
    } else if block.block_type == BlockType::Paragraph {
        let suffixes = (chrome.paragraph_suffixes, ChromeRule::ParagraphSuffix);
        trim_suffix(mapped, units, suffixes, result);
    }
}

/// A heading title without a trailing label `chrome` leaves out.
pub(crate) fn heading_title(chrome: Option<&Chrome>, title: &str) -> String {
    chrome
        .and_then(|chrome| {
            chrome
                .heading_suffixes
                .iter()
                .find_map(|label| title.strip_suffix(label))
        })
        .unwrap_or(title)
        .to_owned()
}

/// A unit's whole text as a range.
fn whole(unit: &SourceUnit) -> TextRange {
    TextRange {
        start: 0,
        end: unit.text.len(),
    }
}

/// The text of a block's units, joined.
fn text(mapped: &MappedDocument, units: &[usize]) -> String {
    units
        .iter()
        .filter_map(|&index| mapped.units.get(index))
        .map(|unit| unit.text.as_str())
        .collect()
}

/// Keep a block's text before a label its last unit ends with, one of `suffixes` with the rule
/// that names them; a last unit holding only the label, and spaces, is chrome as a whole. A cut
/// inside an inline wrapper keeps the unit whole.
fn trim_suffix(
    mapped: &MappedDocument,
    units: &[usize],
    (suffixes, rule): (&[&str], ChromeRule),
    result: &mut Indexed,
) {
    let Some(&last) = units.last() else {
        return;
    };
    let Some((unit, before)) = mapped.units.get(last).and_then(|unit| {
        suffixes
            .iter()
            .find_map(|label| unit.text.strip_suffix(label))
            .map(|before| (unit, before))
    }) else {
        return;
    };
    let range = TextRange {
        start: 0,
        end: before.len(),
    };
    if before.trim().is_empty() {
        result.keep(last, None, rule);
    } else if normalized_range(unit, 0, range.end) == Some(range) {
        result.keep(last, Some(range), rule);
    }
}

/// The rule under which a block of `text` is chrome as a whole, the block after it holding
/// `next`: a label alone, a label before an image placeholder, or markup of known elements
/// alone.
fn whole_chrome(chrome: &Chrome, text: &str, next: Option<&str>) -> Option<ChromeRule> {
    let trimmed = text.trim();
    if chrome.labels.contains(&trimmed) {
        return Some(ChromeRule::Label);
    }
    if chrome.labels_before_image.contains(&trimmed)
        && next.is_some_and(|next| chrome.image_placeholders.contains(&next.trim()))
    {
        return Some(ChromeRule::LabelBeforeImage);
    }
    chrome
        .markup_elements
        .filter(|elements| markup_only(text, elements))
        .map(|_| ChromeRule::Markup)
}

/// Whether a text holds nothing but HTML comments, tags of `elements` and whitespace: an image
/// placeholder such as `<!-- image -->`, an anchor target or line breaks. A tag of another name,
/// such as an XML setting or a placeholder, is content.
fn markup_only(text: &str, elements: &[&str]) -> bool {
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return true;
        }
        let end = if let Some(comment) = rest.strip_prefix("<!--") {
            comment
                .find("-->")
                .map(|end| "<!--".len() + end + "-->".len())
        } else if let Some(tag) = rest.strip_prefix('<') {
            tag.find('>')
                .filter(|&end| known_element(tag.get(..end).unwrap_or_default(), elements))
                .map(|end| "<".len() + end + ">".len())
        } else {
            None
        };
        match end.and_then(|end| rest.get(end..)) {
            Some(after) => rest = after,
            None => return false,
        }
    }
}

/// Whether a tag's inside, between its angle brackets, opens or closes one of `elements`.
fn known_element(inside: &str, elements: &[&str]) -> bool {
    let name = inside.strip_prefix('/').unwrap_or(inside);
    let end = name
        .find(|character: char| !character.is_ascii_alphanumeric())
        .unwrap_or(name.len());
    let (name, rest) = name.split_at(end);
    (rest.is_empty()
        || rest.starts_with(|character: char| character.is_whitespace() || character == '/'))
        && elements
            .iter()
            .any(|element| element.eq_ignore_ascii_case(name))
}
