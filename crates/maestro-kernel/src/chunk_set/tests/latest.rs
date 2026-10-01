//! The default set for publication is the latest complete set of a chunk
//! profile, ignoring building and failed preparations and other profiles.

use super::support::{Scratch, new_set};
use crate::{chunk_set::NewChunkSet, scope::ScopeSet};

#[test]
fn latest_complete_chunk_set_ignores_newer_incomplete_sets() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let complete = |id: &str| {
        database.begin_chunk_set(&new_set(id, "ctm")).unwrap();
        let manifest = database.put(b"{}", "application/json").unwrap();
        database.complete_chunk_set(id, &manifest).unwrap();
    };
    complete("complete-1");
    database.begin_chunk_set(&new_set("failed", "ctm")).unwrap();
    database.fail_chunk_set("failed").unwrap();
    database
        .begin_chunk_set(&new_set("building", "ctm"))
        .unwrap();
    complete("complete-2");
    let other = NewChunkSet {
        chunk_profile: "mapped-structural-chunks/3",
        ..new_set("other-profile", "ctm")
    };
    database.begin_chunk_set(&other).unwrap();
    let manifest = database.put(b"{}", "application/json").unwrap();
    database
        .complete_chunk_set("other-profile", &manifest)
        .unwrap();

    let latest = |profile| {
        database
            .latest_complete_chunk_set(&ScopeSet::default_workspace(), "ctm", profile)
            .unwrap()
            .map(|set| set.id)
    };
    assert_eq!(
        latest("mapped-structural-chunks/2").as_deref(),
        Some("complete-2")
    );
    assert_eq!(
        latest("mapped-structural-chunks/3").as_deref(),
        Some("other-profile")
    );
    assert_eq!(latest("mapped-structural-chunks/4"), None);
}
