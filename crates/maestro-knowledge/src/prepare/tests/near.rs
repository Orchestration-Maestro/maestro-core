//! Near duplicates (01 §6): word 5-gram shingles, `MinHash` signatures and
//! bands propose pairs, each confirmed by its exact shingle Jaccard, at 0.85
//! or more; the confirmed pairs link their documents into groups.

use super::super::near::{Shingles, Vocabulary, groups, signature, words as text_words};
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::{artifact::Digest, document::NearDuplicate};
use serde_json::json;

/// The words `<stem>1` to `<stem><count>`.
fn words(stem: &str, count: usize) -> Vec<String> {
    (1..=count)
        .map(|number| format!("{stem}{number}"))
        .collect()
}

/// `base` with its last `count` words replaced by words of `stem`.
fn ending(base: &[String], count: usize, stem: &str) -> Vec<String> {
    let kept = base.len() - count;
    [base[..kept].to_vec(), words(stem, count)].concat()
}

/// `base` with its first `count` words replaced by words of `stem`.
fn starting(base: &[String], count: usize, stem: &str) -> Vec<String> {
    [words(stem, count), base[count..].to_vec()].concat()
}

/// The groups of `documents`, each a revision and its words, shingled with
/// one vocabulary.
fn grouped(documents: &[(&str, Vec<String>)]) -> Vec<Vec<NearDuplicate>> {
    let mut vocabulary = Vocabulary::default();
    let shingled: Vec<(&str, Shingles)> = documents
        .iter()
        .map(|(revision, words)| {
            let words: Vec<&str> = words.iter().map(String::as_str).collect();
            (*revision, Shingles::of(&words, &mut vocabulary))
        })
        .collect();
    let borrowed: Vec<(&str, &Shingles)> = shingled
        .iter()
        .map(|(revision, shingles)| (*revision, shingles))
        .collect();
    groups(&borrowed)
}

/// The id of the group of `members`.
fn group_id(members: &[&str]) -> String {
    format!(
        "near-{}",
        Digest::of(json!(members).to_string().as_bytes()).as_str()
    )
}

/// The member `revision` of the group of `members`, held there by `jaccard`.
fn member(members: &[&str], revision: &str, jaccard: f64) -> NearDuplicate {
    NearDuplicate {
        group_id: group_id(members),
        revision_id: revision.to_owned(),
        jaccard,
    }
}

#[test]
fn a_pair_is_grouped_only_when_its_exact_jaccard_reaches_0_85() {
    // 104 words make 100 shingles; replacing the last k words replaces k of
    // them, for a Jaccard of (100 - k) / (100 + k).
    let base = words("base", 104);
    // 41 words make 37 shingles: 34 / 40 is 0.85 exactly.
    let short = words("short", 41);
    let documents = [
        ("rev-a", base.clone()),
        ("rev-b", ending(&base, 8, "b")),
        ("rev-c", ending(&base, 9, "c")),
        ("rev-d", short.clone()),
        ("rev-e", ending(&short, 3, "e")),
    ];
    // 92 / 108 is 0.8519; 91 / 109, 0.8349, is not enough.
    let (first, second) = (["rev-a", "rev-b"], ["rev-d", "rev-e"]);
    assert_eq!(
        grouped(&documents),
        [
            vec![
                member(&first, "rev-a", 92.0 / 108.0),
                member(&first, "rev-b", 92.0 / 108.0)
            ],
            vec![
                member(&second, "rev-d", 0.85),
                member(&second, "rev-e", 0.85)
            ]
        ]
    );
}

#[test]
fn confirmed_pairs_link_a_group_and_each_member_keeps_its_best_jaccard() {
    let base = words("base", 104);
    let near = ending(&base, 3, "near");
    // Close to the second, 93 / 107, but not to the first, 90 / 110.
    let chained = starting(&near, 7, "chained");
    let documents = [
        ("rev-c", chained),
        ("rev-a", base),
        ("rev-b", near),
        ("rev-z", words("other", 104)),
    ];
    let members = ["rev-a", "rev-b", "rev-c"];
    assert_eq!(
        grouped(&documents),
        [vec![
            member(&members, "rev-a", 97.0 / 103.0),
            member(&members, "rev-b", 97.0 / 103.0),
            member(&members, "rev-c", 93.0 / 107.0),
        ]]
    );
}

#[test]
fn groups_come_in_the_order_of_their_first_member_whatever_the_input_order() {
    let first = words("first", 60);
    let second = words("second", 60);
    let documents = [
        ("rev-d", ending(&second, 1, "d")),
        ("rev-b", ending(&first, 1, "b")),
        ("rev-c", second),
        ("rev-a", first),
    ];
    let mut reversed = documents.clone();
    reversed.reverse();
    let expected = [
        vec![
            member(&["rev-a", "rev-b"], "rev-a", 55.0 / 57.0),
            member(&["rev-a", "rev-b"], "rev-b", 55.0 / 57.0),
        ],
        vec![
            member(&["rev-c", "rev-d"], "rev-c", 55.0 / 57.0),
            member(&["rev-c", "rev-d"], "rev-d", 55.0 / 57.0),
        ],
    ];
    assert_eq!(grouped(&documents), expected);
    assert_eq!(grouped(&reversed), expected);
}

#[test]
fn a_document_of_fewer_than_five_words_is_one_shingle_and_one_of_none_is_never_grouped() {
    let short = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
    let documents = [
        ("rev-a", short("restart the agent")),
        ("rev-b", short("restart the agent")),
        ("rev-c", short("restart the server")),
        ("rev-d", Vec::new()),
        ("rev-e", Vec::new()),
    ];
    let members = ["rev-a", "rev-b"];
    assert_eq!(
        grouped(&documents),
        [vec![
            member(&members, "rev-a", 1.0),
            member(&members, "rev-b", 1.0)
        ]]
    );
}

#[test]
fn words_are_compared_as_they_are_written() {
    let base = words("base", 20);
    let upper: Vec<String> = base.iter().map(|word| word.to_uppercase()).collect();
    assert_eq!(
        grouped(&[("rev-a", base), ("rev-b", upper)]),
        Vec::<Vec<_>>::new()
    );
}

#[test]
fn signatures_hash_the_words_themselves_the_same_everywhere() {
    // FNV-1a over each word and a byte 0xFF, mixed by SplitMix64 under the
    // seeds 1 to 128: the values an independent implementation gives.
    let words: Vec<&str> = "Restart the agent before the upgrade".split(' ').collect();
    let hashes = signature(&words);
    assert_eq!(hashes.len(), 128);
    assert_eq!(
        hashes[..4],
        [
            0x257f_b76f_68e2_8897,
            0x0614_90da_ee04_29c1,
            0x3343_605c_9c48_620b,
            0xcde2_cc94_7084_209c
        ]
    );
    assert_eq!(hashes[127], 0x3f28_e681_e6d0_a3e7);
    // Fewer than five words are one shingle.
    assert_eq!(
        signature(&["restart", "the", "agent"])[..2],
        [0x8d79_c393_5e2e_a475, 0x6d10_2a35_7e8e_8099]
    );
}

#[test]
fn a_documents_words_are_those_of_its_root_blocks_but_its_frontmatter() {
    let markdown = "---\ntitle: Front matter\n---\n# Heading\n\n- first item\n- second item\n\n\
                    Closing words.\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "words")).unwrap();
    // The list's text holds its items' once; the frontmatter is metadata.
    assert_eq!(
        text_words(&document),
        [
            "Heading", "-", "first", "item", "-", "second", "item", "Closing", "words."
        ]
    );
}
