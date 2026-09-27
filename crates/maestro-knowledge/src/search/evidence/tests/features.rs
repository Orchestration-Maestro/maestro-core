use super::super::{
    features::{diversity_features, diversity_similarity, mmr_score, word_shingles},
    sections::SectionIndex,
};
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ContentNode, Inline, InlineKind, SourceSpan, canonicalize,
};
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "fixture.md")).unwrap()
}

fn features(markdown: &str, version: Option<&str>) -> super::super::features::DiversityFeatures {
    let document = document(markdown);
    diversity_features(
        markdown,
        &document,
        Span {
            start: 0,
            end: markdown.len(),
        },
        version.map(str::to_owned),
    )
    .unwrap()
}

fn assert_no_similarity_penalty(
    left: &super::super::features::DiversityFeatures,
    right: &super::super::features::DiversityFeatures,
) {
    assert!(diversity_similarity(left, right).unwrap().abs() < f64::EPSILON);
}

fn find_inline_code(inline: &mut Inline) -> Option<&mut Inline> {
    if matches!(&inline.content, InlineKind::Code { .. }) {
        return Some(inline);
    }
    inline.children.iter_mut().find_map(find_inline_code)
}

#[test]
fn word_shingles_keep_case_and_use_the_whole_short_sequence() {
    assert_eq!(
        word_shingles(" alpha\tbeta\ngamma delta "),
        BTreeSet::from([
            vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()],
            vec!["beta".to_owned(), "gamma".to_owned(), "delta".to_owned()],
        ])
    );
    assert_eq!(
        word_shingles("alpha beta"),
        BTreeSet::from([vec!["alpha".to_owned(), "beta".to_owned()]])
    );
    assert!(word_shingles(" \n\t ").is_empty());
    assert_eq!(
        word_shingles("A b c"),
        BTreeSet::from([vec!["A".to_owned(), "b".to_owned(), "c".to_owned(),]])
    );
}

#[test]
fn numeric_code_and_condition_differences_disable_redundancy_penalties() {
    let variants = [
        ("# Guide\n\nSet PORT=7005.\n", "# Guide\n\nSet PORT=7006.\n"),
        (
            "# Guide\n\nRun `tool --force`.\n",
            "# Guide\n\nRun `tool --forceful`.\n",
        ),
        (
            "# Guide\n\nIf Linux, choose A.\n",
            "# Guide\n\nIf Windows, choose A.\n",
        ),
    ];
    for (left, right) in variants {
        let left = features(left, Some("1"));
        let right = features(right, Some("1"));
        assert_no_similarity_penalty(&left, &right);
    }
    let same = features("# Guide\n\nSame words here.\n", Some("1"));
    let other_version = features("# Guide\n\nSame words here.\n", Some("2"));
    assert_no_similarity_penalty(&same, &other_version);
    let case_variant = features("A b c", None);
    let lower_case = features("a b c", None);
    assert_no_similarity_penalty(&case_variant, &lower_case);
}

#[test]
fn fenced_code_and_negation_differences_disable_redundancy_penalties() {
    for (left, right) in [
        (
            "# Guide\n\n```sh\nprintf one\n```\n",
            "# Guide\n\n```sh\nprintf two\n```\n",
        ),
        (
            "# Guide\n\nNever enter this area.\n",
            "# Guide\n\nEnter this area.\n",
        ),
        ("# Guide\n\nYou can't", "# Guide\n\nYou can"),
    ] {
        assert_no_similarity_penalty(&features(left, None), &features(right, None));
    }
}

