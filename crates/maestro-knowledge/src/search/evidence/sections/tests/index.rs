use super::super::super::spans::{SeedSpan, SpanUnion};
use super::super::SectionIndex;
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

#[test]
fn a_window_is_marked_when_it_omits_trailing_section_whitespace() -> Result<(), String> {
    let markdown = "## Guide\n\nBody.\n\n## Next\n\nOther.\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "fixture.md"))
        .map_err(|error| error.to_string())?;
    let section = document
        .sections
        .iter()
        .find(|section| section.title == "Guide")
        .ok_or_else(|| "fixture has no Guide section".to_owned())?;
    let body_start = markdown
        .find("Body.")
        .ok_or_else(|| "fixture has no body".to_owned())?;
    let seed = Span {
        start: body_start,
        end: body_start + "Body.".len(),
    };
    let index = SectionIndex::new(&document, markdown)?;
    let expansion = index.expand(&SpanUnion {
        revision_id: "rev-a".to_owned(),
        span: seed,
        seeds: vec![SeedSpan {
            chunk_id: "chunk-a".to_owned(),
            revision_id: "rev-a".to_owned(),
            section_id: Some(section.section_id.clone()),
            span: seed,
            input_position: 0,
            score: Some(1.0),
            routes: BTreeSet::default(),
        }],
    })?;
    let shorter = Span {
        start: expansion.extent.start,
        end: expansion.extent.end - 1,
    };

    assert!(!expansion.is_windowed(expansion.extent));
    assert!(expansion.is_windowed(shorter));
    Ok(())
}
