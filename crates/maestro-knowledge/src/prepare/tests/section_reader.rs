//! The public section reader returns exact spans; these tests reuse prepare's scratch fixtures.

use super::scratch::{
    Scratch, chunk_set_of, corrupt_artifact, decide_all, drop_chunk_sets_table, fail_revision,
    quarantine_revision, remove_artifact, replace_revision_canonical, revision_of,
    set_chunk_sections, tokenizer,
};
use crate::{
    prepare::{Report, prepare},
    search::evidence::{SectionReadError, read_section},
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    artifact::Digest,
    chunk_set::NewChunkSet,
    document::Outcome,
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use std::{error::Error as StdError, io::Error as IoError};

const NESTED: &str = concat!(
    "# Guide\n\nIntro.\n\n## Parent\n\nParent body.\n\n",
    "### Child\n\nNested Ω text.\n\n## Sibling\n\nOutside.\n",
);

fn prepared(scratch: &Scratch, documents: &[(&str, &str)]) -> (Database, ScopeSet, Report) {
    scratch.corpus(documents);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, counter) = tokenizer();
    let report = prepare(&database, &scopes, "notes", &counter).unwrap();
    (database, scopes, report)
}

fn canonical_of(database: &Database, scopes: &ScopeSet, path: &str) -> CanonicalDocument {
    let revision_id = revision_of(database, scopes, path);
    let revision = database.revision(scopes, &revision_id).unwrap().unwrap();
    serde_json::from_slice(&database.get(&revision.canonical_digest).unwrap()).unwrap()
}

fn section_id(document: &CanonicalDocument, title: &str) -> String {
    document
        .sections
        .iter()
        .find(|section| section.title == title)
        .unwrap()
        .section_id
        .clone()
}

#[test]
fn returns_the_nested_non_ascii_section_with_original_byte_offsets() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let start = NESTED.find("## Parent").unwrap();
    let end = NESTED.find("## Sibling").unwrap();
    let expected = &NESTED[start..end];

    let excerpt = read_section(
        &database,
        &scopes,
        &report.chunk_set,
        &parent_id,
        expected.len(),
    )
    .unwrap();

    let document = database
        .document(&scopes, &excerpt.document_id)
        .unwrap()
        .unwrap();
    let revision = database
        .revision(&scopes, &revision_of(&database, &scopes, "guide.md"))
        .unwrap()
        .unwrap();
    assert_eq!(excerpt.document_id, document.id);
    assert_eq!(excerpt.revision_id, revision.id);
    assert_eq!(excerpt.section_id, parent_id);
    assert_eq!(excerpt.source_ref, "https://example.org/guide.md");
    assert_eq!(excerpt.title, "Notes");
    assert_eq!(excerpt.section_path, ["Guide", "Parent"]);
    assert_eq!(excerpt.version, None);
    assert_eq!(excerpt.span, [start, end]);
    assert_eq!(excerpt.digest, Digest::of(expected.as_bytes()));
    assert_eq!(excerpt.text, expected);
}

#[test]
fn returns_a_section_to_its_source_only_grant() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let source: Scope = "workspace/default/collection/notes/source/docs"
        .parse()
        .unwrap();
    database
        .grant("source-reader", &source, Right::Read, "test")
        .unwrap();
    let source_scopes = database.visible("source-reader").unwrap();
    let start = NESTED.find("## Parent").unwrap();
    let end = NESTED.find("## Sibling").unwrap();

    let excerpt = read_section(
        &database,
        &source_scopes,
        &report.chunk_set,
        &parent_id,
        end - start,
    )
    .unwrap();

    assert_eq!(excerpt.text, &NESTED[start..end]);
}

