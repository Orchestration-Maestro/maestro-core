use super::super::{
    sections::SectionIndex,
    spans::{SeedSpan, SpanUnion},
};
use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "fixture.md")).unwrap()
}

fn union(
    document: &CanonicalDocument,
    markdown: &str,
    start_text: &str,
    end_text: &str,
    section_id: Option<&str>,
) -> SpanUnion {
    let start = markdown.find(start_text).unwrap();
    let end = markdown.find(end_text).unwrap() + end_text.len();
    SpanUnion {
        revision_id: document.revision_id.clone(),
        span: Span { start, end },
        seeds: vec![SeedSpan {
            chunk_id: "chunk-a".to_owned(),
            revision_id: document.revision_id.clone(),
            section_id: section_id.map(str::to_owned),
            span: Span { start, end },
            input_position: 0,
            score: Some(0.9),
            routes: BTreeSet::new(),
        }],
    }
}

#[test]
fn enclosing_section_includes_deeper_content_and_stops_at_equal_level_sibling() {
    let markdown = concat!(
        "# Guide\n\n## Install\n\nIntro text.\n\n### Detail\n\n",
        "Detail text.\n\n## Other\n\nOutside text.\n"
    );
    let document = document(markdown);
    let install = document
        .sections
        .iter()
        .find(|section| section.title == "Install")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Intro text.",
        "Detail text.",
        Some(&install.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(
        expansion.section_id.as_deref(),
        Some(install.section_id.as_str())
    );
    assert_eq!(expansion.section_path, ["Guide", "Install"]);
    assert_eq!(expansion.extent.start, markdown.find("## Install").unwrap());
    assert_eq!(expansion.extent.end, markdown.find("## Other").unwrap());
}

#[test]
fn repeated_heading_text_is_resolved_by_its_section_id() {
    let markdown = "## Step\n\nFirst body.\n\n## Step\n\nSecond body.\n";
    let document = document(markdown);
    let second = document
        .sections
        .iter()
        .filter(|section| section.title == "Step")
        .nth(1)
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Second body.",
        "Second body.",
        Some(&second.section_id),
    );
    let index = SectionIndex::new(&document, markdown).unwrap();
    let extent = index.section_extent(&second.section_id).unwrap();
    assert_eq!(extent.start, markdown.rfind("## Step").unwrap());
    assert_eq!(
        markdown.get(extent.start..extent.end),
        Some("## Step\n\nSecond body.\n")
    );
    assert_eq!(index.section_extent("unknown-section"), None);

    let expansion = index.expand(&seed).unwrap();
    assert_eq!(
        expansion.section_id.as_deref(),
        Some(second.section_id.as_str())
    );
    assert_eq!(expansion.extent, extent);
}

#[test]
fn a_union_crossing_sections_uses_document_content_scope() {
    let markdown = "## First\n\nFirst body.\n\n## Second\n\nSecond body.\n";
    let document = document(markdown);
    let first = document
        .sections
        .iter()
        .find(|section| section.title == "First")
        .unwrap();
    let second = document
        .sections
        .iter()
        .find(|section| section.title == "Second")
        .unwrap();
    let mut seed = union(
        &document,
        markdown,
        "First body.",
        "Second body.",
        Some(&first.section_id),
    );
    let mut first_seed = union(
        &document,
        markdown,
        "First body.",
        "First body.",
        Some(&first.section_id),
    );
    let mut second_seed = union(
        &document,
        markdown,
        "Second body.",
        "Second body.",
        Some(&second.section_id),
    );
    seed.seeds = vec![first_seed.seeds.remove(0), second_seed.seeds.remove(0)];
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(expansion.section_id, None);
    assert!(expansion.section_path.is_empty());
    assert_eq!(expansion.extent.start, markdown.find("## First").unwrap());
    assert!(expansion.extent.end >= seed.span.end);
}

