//! The authorized scope bounds each group, and an invalid, duplicate,
//! warned, tampered or unauthorized input refuses the whole batch.
use super::batch_inputs::{document, scope};
use maestro_canonicalization::{DedupInput, WarningPolicy, group_exact};

#[test]
fn scope_and_current_authorization_bound_membership_without_deleting_history() {
    let text = "# Title\n\nSame source content.\n";
    let source_a = document(text, "a");
    let source_b = document(text, "b");
    let authorization = scope(&[&source_a, &source_b]);
    let inputs = [
        DedupInput {
            document: &source_a,
            markdown: text,
        },
        DedupInput {
            document: &source_b,
            markdown: text,
        },
    ];
    let original = group_exact(&authorization, &inputs, WarningPolicy::Preserve).unwrap();
    for (tenant, workspace) in [("tenant-b", "workspace-a"), ("tenant-a", "workspace-b")] {
        let mut changed = scope(&[&source_a, &source_b]);
        changed.tenant_id = tenant.into();
        changed.workspace_id = workspace.into();
        let result = group_exact(&changed, &inputs, WarningPolicy::Preserve).unwrap();
        assert!(
            original
                .groups
                .iter()
                .zip(&result.groups)
                .all(|(unchanged_group, changed_group)| unchanged_group.group_id
                    != changed_group.group_id)
        );
    }
    let revoked = scope(&[&source_b]);
    assert!(group_exact(&revoked, &inputs, WarningPolicy::Preserve).is_err());
    let surviving = group_exact(&revoked, &inputs[1..], WarningPolicy::Preserve).unwrap();
    assert_eq!(surviving.occurrences.len(), 1);
    assert_eq!(
        surviving.occurrences[0].document.document_id,
        source_b.document_id
    );
    assert!(
        surviving
            .groups
            .iter()
            .all(|group| group.occurrence_indices == [0])
    );
    assert_eq!(original.occurrences.len(), 2);
    assert_eq!(source_a.content_hash, source_b.content_hash);
}

#[test]
fn invalid_scope_duplicate_keys_warnings_and_tampering_refuse_the_batch() {
    let text = "# Title\n\nDo not delete.\n";
    let doc = document(text, "a");
    let authorization = scope(&[&doc]);
    let input = DedupInput {
        document: &doc,
        markdown: text,
    };
    for (tenant, workspace) in [("", "a"), ("a", " \t")] {
        let mut invalid = scope(&[&doc]);
        invalid.tenant_id = tenant.into();
        invalid.workspace_id = workspace.into();
        assert!(group_exact(&invalid, &[input], WarningPolicy::Preserve).is_err());
    }
    assert!(group_exact(&authorization, &[input, input], WarningPolicy::Preserve).is_err());
    assert!(group_exact(&authorization, &[input], WarningPolicy::Reject).is_err());
    assert!(
        group_exact(
            &authorization,
            &[DedupInput {
                document: &doc,
                markdown: "changed"
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let mut forged = doc.clone();
    forged.blocks[0].retrieval_text = "forged".into();
    assert!(
        group_exact(
            &authorization,
            &[DedupInput {
                document: &forged,
                markdown: text
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let empty = document("", "empty");
    let empty_scope = scope(&[&empty]);
    assert!(
        group_exact(
            &empty_scope,
            &[DedupInput {
                document: &empty,
                markdown: ""
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let no_inputs = group_exact(&authorization, &[], WarningPolicy::Reject).unwrap();
    assert!(no_inputs.occurrences.is_empty() && no_inputs.groups.is_empty());
}

#[test]
fn unauthorized_inputs_do_not_leak_replay_or_source_details() {
    let text = "# Private marker\n\nContent.\n";
    let valid = document(text, "private-source-marker");
    let mut forged = valid.clone();
    forged.blocks.clear();
    let authorization = scope(&[]);
    let refused = |doc| {
        group_exact(
            &authorization,
            &[DedupInput {
                document: doc,
                markdown: text,
            }],
            WarningPolicy::Preserve,
        )
        .unwrap_err()
        .to_string()
    };
    assert_eq!(refused(&valid), refused(&forged));
    assert!(!refused(&valid).contains("Private marker"));
    assert!(!refused(&valid).contains(&valid.document_id));
}
