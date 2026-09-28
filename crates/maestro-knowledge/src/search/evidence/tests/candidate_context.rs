use super::support::{control, fixture};
use crate::search::evidence::{candidate_context::context, source::SourceCache};
use maestro_kernel::evidence::Span;
use std::fmt::Write as _;

#[test]
fn bounded_context_joins_intro_and_steps_without_changing_spans() {
    let markdown = concat!(
        "# Guide\n\n## Running\n\nStart here.\n\n",
        "1. Select a task.\n2. Press Run.\n\n## Other\n\nUnrelated.\n"
    );
    let fixture = fixture(&[("guide.md", markdown)]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| markdown[chunk.span.start..chunk.span.end].contains("Start here"))
        .unwrap();
    let original = chunk.clone();
    let control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &control);
    let source = cache.load(&chunk.revision_id, &fixture.generation).unwrap();
    let (path, text) = context(source, chunk, Some(1500)).unwrap();
    let text = text.unwrap();
    assert!(path.contains("Running"));
    assert!(text.contains("Start here."));
    assert!(text.contains("1. Select a task.\n2. Press Run."));
    assert!(!text.contains("Unrelated"));
    assert!(text.len() <= 1500);
    assert_eq!(chunk, &original);
    assert!(context(source, chunk, None).unwrap().1.is_none());
}

#[test]
fn oversized_table_falls_back_instead_of_truncating_a_row() {
    let markdown = format!(
        "# Guide\n\n| Name | Value |\n| --- | --- |\n| Task | {} |\n",
        "Ω".repeat(800)
    );
    let fixture = fixture(&[("guide.md", &markdown)]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| markdown[chunk.span.start..chunk.span.end].contains("Task"))
        .unwrap();
    let control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &control);
    let source = cache.load(&chunk.revision_id, &fixture.generation).unwrap();
    assert!(context(source, chunk, Some(1500)).unwrap().1.is_none());
}

#[test]
fn bounded_context_keeps_table_headers_and_respects_byte_cap() {
    let markdown = format!(
        "# Guide\n\n{}\n\n| Name | Value |\n| --- | --- |\n| Task | Run |\n\n{}\n",
        "Before ".repeat(240),
        "After ".repeat(300)
    );
    let fixture = fixture(&[("guide.md", &markdown)]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| markdown[chunk.span.start..chunk.span.end].contains("Task"))
        .unwrap();
    let control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &control);
    let source = cache.load(&chunk.revision_id, &fixture.generation).unwrap();
    let (_, text) = context(source, chunk, Some(1500)).unwrap();
    let text = text.unwrap();
    assert!(text.contains("| Name | Value |\n| --- | --- |\n| Task | Run |"));
    assert!(text.len() <= 1500);
    assert!(!text.contains("Before"));
    assert!(!text.contains("After"));
}

/// Returns the context of a chunk narrowed to the whole block `seed`, as a
/// section too large for one chunk splits its introduction from its steps.
fn context_of(markdown: &str, seed: &str, max_bytes: usize) -> (String, Option<String>) {
    let fixture = fixture(&[("guide.md", markdown)]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let start = markdown.find(seed).unwrap();
    let mut chunk = chunks
        .into_iter()
        .find(|chunk| chunk.span.start <= start && start < chunk.span.end)
        .unwrap();
    chunk.span = Span {
        start,
        end: start + seed.len(),
    };
    let control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &control);
    let source = cache.load(&chunk.revision_id, &fixture.generation).unwrap();
    context(source, &chunk, Some(max_bytes)).unwrap()
}

#[test]
fn an_oversized_section_adds_the_nearest_successor_before_the_second_predecessor() {
    let mut steps = String::new();
    for step in 1..=20 {
        writeln!(steps, "{step}. Press the run button now.").unwrap();
    }
    let markdown = format!(
        "# Guide\n\n## Running\n\n{}\n\n{}\n\nStart here.\n\n{steps}",
        "Far ".repeat(150).trim_end(),
        "Near ".repeat(20).trim_end(),
    );
    let text = context_of(&markdown, "Start here.", 1000).1.unwrap();
    assert!(text.contains("Near Near\n\nStart here.\n\n1. Press the run button now."));
    assert!(text.contains("20. Press the run button now."));
    assert!(!text.contains("Far"));
}

#[test]
fn a_whole_section_that_fits_exactly_is_kept_whole() {
    let markdown = concat!(
        "# Guide\n\n## Running\n\nStart here.\n\nFirst step.\n\n",
        "Second step.\n\nThird step.\n\n## Other\n\nUnrelated.\n"
    );
    let whole = context_of(markdown, "Start here.", 1500).1.unwrap();
    assert!(whole.contains("Third step."));
    let exact = context_of(markdown, "Start here.", whole.len()).1.unwrap();
    assert_eq!(exact, whole);
}

#[test]
fn windows_keep_neighbors_and_the_mandatory_unit_that_fit_exactly() {
    let markdown = format!(
        "# Guide\n\n## Running\n\nStart here.\n\nFirst step.\n\n{}\n",
        "Tail ".repeat(320).trim_end()
    );
    let with_step = context_of(&markdown, "Start here.", 1500).1.unwrap();
    assert!(with_step.contains("Start here.\n\nFirst step."));
    assert!(!with_step.contains("Tail"));
    let exact = context_of(&markdown, "Start here.", with_step.len()).1;
    assert_eq!(exact.unwrap(), with_step);
    let without_step = context_of(&markdown, "Start here.", with_step.len() - 1)
        .1
        .unwrap();
    assert!(without_step.contains("## Running\n\nStart here."));
    assert!(!without_step.contains("First step."));
    let exact = context_of(&markdown, "Start here.", without_step.len()).1;
    assert_eq!(exact.unwrap(), without_step);
    let mandatory = context_of(&markdown, "Start here.", without_step.len() - 1)
        .1
        .unwrap();
    assert!(mandatory.contains("Start here."));
    assert!(!mandatory.contains("## Running"));
    let exact = context_of(&markdown, "Start here.", mandatory.len()).1;
    assert_eq!(exact.unwrap(), mandatory);
}
