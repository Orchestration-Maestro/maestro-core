use super::super::{
    families::{CandidateFamily, ConflictContext},
    versions::{VersionCandidate, collapse_versions, compare_numeric_versions},
};
use maestro_kernel::evidence::Alternate;
use std::{cmp::Ordering, collections::BTreeSet};

#[test]
fn numeric_versions_compare_components_without_machine_integer_limits() {
    assert_eq!(
        compare_numeric_versions("2.10", "2.9"),
        Some(Ordering::Greater)
    );
    assert_eq!(compare_numeric_versions("2.0", "2"), Some(Ordering::Equal));
    assert_eq!(
        compare_numeric_versions("02.0003", "2.3.0"),
        Some(Ordering::Equal)
    );
    assert_eq!(
        compare_numeric_versions("1.9", "1.10"),
        Some(Ordering::Less)
    );
    assert_eq!(
        compare_numeric_versions(
            &format!("1.{}", "9".repeat(100)),
            &format!("1.{}", "1".repeat(101))
        ),
        Some(Ordering::Less)
    );
}

fn candidate(
    identity: (&str, &str, &str),
    version: Option<&str>,
    text: &str,
    input_position: usize,
    groups: &[&str],
) -> VersionCandidate {
    let (document_id, revision_id, section_id) = identity;
    VersionCandidate {
        family: CandidateFamily {
            document_id: document_id.to_owned(),
            near_group_ids: groups.iter().map(|group| (*group).to_owned()).collect(),
            section_path: vec!["Guide".to_owned(), "Install".to_owned()],
            occurrence: 1,
            context: ConflictContext::default(),
        },
        input_position,
        revision_id: revision_id.to_owned(),
        section_id: Some(section_id.to_owned()),
        version: version.map(str::to_owned),
        section_text: text.to_owned(),
        conflict_member: false,
    }
}

#[test]
fn a_single_unversioned_candidate_does_not_report_latest_undetermined() {
    let candidates = [candidate(
        ("doc", "revision", "section"),
        None,
        "same",
        0,
        &[],
    )];

    let collapsed = collapse_versions(&candidates, true).unwrap();

    assert!(!collapsed.latest_undetermined);
    assert!(collapsed.suppressed.is_empty());
    assert!(collapsed.alternates.is_empty());
}

#[test]
fn candidates_from_one_revision_do_not_report_an_undetermined_latest() {
    let candidates = [
        candidate(("doc", "revision", "section"), None, "same", 0, &[]),
        candidate(("doc", "revision", "section"), None, "same", 1, &[]),
    ];

    let collapsed = collapse_versions(&candidates, true).unwrap();

    assert!(!collapsed.latest_undetermined);
    assert!(collapsed.suppressed.is_empty());
}

#[test]
fn invalid_candidate_identities_are_rejected() {
    let candidate = candidate(
        ("doc", "revision", "section"),
        Some("1.0"),
        "same",
        0,
        &["near"],
    );
    let mut blank_revision = candidate.clone();
    blank_revision.revision_id.clear();
    let mut blank_document = candidate.clone();
    blank_document.family.document_id.clear();
    let mut blank_group = candidate.clone();
    blank_group.family.near_group_ids = BTreeSet::from([String::new()]);
    let mut blank_section = candidate;
    blank_section.section_id = Some(" ".to_owned());

    for invalid in [blank_revision, blank_document, blank_group, blank_section] {
        assert!(matches!(
            collapse_versions(&[invalid], true),
            Err(reason) if reason == "version candidate identity is invalid"
        ));
    }
}

#[test]
fn sectionless_candidates_do_not_correspond_across_revisions() {
    let mut sectionless = candidate(
        ("doc-a", "rev-a", "section-a"),
        Some("1.0"),
        "same",
        0,
        &["near"],
    );
    sectionless.section_id = None;
    let candidates = [
        sectionless,
        candidate(
            ("doc-b", "rev-b", "section-b"),
            Some("2.0"),
            "same",
            1,
            &["near"],
        ),
    ];

    let collapsed = collapse_versions(&candidates, true).unwrap();

    assert!(collapsed.suppressed.is_empty());
    assert!(collapsed.alternates.is_empty());
}