#[test]
fn refuses_missing_and_out_of_scope_sections_without_distinguishing_them() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let no_scopes = database.visible("nobody").unwrap();

    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            "missing-section",
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
    assert!(matches!(
        read_section(
            &database,
            &no_scopes,
            &report.chunk_set,
            &parent_id,
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
}

#[test]
fn reports_the_complete_extent_size_before_returning_an_over_limit_section() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let start = NESTED.find("## Parent").unwrap();
    let end = NESTED.find("## Sibling").unwrap();

    assert!(matches!(
        read_section(&database, &scopes, &report.chunk_set, &parent_id, end - start - 1),
        Err(SectionReadError::TooLarge { bytes }) if bytes == end - start
    ));
}

#[test]
fn returns_a_nested_child_section_only_through_the_next_parent_sibling() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let child_id = section_id(&canonical, "Child");
    let start = NESTED.find("### Child").unwrap();
    let end = NESTED.find("## Sibling").unwrap();

    let excerpt = read_section(
        &database,
        &scopes,
        &report.chunk_set,
        &child_id,
        end - start,
    )
    .unwrap();

    assert_eq!(excerpt.span, [start, end]);
    assert_eq!(excerpt.text, &NESTED[start..end]);
    assert_eq!(excerpt.section_path, ["Guide", "Parent", "Child"]);
}

#[test]
fn includes_the_last_section_through_its_final_multibyte_byte() {
    let markdown = "# Guide\n\n## Last\n\nEnds Ω";
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("last.md", markdown)]);
    let canonical = canonical_of(&database, &scopes, "last.md");
    let section_id = section_id(&canonical, "Last");
    let start = markdown.find("## Last").unwrap();
    let expected = &markdown[start..];

    let excerpt = read_section(
        &database,
        &scopes,
        &report.chunk_set,
        &section_id,
        expected.len(),
    )
    .unwrap();

    assert_eq!(excerpt.span, [start, markdown.len()]);
    assert_eq!(excerpt.text, expected);
}

#[test]
fn refuses_a_section_id_that_occurs_in_more_than_one_document() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(
        &scratch,
        &[
            ("first.md", "# First\n\n## Shared\n\nFirst body.\n"),
            ("second.md", "# Second\n\n## Shared\n\nSecond body.\n"),
        ],
    );
    set_chunk_sections(&scratch, &report.chunk_set, "shared-section");

    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            "shared-section",
            usize::MAX
        ),
        Err(SectionReadError::Ambiguous)
    ));
}

#[test]
fn maps_a_damaged_canonical_artifact_to_integrity() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let revision_id = revision_of(&database, &scopes, "guide.md");
    let revision = database.revision(&scopes, &revision_id).unwrap().unwrap();
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    corrupt_artifact(&scratch, &revision.canonical_digest);

    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            &parent_id,
            usize::MAX
        ),
        Err(SectionReadError::Integrity)
    ));
}

#[test]
fn decides_too_large_before_reading_a_missing_original_artifact() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let revision_id = revision_of(&database, &scopes, "guide.md");
    let revision = database.revision(&scopes, &revision_id).unwrap().unwrap();
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let start = NESTED.find("## Parent").unwrap();
    let end = NESTED.find("## Sibling").unwrap();
    remove_artifact(&scratch, &revision.original_digest);

    assert!(matches!(
        read_section(&database, &scopes, &report.chunk_set, &parent_id, end - start - 1),
        Err(SectionReadError::TooLarge { bytes }) if bytes == end - start
    ));
    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            &parent_id,
            usize::MAX
        ),
        Err(SectionReadError::Integrity)
    ));
}

#[test]
fn building_and_failed_chunk_sets_are_not_found() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let complete = chunk_set_of(&database, &scopes, &report);

    for id in ["building-set", "failed-set"] {
        database
            .begin_chunk_set(&NewChunkSet {
                id,
                collection_id: &complete.collection_id,
                chunk_profile: &complete.chunk_profile,
                counter_contract_id: &complete.counter_contract_id,
            })
            .unwrap();
        if id == "failed-set" {
            database.fail_chunk_set(id).unwrap();
        }
        assert!(matches!(
            read_section(&database, &scopes, id, &parent_id, usize::MAX),
            Err(SectionReadError::NotFound)
        ));
    }
}

#[test]
fn a_nonaccepted_disposition_is_not_found() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let revision_id = revision_of(&database, &scopes, "guide.md");
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    quarantine_revision(&scratch, &revision_id);

    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            &parent_id,
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
}

#[test]
fn a_failed_revision_is_not_found() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let revision_id = revision_of(&database, &scopes, "guide.md");
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    fail_revision(&scratch, &revision_id);

    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            &parent_id,
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
}

