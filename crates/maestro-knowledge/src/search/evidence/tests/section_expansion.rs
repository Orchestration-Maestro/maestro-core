use super::{SectionIndex, document, union};
use maestro_kernel::evidence::Span;

#[test]
fn crossing_sibling_sections_expands_to_their_parent() {
    let markdown = concat!(
        "## Parent\n\n",
        "### First\n\nFirst body.\n\n",
        "### Second\n\nSecond body.\n\n",
        "## Outside\n\nOutside body.\n",
    );
    let document = document(markdown);
    let parent = document
        .sections
        .iter()
        .find(|section| section.title == "Parent")
        .unwrap();
    let first = document
        .sections
        .iter()
        .find(|section| section.title == "First")
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
    let second = document
        .sections
        .iter()
        .find(|section| section.title == "Second")
        .unwrap();
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

    assert_eq!(
        expansion.section_id.as_deref(),
        Some(parent.section_id.as_str())
    );
    assert_eq!(expansion.section_path, ["Parent"]);
    assert_eq!(expansion.extent.start, markdown.find("## Parent").unwrap());
    assert_eq!(expansion.extent.end, markdown.find("## Outside").unwrap());
}

#[test]
fn a_span_outside_every_content_extent_is_refused() {
    let markdown = "## Main\n\nBody.\n";
    let document = document(markdown);
    let main = document
        .sections
        .iter()
        .find(|section| section.title == "Main")
        .unwrap();
    let mut seed = union(
        &document,
        markdown,
        "Body.",
        "Body.",
        Some(&main.section_id),
    );
    let outside = Span {
        start: markdown.len() + 1,
        end: markdown.len() + 2,
    };
    seed.span = outside;
    for span in &mut seed.seeds {
        span.span = outside;
    }

    assert_eq!(
        SectionIndex::new(&document, markdown)
            .unwrap()
            .expand(&seed)
            .unwrap_err(),
        "content extent does not contain its seed union"
    );
}
