use super::support::{control, fixture};
use crate::{
    prepare::tests::scratch::{fail_revision, quarantine_revision, revision_of},
    search::evidence::{source::SourceCache, types::EvidenceError},
};
use maestro_kernel::evidence::Span;

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
