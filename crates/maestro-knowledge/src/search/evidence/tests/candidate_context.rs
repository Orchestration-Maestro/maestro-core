use super::support::{control, fixture, fixture_under};
use crate::search::{
    CandidateContext, SearchConfiguration,
    candidate_enrichment::{Settings, enrich},
    evidence::{
        candidate_context::{Indexing, context},
        source::SourceCache,
    },
};
use maestro_canonicalization::ChunkProfile;
use maestro_kernel::evidence::Span;
use std::{
    fmt::Write as _,
    time::{Duration, Instant},
};

/// The default profile's indexing: nothing left out.
const STRUCTURAL: Indexing<'static> = Indexing {
    profile: ChunkProfile::Structural,
    chrome: &[],
};

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
    let (path, text) = context(source, chunk, Some(1500), STRUCTURAL).unwrap();
    let text = text.unwrap();
    assert!(path.contains("Running"));
    assert!(text.contains("Start here."));
    assert!(text.contains("1. Select a task.\n2. Press Run."));
    assert!(!text.contains("Unrelated"));
    assert!(text.len() <= 1500);
    assert_eq!(chunk, &original);
    assert!(
        context(source, chunk, None, STRUCTURAL)
            .unwrap()
            .1
            .is_none()
    );
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
    assert!(
        context(source, chunk, Some(1500), STRUCTURAL)
            .unwrap()
            .1
            .is_none()
    );
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
    let (_, text) = context(source, chunk, Some(1500), STRUCTURAL).unwrap();
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
    context(source, &chunk, Some(max_bytes), STRUCTURAL).unwrap()
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

/// The bounded rerank context of the chunk of a guide with page chrome that
/// holds its introduction, when the guide is prepared under `profile`.
fn chrome_context(profile: ChunkProfile) -> String {
    let markdown = concat!(
        "# Guide\n\n## Running Link copied to clipboard\n\n",
        "Start here. Copy Copied to clipboard\n\nClosed\n\n<!-- image -->\n\n",
        "1. Select a task.\n2. Press Run.\n\n## Other\n\nUnrelated.\n"
    );
    let fixture = fixture_under(&[("guide.md", markdown)], profile);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let chunk = chunks
        .iter()
        .find(|chunk| markdown[chunk.span.start..chunk.span.end].contains("Start here"))
        .unwrap();
    let settings = Settings {
        configuration: SearchConfiguration {
            candidate_context: CandidateContext::BoundedSection { max_bytes: 1500 },
            ..SearchConfiguration::default()
        },
        query: "run a task",
        generation: &fixture.generation,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let mut candidates = [(chunk, "indexed".to_owned())];
    let database = (&fixture.database, &fixture.scopes, &control());
    let enriched = enrich(database, &settings, &mut candidates);
    assert!(enriched.fallbacks.is_empty(), "{:?}", enriched.fallbacks);
    candidates[0].1.clone()
}

#[test]
fn bounded_context_of_a_complete_ideas_set_leaves_its_page_chrome_out() {
    let text = chrome_context(ChunkProfile::CompleteIdeas);
    assert!(
        text.starts_with("Guide / Running\n## Running\n\nStart here."),
        "{text}"
    );
    assert!(text.contains("1. Select a task.\n2. Press Run."), "{text}");
    for chrome in ["clipboard", "Closed", "<!-- image -->"] {
        assert!(!text.contains(chrome), "{chrome}: {text}");
    }
    // The default profile indexed the chrome, so its context keeps it.
    let text = chrome_context(ChunkProfile::Structural);
    for chrome in [
        "Link copied to clipboard",
        "Copy Copied to clipboard",
        "Closed",
        "<!-- image -->",
    ] {
        assert!(text.contains(chrome), "{chrome}: {text}");
    }
}
