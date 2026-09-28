//! Profiles: the pinned default batch and what each profile names.
use super::*;
use crate::chunk_mapping::map_document;
use crate::chunk_profile::ChromeRule;
use crate::chunk_split::build_drafts;
use crate::hashing::digest;
use std::fmt::Write as _;
use std::iter;

/// A page shaped like exported product help: a copy-link label after each heading, an
/// introduction and its steps in separate blocks, a collapsed-image label and an image
/// placeholder, a copy-button label before code, and a parameter table after "Where:".
pub(super) const HELP_PAGE: &str = "\
# Service Tasks

The following sections describe tasks on services.

## Restarting a Service Link copied to clipboard

This procedure describes how to restart a service.

Begin

1. From the **Services** view, select the service.
2. Do one of the following:
   - To restart now, click **Restart**.
   - To restart later, click **Schedule**.

Closed

<!-- image -->

## Restart Settings Link copied to clipboard

The following example shows the restart settings of a service.

Copy Copied to clipboard

```
{\"Restart\": {\"Delay\": \"5\"}}
```

Where:

| Parameter | Description |
|---|---|
| Delay | Minutes to wait before the restart. |
";

/// A one-document batch of `markdown` under `profile`, serialized, and the batch.
fn batch<'a>(
    scope: &'a DedupScope,
    document: &'a CanonicalDocument,
    markdown: &'a str,
    profile: ChunkProfile,
) -> ChunkBatch<'a> {
    chunk_documents(
        scope,
        &[DedupInput { document, markdown }],
        WarningPolicy::Preserve,
        profile,
        &TestCounter::new("test/unqualified"),
    )
    .unwrap()
}

/// The default profile's batch of the help page, serialized.
fn default_batch_bytes() -> Vec<u8> {
    let document = canonicalize(CanonicalizeInput::new(HELP_PAGE, "help-page")).unwrap();
    let scope = scope(&[&document]);
    let batch = batch(&scope, &document, HELP_PAGE, ChunkProfile::default());
    serde_json::to_vec(&batch).unwrap()
}

#[test]
fn the_default_profile_batch_stays_byte_for_byte_what_it_was() {
    // Pinned before the second profile existed: the published generation's chunks stay
    // reproducible, identities included.
    assert_eq!(
        digest(&default_batch_bytes()),
        "d11ba4a8d2cd4eaaa7c2db2858d27ac2caf100be9c3f23c3be5c396f5a7daf6a"
    );
}

/// Pages shaped like the complete-ideas cases: steps under an introduction, a caption and code
/// before a table, two tables in one section, content that looks like chrome but is not, a
/// trailing copy label, accents and identifiers, an oversized step, and a small chunk after a
/// large one.
fn ideas_pages() -> Vec<String> {
    let mut substeps = String::new();
    for sub in 0..6 {
        writeln!(substeps, "   - Sub {sub} {}", "s".repeat(150)).unwrap();
    }
    vec![
        "# Restarting\n\nThis restarts a service.\n\nBegin\n\n\
         1. Select it.\n2. Click **Restart**.\n"
            .to_owned(),
        "# Delay\n\nThe example sets a delay.\n\n```\n{\"Delay\": \"5\"}\n```\n\nWhere:\n\n\
         | Parameter | Description |\n|---|---|\n| Delay | Minutes to wait. |\n"
            .to_owned(),
        "# Keys\n\n| Key | Value |\n|---|---|\n| one | 1 |\n\n\
         | Key | Value |\n|---|---|\n| two | 2 |\n"
            .to_owned(),
        "# Variables\n\n<setting name=\"a\" value=\"b\"/>\n\n<name>\n\n- Open\n\n- Closed\n"
            .to_owned(),
        "# Example\n\nRun it. Copy Copied to clipboard\n\n\
         | Code |\n|---|\n| x Copy Copied to clipboard |\n"
            .to_owned(),
        "# Relay-X 4.2.17\n\nRéglages de relay_x : exécuter `relay-x send batch`.\n".to_owned(),
        format!("{}\n\n1. Parent step.\n{substeps}", "a".repeat(100)),
        format!("{}\n\nSee the guide for more.\n", "a".repeat(490)),
    ]
}

/// The complete-ideas batch of the help page and of the ideas pages, serialized.
fn complete_ideas_batch_bytes() -> Vec<u8> {
    let markdowns = iter::once(HELP_PAGE.to_owned()).chain(ideas_pages());
    let pages: Vec<(String, CanonicalDocument)> = markdowns
        .enumerate()
        .map(|(index, markdown)| {
            let id = format!("ideas-page-{index}");
            let document = canonicalize(CanonicalizeInput::new(&markdown, &id)).unwrap();
            (markdown, document)
        })
        .collect();
    let documents: Vec<_> = pages.iter().map(|(_, document)| document).collect();
    let scope = scope(&documents);
    let inputs: Vec<_> = pages
        .iter()
        .map(|(markdown, document)| DedupInput { document, markdown })
        .collect();
    let batch = chunk_documents(
        &scope,
        &inputs,
        WarningPolicy::Preserve,
        ChunkProfile::CompleteIdeas,
        &TestCounter::new("test/unqualified"),
    )
    .unwrap();
    serde_json::to_vec(&batch).unwrap()
}

