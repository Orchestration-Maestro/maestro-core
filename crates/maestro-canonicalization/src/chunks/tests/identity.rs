//! Identities: chunk and prepared-input identities follow scope and content alone.
use super::*;

#[test]
fn equal_inputs_retain_occurrences_policies_and_deterministic_scoped_groups() {
    let markdown = "Keep 9.0.22.\n";
    let (first_document, second_document) = two_access_policies(markdown);
    let before = serde_json::to_vec(&(&first_document, &second_document)).unwrap();
    let scope = scope(&[&first_document, &second_document]);
    let first = DedupInput {
        document: &first_document,
        markdown,
    };
    let second = DedupInput {
        document: &second_document,
        markdown,
    };
    let left = check_order_does_not_change_the_batch(&scope, first, second);
    let subset = check_a_subset_keeps_group_and_chunk_identities(&scope, first, &left);
    check_scope_changes_change_the_identities(&scope, first, &subset);
    assert_eq!(
        before,
        serde_json::to_vec(&(&first_document, &second_document)).unwrap()
    );
}

/// Two documents with the same text and different access policies.
fn two_access_policies(markdown: &str) -> (CanonicalDocument, CanonicalDocument) {
    let mut input = CanonicalizeInput::new(markdown, "a");
    input.metadata.access_policy = Some(serde_json::json!({"role":"a"}));
    let first_document = canonicalize(input).unwrap();
    let mut input = CanonicalizeInput::new(markdown, "b");
    input.metadata.access_policy = Some(serde_json::json!({"role":"b"}));
    let second_document = canonicalize(input).unwrap();
    (first_document, second_document)
}

/// Equal inputs in either order give one batch: two chunks, one prepared
/// group, and both access policies kept.
fn check_order_does_not_change_the_batch<'a>(
    scope: &'a DedupScope,
    first: DedupInput<'a>,
    second: DedupInput<'a>,
) -> ChunkBatch<'a> {
    let left = chunk_with_count(
        scope,
        &[first, second],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    let right = chunk_with_count(
        scope,
        &[second, first],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&left).unwrap(),
        serde_json::to_vec(&right).unwrap()
    );
    assert_eq!(left.chunks.len(), 2);
    assert_ne!(left.chunks[0].chunk_id, left.chunks[1].chunk_id);
    assert_eq!(left.prepared_groups.len(), 1);
    assert_eq!(left.prepared_groups[0].chunk_indices, [0, 1]);
    assert_ne!(
        left.deduplication.occurrences[0].document.access_policy,
        left.deduplication.occurrences[1].document.access_policy
    );
    left
}

/// A batch of one of the inputs keeps the group and chunk identities.
fn check_a_subset_keeps_group_and_chunk_identities<'a>(
    scope: &'a DedupScope,
    first: DedupInput<'a>,
    left: &ChunkBatch<'_>,
) -> ChunkBatch<'a> {
    let subset = chunk_with_count(
        scope,
        &[first],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_eq!(
        subset.prepared_groups[0].group_id,
        left.prepared_groups[0].group_id
    );
    assert!(
        left.chunks
            .iter()
            .any(|chunk| chunk.chunk_id == subset.chunks[0].chunk_id)
    );
    subset
}

/// Another tenant or workspace gives other identities; no authorization
/// refuses the batch.
fn check_scope_changes_change_the_identities(
    scope: &DedupScope,
    first: DedupInput<'_>,
    subset: &ChunkBatch<'_>,
) {
    let mut changed_scope = scope.clone();
    changed_scope.tenant_id = "tenant-b".into();
    let changed = chunk_with_count(
        &changed_scope,
        &[first],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_ne!(
        changed.prepared_groups[0].group_id,
        subset.prepared_groups[0].group_id
    );
    assert_ne!(changed.chunks[0].chunk_id, subset.chunks[0].chunk_id);
    changed_scope = scope.clone();
    changed_scope.workspace_id = "workspace-b".into();
    let changed = chunk_with_count(
        &changed_scope,
        &[first],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_ne!(
        changed.prepared_groups[0].group_id,
        subset.prepared_groups[0].group_id
    );
    changed_scope.authorized_revisions.clear();
    assert!(
        chunk_with_count(
            &changed_scope,
            &[first],
            WarningPolicy::Preserve,
            "test/unqualified",
            &mut |input| Ok(fake_count(input))
        )
        .is_err()
    );
}

#[test]
fn context_revisions_and_counter_profiles_are_not_false_equalities() {
    let first_md = "# First\n\nbody\n";
    let second_md = "# Second\n\nbody\n";
    let first_document = canonicalize(CanonicalizeInput::new(first_md, "same-source")).unwrap();
    let second_document = canonicalize(CanonicalizeInput::new(second_md, "same-source")).unwrap();
    let scope = scope(&[&first_document, &second_document]);
    let first = DedupInput {
        document: &first_document,
        markdown: first_md,
    };
    let second = DedupInput {
        document: &second_document,
        markdown: second_md,
    };
    let result = chunk_with_count(
        &scope,
        &[first, second],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_eq!(result.prepared_groups.len(), 2);
    assert_ne!(result.chunks[0].chunk_id, result.chunks[1].chunk_id);
    let changed = chunk_with_count(
        &scope,
        &[first],
        WarningPolicy::Preserve,
        "test/changed",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert!(
        result
            .chunks
            .iter()
            .all(|chunk| chunk.chunk_id != changed.chunks[0].chunk_id)
    );
    assert!(
        result
            .prepared_groups
            .iter()
            .all(|group| group.group_id != changed.prepared_groups[0].group_id)
    );
}

#[test]
fn operational_metadata_does_not_change_chunk_or_prepared_identity() {
    let markdown = "Stable bytes.\n";
    let original = canonicalize(CanonicalizeInput::new(markdown, "stable")).unwrap();
    let mut rerun = original.clone();
    rerun
        .operational_metadata
        .insert("run_id".into(), serde_json::json!("another-run"));
    let scope = scope(&[&original]);
    let left = chunk_with_count(
        &scope,
        &[DedupInput {
            document: &original,
            markdown,
        }],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    let right = chunk_with_count(
        &scope,
        &[DedupInput {
            document: &rerun,
            markdown,
        }],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert_eq!(left.chunks, right.chunks);
    assert_eq!(left.prepared_groups, right.prepared_groups);
    assert_ne!(
        left.deduplication.occurrences[0]
            .document
            .operational_metadata,
        right.deduplication.occurrences[0]
            .document
            .operational_metadata
    );
}

#[test]
fn equal_candidate_hashes_do_not_merge_different_inputs() {
    let mut groups = PreparedGroups::new();
    insert_prepared_group(
        &mut groups,
        "forced".into(),
        b"first".to_vec(),
        "group".into(),
        0,
    )
    .unwrap();
    insert_prepared_group(
        &mut groups,
        "forced".into(),
        b"first".to_vec(),
        "group".into(),
        1,
    )
    .unwrap();
    assert_eq!(groups["forced"].1.chunk_indices, [0, 1]);
    assert!(
        insert_prepared_group(
            &mut groups,
            "forced".into(),
            b"second".to_vec(),
            "group".into(),
            2
        )
        .is_err()
    );
}
