//! Near duplicates (docs/architecture/01 §6): documents whose word 5-gram
//! shingles mostly agree, such as two releases of one page.
//!
//! `MinHash` signatures of 128 hashes, cut into 32 bands of 4, propose the
//! pairs that share a band; each pair is then confirmed by the exact Jaccard
//! of its two shingle sets, 0.85 or more, since a signature's estimate alone
//! is never trusted. The confirmed pairs link documents into groups: a group
//! is every document they link, named after its members, and each member
//! keeps the best Jaccard that links it. A group deletes nothing.
//!
//! A document's words are those of its canonical text: the retrieval text of
//! its root blocks but its frontmatter, split on whitespace and compared as
//! written, case included. A shingle is five consecutive words, or all of
//! them when a document has fewer; a document without words has no shingle
//! and is never grouped. Signatures hash the words themselves, with FNV-1a
//! and `SplitMix64`, so they are the same on every platform and in every run.

use maestro_canonicalization::{BlockType, CanonicalDocument};
use maestro_kernel::{artifact::Digest, document::NearDuplicate};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
};

/// Words in a shingle.
const SHINGLE: usize = 5;
/// Hashes in a signature.
const HASHES: u64 = 128;
/// Hashes in a band: a signature has 32.
const ROWS: usize = 4;
/// The least Jaccard that confirms a pair, as a fraction: 85 in 100.
const LEAST: (u64, u64) = (85, 100);

/// The words of `document` its shingles are made of.
pub(super) fn words(document: &CanonicalDocument) -> Vec<&str> {
    document
        .blocks
        .iter()
        .filter(|block| block.parent_block_id.is_none() && block.block_type != BlockType::Metadata)
        .flat_map(|block| block.retrieval_text.split_whitespace())
        .collect()
}

/// The words the documents of one preparation are written in, each with a
/// number of its own.
#[derive(Debug, Default)]
pub(super) struct Vocabulary(HashMap<String, usize>);

impl Vocabulary {
    /// The number of `word`, a new one the first time it is seen.
    fn number(&mut self, word: &str) -> usize {
        let next = self.0.len();
        *self.0.entry(word.to_owned()).or_insert(next)
    }
}

/// A document's words, as numbers, and the `MinHash` signature of its
/// shingles: none for a document without words.
#[derive(Debug)]
pub(super) struct Shingles {
    /// Its words, in order.
    words: Vec<usize>,
    /// The least of each of [`HASHES`] hashes over its shingles.
    signature: Option<Vec<u64>>,
}

impl Shingles {
    /// The shingles of `words`, numbered in `vocabulary`.
    pub(super) fn of(words: &[&str], vocabulary: &mut Vocabulary) -> Self {
        Self {
            words: words.iter().map(|word| vocabulary.number(word)).collect(),
            signature: (!words.is_empty()).then(|| signature(words)),
        }
    }

    /// Its distinct shingles, as runs of its word numbers, in order.
    fn set(&self) -> Vec<&[usize]> {
        let mut set: Vec<&[usize]> = self.words.windows(SHINGLE).collect();
        if set.is_empty() && !self.words.is_empty() {
            set.push(&self.words);
        }
        set.sort_unstable();
        set.dedup();
        set
    }
}

/// The groups of near duplicates among `documents`, each a revision and its
/// shingles: each group's members in revision order, with the group's id
/// and the best Jaccard each keeps, and the groups in the order of their
/// first member.
pub(super) fn groups(documents: &[(&str, &Shingles)]) -> Vec<Vec<NearDuplicate>> {
    let mut best: BTreeMap<&str, f64> = BTreeMap::new();
    let mut links: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for ((first, second), jaccard) in confirmed(documents) {
        for (member, other) in [(first, second), (second, first)] {
            let kept = best.entry(member).or_insert(jaccard);
            *kept = kept.max(jaccard);
            links.entry(member).or_default().insert(other);
        }
    }
    let mut grouped = BTreeSet::new();
    let mut groups = Vec::new();
    for start in links.keys().copied() {
        if grouped.insert(start) {
            let members = linked(start, &links, &mut grouped);
            groups.push(group(&members, &best));
        }
    }
    groups
}

/// `start` and every revision `links` link to it, however indirectly, each
/// added to `grouped` as it is reached.
fn linked<'a>(
    start: &'a str,
    links: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    grouped: &mut BTreeSet<&'a str>,
) -> BTreeSet<&'a str> {
    let mut members = BTreeSet::from([start]);
    let mut pending = vec![start];
    while let Some(member) = pending.pop() {
        for other in links.get(member).into_iter().flatten().copied() {
            if grouped.insert(other) {
                members.insert(other);
                pending.push(other);
            }
        }
    }
    members
}