#[test]
fn the_deepest_containing_section_wins() {
    let markdown = "## Install\n\n### Detail\n\nDetail text.\n\n## Other\n";
    let document = document(markdown);
    let install = document
        .sections
        .iter()
        .find(|section| section.title == "Install")
        .unwrap();
    let detail = document
        .sections
        .iter()
        .find(|section| section.title == "Detail")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Detail text.",
        "Detail text.",
        Some(&install.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(
        expansion.section_id.as_deref(),
        Some(detail.section_id.as_str())
    );
}

#[test]
fn source_span_selects_an_enclosing_section_when_the_named_section_does_not_contain_it() {
    let markdown = "## First\n\nFirst body.\n\n## Second\n\nSecond body.\n";
    let document = document(markdown);
    let first = document
        .sections
        .iter()
        .find(|section| section.title == "First")
        .unwrap();
    let second = document
        .sections
        .iter()
        .find(|section| section.title == "Second")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Second body.",
        "Second body.",
        Some(&first.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(
        expansion.section_id.as_deref(),
        Some(second.section_id.as_str())
    );
    assert_eq!(expansion.section_path, ["Second"]);
}

#[test]
fn an_unknown_named_section_is_refused() {
    let markdown = "## First\n\nFirst body.\n";
    let document = document(markdown);
    let seed = union(
        &document,
        markdown,
        "First body.",
        "First body.",
        Some("missing"),
    );

    assert_eq!(
        SectionIndex::new(&document, markdown)
            .unwrap()
            .expand(&seed)
            .unwrap_err(),
        "candidate names a missing canonical section"
    );
}

#[test]
fn latest_start_selects_between_equal_depth_containing_sections() {
    let markdown = concat!(
        "## Main\n\nLead text.\n\n> ## Quoted heading\n",
        "> Quoted text.\n\nTail text.\n\n## Next\n"
    );
    let document = document(markdown);
    let main = document
        .sections
        .iter()
        .find(|section| section.title == "Main")
        .unwrap();
    let quoted = document
        .sections
        .iter()
        .find(|section| section.title == "Quoted heading")
        .unwrap();
    assert_eq!(main.heading_path.len(), quoted.heading_path.len());
    let seed = union(
        &document,
        markdown,
        "Quoted text.",
        "Quoted text.",
        Some(&main.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(
        expansion.section_id.as_deref(),
        Some(quoted.section_id.as_str())
    );
}

#[test]
fn invalid_seed_unions_are_rejected() {
    let markdown = "## First\n\nFirst body.\n";
    let document = document(markdown);
    let section = &document.sections[0];
    let mut empty = union(
        &document,
        markdown,
        "First body.",
        "First body.",
        Some(&section.section_id),
    );
    empty.span.end = empty.span.start;
    assert_eq!(
        SectionIndex::new(&document, markdown)
            .unwrap()
            .expand(&empty)
            .unwrap_err(),
        "seed union is invalid"
    );

    let mut mismatched_revision = union(
        &document,
        markdown,
        "First body.",
        "First body.",
        Some(&section.section_id),
    );
    mismatched_revision.seeds[0].revision_id = "other-revision".to_owned();
    assert_eq!(
        SectionIndex::new(&document, markdown)
            .unwrap()
            .expand(&mismatched_revision)
            .unwrap_err(),
        "seed union is invalid"
    );
}

#[test]
fn nested_quote_headings_do_not_end_a_root_section() {
    let markdown = concat!(
        "## Main\n\nLead text.\n\n> ## Quoted heading\n",
        "> Quoted text.\n\nTail text.\n\n## Next\n\nNext text.\n"
    );
    let document = document(markdown);
    let main = document
        .sections
        .iter()
        .find(|section| section.title == "Main")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Lead text.",
        "Lead text.",
        Some(&main.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(expansion.extent.end, markdown.find("## Next").unwrap());
}

#[test]
fn a_window_is_mandatory_whole_siblings_plus_two_neighbors_each_side() {
    let markdown = concat!(
        "## A\n\nBefore text.\n\nSeed text.\n\nAfter one.\n",
        "\nAfter two.\n\nAfter three.\n\n## B\n\nOutside.\n"
    );
    let document = document(markdown);
    let section = document
        .sections
        .iter()
        .find(|section| section.title == "A")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Seed text.",
        "Seed text.",
        Some(&section.section_id),
    );
    let index = SectionIndex::new(&document, markdown).unwrap();
    let expansion = index.expand(&seed).unwrap();
    let window = expansion.window_plan(seed.span).unwrap();

    assert_eq!(
        window.mandatory,
        Span {
            start: markdown.find("Seed text.").unwrap(),
            end: markdown.find("Seed text.").unwrap() + "Seed text.\n".len(),
        }
    );
    assert_eq!(window.before.len(), 2);
    assert_eq!(window.after.len(), 2);
    assert_eq!(
        window.before[0].start,
        markdown.find("Before text.").unwrap()
    );
    assert_eq!(window.before[1].start, markdown.find("## A").unwrap());
    assert_eq!(window.after[0].start, markdown.find("After one.").unwrap());
    assert_eq!(window.after[1].start, markdown.find("After two.").unwrap());
    assert_eq!(
        expansion
            .window_plan(Span {
                start: seed.span.start,
                end: seed.span.start,
            })
            .unwrap_err(),
        "window seed is outside its expansion extent"
    );
}

#[test]
fn a_shorter_side_does_not_shift_the_order_of_other_window_neighbors() {
    let markdown = "Before text.\n\nSeed text.\n\nAfter one.\n\nAfter two.\n";
    let document = document(markdown);
    let seed = union(&document, markdown, "Seed text.", "Seed text.", None);
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();
    let window = expansion.window_plan(seed.span).unwrap();

    assert_eq!(window.before.len(), 1);
    assert_eq!(
        window.before[0].start,
        markdown.find("Before text.").unwrap()
    );
    assert_eq!(window.after.len(), 2);
    assert_eq!(window.after[0].start, markdown.find("After one.").unwrap());
    assert_eq!(window.after[1].start, markdown.find("After two.").unwrap());
}

#[test]
fn a_mandatory_window_includes_every_sibling_crossed_by_its_seed() {
    let markdown = concat!(
        "## A\n\nBefore text.\n\nSeed text.\n\nAfter one.\n",
        "\nAfter two.\n\n## B\n\nOutside.\n"
    );
    let document = document(markdown);
    let section = document
        .sections
        .iter()
        .find(|section| section.title == "A")
        .unwrap();
    let seed = union(
        &document,
        markdown,
        "Before text.",
        "After one.",
        Some(&section.section_id),
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();
    let window = expansion.window_plan(seed.span).unwrap();

    assert_eq!(
        markdown.get(window.mandatory.start..window.mandatory.end),
        Some("Before text.\n\nSeed text.\n\nAfter one.\n")
    );
}

#[test]
fn an_unsectioned_union_crossing_a_heading_uses_document_content_scope() {
    let markdown = "Preamble text.\n\n## Heading\n\nSection text.\n";
    let document = document(markdown);
    let seed = union(&document, markdown, "Preamble text.", "Section text.", None);
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(expansion.section_id, None);
    assert!(expansion.section_path.is_empty());
    assert_eq!(
        expansion.extent.start,
        markdown.find("Preamble text.").unwrap()
    );
    assert!(expansion.extent.end >= seed.span.end);
}

#[test]
fn root_preamble_windows_never_include_the_first_heading() {
    let markdown = "Preamble text.\n\n## Heading\n\nSection text.\n";
    let document = document(markdown);
    let seed = union(
        &document,
        markdown,
        "Preamble text.",
        "Preamble text.",
        None,
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();
    let window = expansion.window_plan(seed.span).unwrap();

    assert!(window.after.is_empty());
}

#[test]
fn a_chunk_without_a_section_uses_the_root_preamble_without_inventing_an_id() {
    let markdown = "Preamble text.\n\n## Heading\n\nSection text.\n";
    let document = document(markdown);
    let seed = union(
        &document,
        markdown,
        "Preamble text.",
        "Preamble text.",
        None,
    );
    let expansion = SectionIndex::new(&document, markdown)
        .unwrap()
        .expand(&seed)
        .unwrap();

    assert_eq!(expansion.section_id, None);
    assert!(expansion.section_path.is_empty());
    assert_eq!(
        expansion.extent.start,
        markdown.find("Preamble text.").unwrap()
    );
    assert_eq!(expansion.extent.end, markdown.find("## Heading").unwrap());
}

#[path = "section_expansion.rs"]
mod expansion;
#[path = "section_selection.rs"]
mod selection;