#[test]
fn numeric_token_positions_use_absolute_offsets_for_nonzero_extents() {
    let prefix = "## Intro\n\nLead text.\n\n## Guide\n\n";
    let code_first = format!("{prefix}Run `tool` then 7005.\n");
    let number_first = format!("{prefix}Run 7005 then `tool`.\n");
    let section_features = |markdown: &str| {
        let source_document = document(markdown);
        let section = source_document
            .sections
            .iter()
            .find(|section| section.title == "Guide")
            .unwrap();
        let extent = SectionIndex::new(&source_document, markdown)
            .unwrap()
            .section_extent(&section.section_id)
            .unwrap();
        diversity_features(markdown, &source_document, extent, None).unwrap()
    };

    assert_no_similarity_penalty(
        &section_features(&code_first),
        &section_features(&number_first),
    );
}

#[test]
fn protected_signature_keeps_numeric_tokens_in_source_order() {
    let code_before_number = features("# Guide\n\nRun `tool` before 7005.\n", None);
    let number_before_code = features("# Guide\n\nRun 7005 before `tool`.\n", None);

    assert_no_similarity_penalty(&code_before_number, &number_before_code);
}

#[test]
fn jaccard_similarity_uses_intersection_over_union() {
    let same = features("alpha beta gamma delta", None);
    assert!((diversity_similarity(&same, &same).unwrap() - 1.0).abs() < f64::EPSILON);

    let left = features("alpha beta gamma delta", None);
    let right = features("alpha beta gamma epsilon", None);
    assert!((diversity_similarity(&left, &right).unwrap() - (1.0 / 3.0)).abs() < f64::EPSILON);
}

#[test]
fn inline_code_outside_the_selected_extent_does_not_change_its_signature() {
    let markdown = "## A\n\nUse `tool`.\n\n## B\n\nOutside.\n";
    let extent = Span {
        start: 0,
        end: markdown.find("## B").unwrap(),
    };
    let outside = markdown.find("Outside.").unwrap();
    let mut code_document = document(markdown);
    let code = code_document
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structured_content.children)
        .find_map(|node| match node {
            ContentNode::Inline { inline } => find_inline_code(inline),
            ContentNode::Block { .. } => None,
        })
        .unwrap();
    code.source_span = SourceSpan {
        start: outside,
        end: outside + "Outside.".len(),
    };

    let mut text_document = document(markdown);
    let text = text_document
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structured_content.children)
        .find_map(|node| match node {
            ContentNode::Inline { inline } => find_inline_code(inline),
            ContentNode::Block { .. } => None,
        })
        .unwrap();
    text.content = InlineKind::Text {
        text: "tool".to_owned(),
    };

    let code_features = diversity_features(markdown, &code_document, extent, None).unwrap();
    let text_features = diversity_features(markdown, &text_document, extent, None).unwrap();
    assert!(diversity_similarity(&code_features, &text_features).unwrap() > 0.0);
}

#[test]
fn invalid_feature_extents_are_rejected() {
    let markdown = "valid";
    let source_document = document(markdown);
    assert!(
        diversity_features(
            markdown,
            &source_document,
            Span {
                start: 0,
                end: usize::MAX
            },
            None,
        )
        .is_err()
    );
    assert!(
        diversity_features(markdown, &source_document, Span { start: 1, end: 0 }, None,).is_err()
    );
    assert!(
        diversity_features(markdown, &source_document, Span { start: 1, end: 1 }, None,).is_err()
    );
    let unicode = "é";
    let unicode_document = document(unicode);
    assert!(
        diversity_features(unicode, &unicode_document, Span { start: 1, end: 2 }, None).is_err()
    );
    assert!(
        diversity_features(unicode, &unicode_document, Span { start: 0, end: 1 }, None).is_err()
    );
}

#[test]
fn ordinal_mmr_matches_the_fixed_relevance_and_diversity_weights() {
    let duplicate = mmr_score(1, 1.0).unwrap();
    let disjoint = mmr_score(2, 0.0).unwrap();
    assert!((duplicate - 0.05).abs() < f64::EPSILON);
    assert!((disjoint - (0.7 / 3.0)).abs() < f64::EPSILON);
    assert!(mmr_score(0, -0.1).is_err());
    assert!(mmr_score(0, f64::NAN).is_err());
}