/// The group of `members`, each with its best Jaccard of `best`.
fn group(members: &BTreeSet<&str>, best: &BTreeMap<&str, f64>) -> Vec<NearDuplicate> {
    let names: Vec<&str> = members.iter().copied().collect();
    let group_id = format!(
        "near-{}",
        Digest::of(json!(names).to_string().as_bytes()).as_str()
    );
    names
        .into_iter()
        .map(|revision| NearDuplicate {
            group_id: group_id.clone(),
            revision_id: revision.to_owned(),
            jaccard: best.get(revision).copied().unwrap_or_default(),
        })
        .collect()
}

/// The pairs of `documents`' revisions whose signatures share a band and
/// whose exact Jaccard is at least 0.85, each with that Jaccard.
fn confirmed<'a>(documents: &[(&'a str, &Shingles)]) -> Vec<((&'a str, &'a str), f64)> {
    let candidates = candidates(documents);
    let involved: BTreeSet<usize> = candidates
        .iter()
        .flat_map(|(first, second)| [*first, *second])
        .collect();
    let sets: HashMap<usize, (&str, Vec<&[usize]>)> = involved
        .into_iter()
        .filter_map(|index| {
            let (revision, shingles) = documents.get(index)?;
            Some((index, (*revision, shingles.set())))
        })
        .collect();
    let mut pairs = Vec::new();
    for (first, second) in candidates {
        let (Some((first_revision, first_set)), Some((second_revision, second_set))) =
            (sets.get(&first), sets.get(&second))
        else {
            continue;
        };
        let (shared, union) = overlap(first_set, second_set);
        if shared * LEAST.1 >= union * LEAST.0 {
            pairs.push(((*first_revision, *second_revision), ratio(shared, union)));
        }
    }
    pairs
}

/// The pairs of indexes of `documents` whose signatures share a band, the
/// smaller index first, each once, in order.
fn candidates(documents: &[(&str, &Shingles)]) -> BTreeSet<(usize, usize)> {
    let mut buckets: HashMap<(usize, &[u64]), Vec<usize>> = HashMap::new();
    for (index, (_, shingles)) in documents.iter().enumerate() {
        for (band, rows) in shingles
            .signature
            .iter()
            .flat_map(|signature| signature.chunks(ROWS))
            .enumerate()
        {
            buckets.entry((band, rows)).or_default().push(index);
        }
    }
    let mut pairs = BTreeSet::new();
    for members in buckets.values() {
        for (position, first) in members.iter().enumerate() {
            pairs.extend(
                members
                    .iter()
                    .skip(position + 1)
                    .map(|second| (*first, *second)),
            );
        }
    }
    pairs
}

/// How many shingles the sorted sets `first` and `second` share, and how
/// many they hold together.
fn overlap(first: &[&[usize]], second: &[&[usize]]) -> (u64, u64) {
    let (mut left, mut right) = (first.iter().peekable(), second.iter().peekable());
    let mut shared = 0_u64;
    while let (Some(one), Some(other)) = (left.peek(), right.peek()) {
        match one.cmp(other) {
            Ordering::Less => {
                left.next();
            }
            Ordering::Greater => {
                right.next();
            }
            Ordering::Equal => {
                shared += 1;
                left.next();
                right.next();
            }
        }
    }
    let total = u64::try_from(first.len() + second.len()).unwrap_or(u64::MAX);
    (shared, total - shared)
}

/// `shared` over `union` as a number from 0 to 1.
fn ratio(shared: u64, union: u64) -> f64 {
    let exact = |count: u64| f64::from(u32::try_from(count).unwrap_or(u32::MAX));
    exact(shared) / exact(union)
}

/// The `MinHash` signature of the shingles of `words`, which holds at least
/// one: for each of [`HASHES`] seeds, the least hash of a shingle under it.
pub(super) fn signature(words: &[&str]) -> Vec<u64> {
    let mut shingles: Vec<&[&str]> = words.windows(SHINGLE).collect();
    if shingles.is_empty() {
        shingles.push(words);
    }
    let hashes: Vec<u64> = shingles.iter().map(|shingle| fnv(shingle)).collect();
    (1..=HASHES)
        .map(|seed| {
            let seed = mix(seed);
            hashes
                .iter()
                .map(|hash| mix(hash ^ seed))
                .min()
                .unwrap_or(u64::MAX)
        })
        .collect()
}

/// The 64-bit FNV-1a hash of `words`, each followed by the byte 0xFF, which
/// UTF-8 never holds.
fn fnv(words: &[&str]) -> u64 {
    words
        .iter()
        .flat_map(|word| word.bytes().chain([0xFF]))
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

/// `SplitMix64`'s mix of `value`.
fn mix(value: u64) -> u64 {
    let mixed = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