#[test]
fn the_complete_ideas_batch_stays_byte_for_byte_what_it_is() {
    // A change here is a new profile, mapped-structural-chunks/4, never a new pin.
    assert_eq!(
        digest(&complete_ideas_batch_bytes()),
        "becd88a69fe720f1713d28eac576cd520e8307142524957227702d89cda9891f"
    );
}

#[test]
fn complete_ideas_packs_the_help_page_into_one_chunk_per_section() {
    let document = canonicalize(CanonicalizeInput::new(HELP_PAGE, "help-page")).unwrap();
    let scope = scope(&[&document]);
    let ideas = batch(&scope, &document, HELP_PAGE, ChunkProfile::CompleteIdeas);
    let texts: Vec<_> = ideas
        .chunks
        .iter()
        .map(|chunk| chunk.content.prepared_input.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "Service Tasks\n\nThe following sections describe tasks on services.",
            "Service Tasks\n\nRestarting a Service\n\nThis procedure describes how to restart a \
             service.\n\nBegin\n1. From the Services view, select the service.\n2. Do one of the \
             following:\n  - To restart now, click Restart.\n  - To restart later, click Schedule.",
            "Service Tasks\n\nRestart Settings\n\nThe following example shows the restart settings \
             of a service.\n\n{\"Restart\": {\"Delay\": \"5\"}}\n\n\nWhere:\n\n\
             Parameter\tDescription\nDelay\tMinutes to wait before the restart.",
        ]
    );
    let structural = batch(&scope, &document, HELP_PAGE, ChunkProfile::Structural);
    assert_eq!(structural.chunks.len(), 10);
}

#[test]
fn each_profile_names_itself_in_the_batch_and_in_every_identity() {
    let document = canonicalize(CanonicalizeInput::new(HELP_PAGE, "help-page")).unwrap();
    let scope = scope(&[&document]);
    let structural = batch(&scope, &document, HELP_PAGE, ChunkProfile::Structural);
    let ideas = batch(&scope, &document, HELP_PAGE, ChunkProfile::CompleteIdeas);
    assert_eq!(
        (
            structural.version.as_str(),
            structural.preparation_profile.as_str()
        ),
        ("mapped-structural-chunks/2", "canonical-context-parts/v1")
    );
    assert_eq!(
        (ideas.version.as_str(), ideas.preparation_profile.as_str()),
        ("mapped-structural-chunks/3", "canonical-context-parts/v2")
    );
    // The first chunk has the same text under both profiles, yet its identities differ.
    assert_eq!(
        structural.chunks[0].content.prepared_input,
        ideas.chunks[0].content.prepared_input
    );
    assert_ne!(structural.chunks[0].chunk_id, ideas.chunks[0].chunk_id);
    assert_ne!(
        structural.chunks[0].retrieval_input_fingerprint,
        ideas.chunks[0].retrieval_input_fingerprint
    );
    assert_ne!(
        structural.prepared_groups[0].group_id,
        ideas.prepared_groups[0].group_id
    );
    for profile in ChunkProfile::ALL {
        assert_eq!(
            ChunkProfile::named(profile.chunker_version()),
            Some(profile)
        );
    }
    assert_eq!(ChunkProfile::named("canonical-context-parts/v2"), None);
    assert_eq!(ChunkProfile::default(), ChunkProfile::Structural);
}