#[test]
fn latest_equal_sections_carry_older_versions_without_collapsing_latest_mirrors() {
    let candidates = [
        candidate(
            ("doc-old", "rev-old", "section-old"),
            Some("2.9"),
            "same",
            0,
            &["near"],
        ),
        candidate(
            ("doc-new", "rev-new", "section-new"),
            Some("2.10"),
            "same",
            1,
            &["near"],
        ),
        candidate(
            ("doc-mirror", "rev-mirror", "section-mirror"),
            Some("2.10"),
            "same",
            2,
            &["near"],
        ),
    ];

    let collapsed = collapse_versions(&candidates, true).unwrap();

    assert_eq!(collapsed.suppressed, BTreeSet::from([0]));
    assert_eq!(
        collapsed.alternates.get(&1),
        Some(&vec![Alternate {
            version: Some("2.9".to_owned()),
            section_id: "section-old".to_owned(),
        }])
    );
    assert!(!collapsed.suppressed.contains(&2));
    assert!(!collapsed.latest_undetermined);
}

#[test]
fn numeric_equal_versions_and_material_differences_remain_independent() {
    for candidates in [
        [
            candidate(
                ("doc-a", "rev-a", "section-a"),
                Some("2.0"),
                "same",
                0,
                &["near"],
            ),
            candidate(
                ("doc-b", "rev-b", "section-b"),
                Some("2"),
                "same",
                1,
                &["near"],
            ),
        ],
        [
            candidate(
                ("doc-a", "rev-a", "section-a"),
                Some("2.9"),
                "old bytes",
                0,
                &["near"],
            ),
            candidate(
                ("doc-b", "rev-b", "section-b"),
                Some("2.10"),
                "new bytes",
                1,
                &["near"],
            ),
        ],
    ] {
        let collapsed = collapse_versions(&candidates, true).unwrap();
        assert!(collapsed.suppressed.is_empty());
        assert!(collapsed.alternates.is_empty());
        assert!(!collapsed.latest_undetermined);
    }
}

#[test]
fn unknown_versions_report_the_limit_and_conflict_members_are_never_suppressed() {
    let unknown = [
        candidate(
            ("doc-a", "rev-a", "section-a"),
            Some("2.9"),
            "same",
            0,
            &["near"],
        ),
        candidate(("doc-b", "rev-b", "section-b"), None, "same", 1, &["near"]),
    ];
    let collapsed = collapse_versions(&unknown, true).unwrap();
    assert!(collapsed.suppressed.is_empty());
    assert!(collapsed.latest_undetermined);

    let mut conflict = candidate(
        ("doc-a", "rev-a", "section-a"),
        Some("2.9"),
        "same",
        0,
        &["near"],
    );
    conflict.conflict_member = true;
    let conflict_pair = [
        conflict,
        candidate(
            ("doc-b", "rev-b", "section-b"),
            Some("2.10"),
            "same",
            1,
            &["near"],
        ),
    ];
    let collapsed = collapse_versions(&conflict_pair, true).unwrap();
    assert!(collapsed.suppressed.is_empty());
    assert!(collapsed.alternates.is_empty());
}

#[test]
fn disabled_collapse_and_unrelated_families_remain_independent() {
    let candidates = [
        candidate(
            ("doc-a", "rev-a", "section-a"),
            Some("2.9"),
            "same",
            0,
            &["near-a"],
        ),
        candidate(
            ("doc-b", "rev-b", "section-b"),
            Some("2.10"),
            "same",
            1,
            &["near-b"],
        ),
    ];
    let disabled = collapse_versions(&candidates, false).unwrap();
    assert!(disabled.suppressed.is_empty());
    assert!(!disabled.latest_undetermined);

    let mut other_occurrence = candidates[1].clone();
    other_occurrence
        .family
        .section_path
        .push("Install".to_owned());
    other_occurrence.family.occurrence = 2;
    let unrelated = [candidates[0].clone(), other_occurrence];
    let collapsed = collapse_versions(&unrelated, true).unwrap();
    assert!(collapsed.suppressed.is_empty());
}

#[test]
fn non_numeric_or_malformed_versions_have_no_order() {
    for version in ["", ".1", "1.", "1..2", "v1.2", "1.-2", " 1.2"] {
        assert_eq!(
            compare_numeric_versions(version, "1.2"),
            None,
            "{version:?}"
        );
    }
}
