use super::support::{control, fixture};
use crate::{
    prepare::tests::scratch::{corrupt_artifact, fail_revision, quarantine_revision, revision_of},
    search::evidence::{source::SourceCache, types::EvidenceError},
};
use maestro_kernel::{evidence::Span, retrieval::ReadControl};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn reads_authoritative_original_and_canonical_once_per_revision() {
    let markdown = "# Guide\n\nThe original Ω source.\n";
    let fixture = fixture(&[("guide.md", markdown)]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    let source = cache.load(&revision, &fixture.generation).unwrap();
    assert_eq!(source.markdown, markdown);
    assert!(
        SourceCache::validate_chunk_span(
            source,
            Span {
                start: 0,
                end: markdown.len(),
            }
        )
        .is_ok()
    );
    let omega = markdown.find('Ω').unwrap();
    assert!(matches!(
        SourceCache::validate_chunk_span(
            source,
            Span {
                start: omega + 1,
                end: omega + 2,
            }
        ),
        Err(EvidenceError::Integrity(_))
    ));
    assert!(matches!(
        SourceCache::validate_chunk_span(source, Span { start: 0, end: 0 }),
        Err(EvidenceError::Integrity(_))
    ));
    assert_eq!(
        cache.load_counts(&revision),
        Some((1, 1)),
        "one canonical and one original artifact read"
    );

    let source = cache.load(&revision, &fixture.generation).unwrap();
    assert_eq!(source.markdown, markdown);
    assert_eq!(cache.load_counts(&revision), Some((1, 1)));
}

#[test]
fn refuses_a_different_collection_before_loading_source_artifacts() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let mut other_generation = fixture.generation.clone();
    other_generation.collection_id = "another-collection".to_owned();
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    assert!(matches!(
        cache.load(&revision, &other_generation),
        Err(EvidenceError::Integrity(_))
    ));
    assert_eq!(cache.load_counts(&revision), None);
}

#[test]
fn refuses_a_failed_revision_before_loading_source_artifacts() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    fail_revision(&fixture.scratch, &revision);
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    assert!(matches!(
        cache.load(&revision, &fixture.generation),
        Err(EvidenceError::Integrity(_))
    ));
    assert_eq!(cache.load_counts(&revision), None);
}

#[test]
fn refuses_a_quarantined_revision_before_loading_source_artifacts() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    quarantine_revision(&fixture.scratch, &revision);
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    assert!(matches!(
        cache.load(&revision, &fixture.generation),
        Err(EvidenceError::Integrity(_))
    ));
    assert_eq!(cache.load_counts(&revision), None);
}

#[test]
fn refuses_a_revision_outside_the_permission_snapshot() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let no_scopes = fixture.database.visible("nobody").unwrap();
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &no_scopes, &read_control);

    assert!(matches!(
        cache.load(&revision, &fixture.generation),
        Err(EvidenceError::NotVisible)
    ));
}

#[test]
fn parallel_source_loads_cache_each_unique_authorized_revision_once() {
    let fixture = fixture(&[
        ("first.md", "# First\n\nThe first source.\n"),
        ("second.md", "# Second\n\nThe second source.\n"),
    ]);
    let first = revision_of(&fixture.database, &fixture.scopes, "first.md");
    let second = revision_of(&fixture.database, &fixture.scopes, "second.md");
    let revisions = vec![second.clone(), first.clone(), second.clone()];
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    cache
        .load_many_with_workers(&revisions, &fixture.generation, 4)
        .unwrap();

    assert_eq!(
        cache.get(&first).unwrap().markdown,
        "# First\n\nThe first source.\n"
    );
    assert_eq!(
        cache.get(&second).unwrap().markdown,
        "# Second\n\nThe second source.\n"
    );
    assert_eq!(cache.load_counts(&first), Some((1, 1)));
    assert_eq!(cache.load_counts(&second), Some((1, 1)));
}

#[test]
fn parallel_source_loads_keep_a_revision_the_request_already_verified() {
    let fixture = fixture(&[
        ("first.md", "# First\n\nThe first source.\n"),
        ("second.md", "# Second\n\nThe second source.\n"),
    ]);
    let first = revision_of(&fixture.database, &fixture.scopes, "first.md");
    let second = revision_of(&fixture.database, &fixture.scopes, "second.md");
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);
    cache.load(&first, &fixture.generation).unwrap();
    fail_revision(&fixture.scratch, &first);

    cache
        .load_many_with_workers(&[first.clone(), second.clone()], &fixture.generation, 4)
        .unwrap();

    assert_eq!(
        cache.get(&first).unwrap().markdown,
        "# First\n\nThe first source.\n"
    );
    assert_eq!(cache.load_counts(&second), Some((1, 1)));
}

#[test]
fn parallel_source_load_errors_follow_candidate_order_not_completion_order() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let first = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let digest = fixture
        .database
        .revision(&fixture.scopes, &first)
        .unwrap()
        .unwrap()
        .original_digest;
    corrupt_artifact(&fixture.scratch, &digest);
    let revisions = vec![first, "missing-revision".to_owned()];
    let read_control = control();
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    assert!(matches!(
        cache.load_many_with_workers(&revisions, &fixture.generation, 4),
        Err(EvidenceError::Store(_))
    ));
}

#[test]
fn source_cache_refuses_an_already_cancelled_read() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let read_control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(1),
        cancelled: Arc::new(AtomicBool::new(true)),
    };
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    assert!(matches!(
        cache.load(&revision, &fixture.generation),
        Err(EvidenceError::TimedOut)
    ));
}

#[test]
fn parallel_source_loads_refuse_expiry_during_a_batch() {
    let documents: Vec<_> = (0..16)
        .map(|index| {
            (
                format!("doc-{index}.md"),
                format!("# Guide {index}\n\nSource {index} with enough text to prepare.\n"),
            )
        })
        .collect();
    let refs: Vec<_> = documents
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let fixture = fixture(&refs);
    let revisions: Vec<_> = documents
        .iter()
        .map(|(path, _)| revision_of(&fixture.database, &fixture.scopes, path))
        .collect();
    let deadline = Instant::now() + Duration::from_millis(5);
    let read_control = ReadControl {
        deadline,
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &read_control);

    let result = cache.load_many_with_workers(&revisions, &fixture.generation, 4);

    assert!(
        Instant::now() >= deadline,
        "deadline did not expire mid-batch"
    );
    assert!(matches!(result, Err(EvidenceError::TimedOut)));
    assert!(
        revisions
            .iter()
            .all(|revision| cache.get(revision).is_none())
    );
}