#[test]
fn complete_ideas_records_the_chrome_it_leaves_out_of_its_coverage_by_rule() {
    let markdown = "# Restart Link copied to clipboard\n\nClosed\n\n<!-- image -->\n\n\
                    Restart it. Copy Copied to clipboard\n\nCopy Copied to clipboard\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "chrome")).unwrap();
    let scope = scope(&[&document]);
    let ideas = batch(&scope, &document, markdown, ChunkProfile::CompleteIdeas);
    let coverage = &ideas.documents[0].coverage;
    let mapped = &ideas.documents[0].mapped;
    let left_out: Vec<_> = coverage
        .iter()
        .flat_map(|unit| {
            let text = &mapped.units[unit.unit_index].text;
            unit.chrome_ranges
                .iter()
                .map(move |range| (&text[range.start..range.end], unit.chrome_rule))
        })
        .collect();
    assert_eq!(
        left_out,
        [
            (" Link copied to clipboard", Some(ChromeRule::HeadingSuffix)),
            ("Closed", Some(ChromeRule::LabelBeforeImage)),
            ("<!-- image -->\n", Some(ChromeRule::Markup)),
            (
                " Copy Copied to clipboard",
                Some(ChromeRule::ParagraphSuffix)
            ),
            ("Copy Copied to clipboard", Some(ChromeRule::Label)),
        ]
    );
    assert_eq!(
        ideas.chunks[0].content.prepared_input,
        "Restart\n\nRestart it."
    );
    let counts = ideas.documents[0].chrome();
    let count = |units, bytes| ChromeCount { units, bytes };
    assert_eq!(
        counts.into_iter().collect::<Vec<_>>(),
        [
            (ChromeRule::HeadingSuffix, count(1, 25)),
            (ChromeRule::ParagraphSuffix, count(1, 25)),
            (ChromeRule::Label, count(1, 24)),
            (ChromeRule::LabelBeforeImage, count(1, 6)),
            (ChromeRule::Markup, count(1, 15)),
        ]
    );
    assert_eq!(
        left_out_chrome(&document, markdown, ChunkProfile::CompleteIdeas).unwrap(),
        ideas.documents[0].chrome()
    );
    let json = serde_json::to_string(&ideas.documents[0]).unwrap();
    assert!(
        json.contains("\"chrome_rule\":\"label_before_image\""),
        "{json}"
    );
    let structural = batch(&scope, &document, markdown, ChunkProfile::Structural);
    assert!(
        structural.documents[0]
            .coverage
            .iter()
            .all(|unit| { unit.chrome_ranges.is_empty() && unit.chrome_rule.is_none() })
    );
    assert!(structural.documents[0].chrome().is_empty());
    assert!(
        left_out_chrome(&document, markdown, ChunkProfile::Structural)
            .unwrap()
            .is_empty()
    );
    let json = serde_json::to_string(&structural.documents[0]).unwrap();
    assert!(!json.contains("chrome_"));
}

#[test]
fn a_page_of_chrome_alone_has_no_searchable_content_under_complete_ideas() {
    let markdown = "Closed\n\n<!-- image -->\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "chrome-only")).unwrap();
    let scope = scope(&[&document]);
    let ideas = batch(&scope, &document, markdown, ChunkProfile::CompleteIdeas);
    assert!(ideas.chunks.is_empty());
    assert!(ideas.documents[0].no_searchable_content);
    let structural = batch(&scope, &document, markdown, ChunkProfile::Structural);
    assert!(!structural.chunks.is_empty());
    assert!(!structural.documents[0].no_searchable_content);
}

#[test]
fn coverage_refuses_chrome_in_a_chunk_and_kept_text_left_out() {
    let markdown = "# Restart Link copied to clipboard\n\nRestart it.\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "chrome")).unwrap();
    let mapped = map_document(&document, markdown).unwrap();
    let layout = Layout::new(&document, markdown, &mapped, ChunkProfile::CompleteIdeas).unwrap();
    let drafts = build_drafts(&layout, &mut |input| Ok(fake_count(input))).unwrap();
    assert!(validate_coverage(&layout, &drafts).is_ok());
    let heading = &mapped.units[drafts[0].fragments[0].contribution.unit_index];
    let mut chrome = drafts.clone();
    chrome[0].fragments[0].contribution.range.end = heading.text.len();
    assert!(validate_coverage(&layout, &chrome).is_err());
    let mut short = drafts.clone();
    short[0].fragments[0].contribution.range.end -= 1;
    assert!(validate_coverage(&layout, &short).is_err());
}

#[test]
fn chrome_spans_name_the_original_bytes_complete_ideas_leaves_out() {
    let markdown = "# Restart Link copied to clipboard\n\nClosed\n\n<!-- image -->\n\n\
                    Restart it. Copy Copied to clipboard\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "chrome")).unwrap();
    let spans = chrome_spans(&document, markdown, ChunkProfile::CompleteIdeas).unwrap();
    let texts: Vec<_> = spans
        .iter()
        .map(|span| &markdown[span.start..span.end])
        .collect();
    assert_eq!(
        texts,
        [
            " Link copied to clipboard",
            "Closed",
            "<!-- image -->\n",
            " Copy Copied to clipboard"
        ]
    );
    assert_eq!(
        chrome_spans(&document, markdown, ChunkProfile::Structural).unwrap(),
        []
    );
    // A profile without chrome rules reads nothing, not even a mismatched original.
    assert_eq!(
        chrome_spans(&document, "other", ChunkProfile::Structural).unwrap(),
        []
    );
    assert!(chrome_spans(&document, "other", ChunkProfile::CompleteIdeas).is_err());
    let title = "Restart Link copied to clipboard";
    assert_eq!(indexed_title(ChunkProfile::CompleteIdeas, title), "Restart");
    assert_eq!(indexed_title(ChunkProfile::Structural, title), title);
}
