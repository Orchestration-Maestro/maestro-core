//! Unit ranges a chunk may hold: never starting or ending inside an inline delimiter.
use crate::source_units::{SourceUnit, TextRange};

/// A range of a unit's text that neither starts nor ends inside an inline delimiter, extended over
/// closing delimiters and holding some text outside them; none otherwise.
pub(super) fn normalized_range(
    unit: &SourceUnit,
    start: usize,
    mut end: usize,
) -> Option<TextRange> {
    unit.text.get(start..end)?;
    if start == end {
        return None;
    }
    for envelope in &unit.envelopes {
        for wrapper in [envelope.opening, envelope.closing] {
            if wrapper.start < start && start < wrapper.end {
                return None;
            }
        }
        if envelope.opening.start < end && end <= envelope.opening.end {
            return None;
        }
    }
    // Take in closing delimiters; each pass moves past one for good: one pass per envelope.
    for _ in &unit.envelopes {
        let next = unit
            .envelopes
            .iter()
            .filter(|envelope| envelope.closing.start <= end && end < envelope.closing.end)
            .map(|envelope| envelope.closing.end)
            .max();
        match next {
            Some(next) => end = next,
            None => break,
        }
    }
    let text = unit.text.get(start..end)?;
    let substantive = text.char_indices().any(|(offset, _)| {
        let at = start + offset;
        !unit.envelopes.iter().any(|envelope| {
            [envelope.opening, envelope.closing]
                .iter()
                .any(|wrapper| wrapper.start <= at && at < wrapper.end)
        })
    });
    substantive.then_some(TextRange { start, end })
}