#[test]
fn canonical_document_revision_and_hash_identities_are_checked_separately() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    let revision_id = revision_of(&database, &scopes, "guide.md");
    let canonical = canonical_of(&database, &scopes, "guide.md");
    let parent_id = section_id(&canonical, "Parent");
    let mut bad_documents = Vec::new();

    let mut wrong_document = canonical.clone();
    wrong_document.document_id = "other-document".to_owned();
    bad_documents.push(wrong_document);
    let mut wrong_revision = canonical.clone();
    wrong_revision.revision_id = "other-revision".to_owned();
    bad_documents.push(wrong_revision);
    let mut wrong_hash = canonical.clone();
    wrong_hash.content_hash = "sha256:wrong".to_owned();
    bad_documents.push(wrong_hash);
    let mut wrong_reference = canonical;
    wrong_reference.original_markdown_reference.content_hash = "sha256:wrong".to_owned();
    bad_documents.push(wrong_reference);

    for invalid in bad_documents {
        replace_revision_canonical(
            &scratch,
            &database,
            &revision_id,
            &serde_json::to_vec(&invalid).unwrap(),
        );
        assert!(matches!(
            read_section(
                &database,
                &scopes,
                &report.chunk_set,
                &parent_id,
                usize::MAX
            ),
            Err(SectionReadError::Integrity)
        ));
    }
}

#[test]
fn hidden_callers_cannot_observe_ambiguity_or_corrupt_canonical_data() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(
        &scratch,
        &[
            ("first.md", "# First\n\n## Shared\n\nFirst body.\n"),
            ("second.md", "# Second\n\n## Shared\n\nSecond body.\n"),
        ],
    );
    set_chunk_sections(&scratch, &report.chunk_set, "shared-section");
    let no_scopes = database.visible("nobody").unwrap();

    assert!(matches!(
        read_section(
            &database,
            &no_scopes,
            &report.chunk_set,
            "shared-section",
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
    assert!(matches!(
        read_section(
            &database,
            &scopes,
            &report.chunk_set,
            "shared-section",
            usize::MAX
        ),
        Err(SectionReadError::Ambiguous)
    ));

    let revision_id = revision_of(&database, &scopes, "first.md");
    let revision = database.revision(&scopes, &revision_id).unwrap().unwrap();
    corrupt_artifact(&scratch, &revision.canonical_digest);
    assert!(matches!(
        read_section(
            &database,
            &no_scopes,
            &report.chunk_set,
            "shared-section",
            usize::MAX
        ),
        Err(SectionReadError::NotFound)
    ));
}

#[test]
fn public_errors_redact_store_details_and_preserve_source_chains() {
    for (error, display) in [
        (
            SectionReadError::NotFound,
            "section is unknown or inaccessible",
        ),
        (
            SectionReadError::Ambiguous,
            "section identity is ambiguous in this chunk set",
        ),
        (
            SectionReadError::TooLarge { bytes: 9 },
            "section extent is 9 bytes, above the requested limit",
        ),
        (
            SectionReadError::Integrity,
            "section content failed an integrity check",
        ),
    ] {
        assert_eq!(error.to_string(), display);
        assert!(StdError::source(&error).is_none());
    }

    let error = SectionReadError::Store(Box::new(IoError::other("private SQL body")));
    assert_eq!(
        error.to_string(),
        "kernel store could not read section data"
    );
    assert!(!error.to_string().contains("SQL"));
    assert_eq!(
        StdError::source(&error).unwrap().to_string(),
        "private SQL body"
    );
}

#[test]
fn keeps_the_kernel_cause_for_a_store_failure() {
    let scratch = Scratch::new();
    let (database, scopes, report) = prepared(&scratch, &[("guide.md", NESTED)]);
    drop_chunk_sets_table(&scratch);

    let error = read_section(
        &database,
        &scopes,
        &report.chunk_set,
        "any-section",
        usize::MAX,
    )
    .unwrap_err();
    assert!(matches!(error, SectionReadError::Store(_)));
    assert!(StdError::source(&error).is_some());
}
