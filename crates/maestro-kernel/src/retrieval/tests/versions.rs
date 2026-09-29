//! Exact version existence in a pinned, scoped generation.

use super::support::SearchDb;
use crate::{
    document::RevisionStatus,
    retrieval::{ReadControl, SearchMember, SearchRead, SystemClock},
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn version_exists_matches_only_the_exact_pinned_revision_version() {
    let search = SearchDb::new("Install the tool with --force.");
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = |version| SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version,
        control: &control,
    };

    assert!(search.database.version_exists(&read(Some("1.2"))).unwrap());
    assert!(!search.database.version_exists(&read(Some("9.9"))).unwrap());
    assert!(search.database.version_exists(&read(None)).unwrap());
}

#[test]
fn version_exists_excludes_out_of_scope_and_ineligible_chunk_owners() {
    let search = SearchDb::new("Install the tool with --force.");
    search.ready();
    let control = control();
    let no_scopes = search.database.visible("ungranted-reader").unwrap();
    let out_of_scope = SearchRead {
        generation: &search.generation,
        scopes: &no_scopes,
        version: Some("1.2"),
        control: &control,
    };
    assert!(!search.database.version_exists(&out_of_scope).unwrap());

    for (status, disposition) in [
        (RevisionStatus::Failed, "accepted"),
        (RevisionStatus::Valid, "quarantined"),
    ] {
        let ineligible =
            SearchDb::with_eligibility("Install the tool with --force.", status, disposition);
        ineligible.ready();
        let read = SearchRead {
            generation: &ineligible.generation,
            scopes: &ineligible.scopes,
            version: Some("1.2"),
            control: &control,
        };
        assert!(
            !ineligible.database.version_exists(&read).unwrap(),
            "{status:?} / {disposition} revisions are not documented versions"
        );
    }
}

#[test]
fn version_exists_ignores_a_version_only_carried_by_a_duplicate_member() {
    let search = SearchDb::new("Install the tool with --force.");
    search.add_duplicate_document();
    search.ready_with_members(&[
        SearchMember {
            revision_id: "rev-a".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        },
        SearchMember {
            revision_id: "rev-b".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        },
    ]);
    let control = control();
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: Some("2.0"),
        control: &control,
    };
    assert!(!search.database.version_exists(&read).unwrap());
}

fn control() -> ReadControl {
    ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
