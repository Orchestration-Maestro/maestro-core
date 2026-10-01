//! What a chunk set records: its profile and its counter, the chunker's
//! chunk IDs, the same for the same input, and each chunk's exact prepared
//! input stored as the artifact its digest names, pinned. A rerun of a
//! complete set changes nothing, and a new counter makes a new set.

use super::{
    port::Goldens,
    scratch::{
        COLLECTION, Scratch, chunk_set_of, chunks_of, decide, decide_all, manifest_of, revision_of,
        tokenizer, words,
    },
    support::{BUILD, MODEL_FILE, OTHER_FILE, card},
};
use crate::prepare::{
    ChunkProfile, Error, Preparation, RouterTokenizer, TokenizerQualification, chunk_set_id,
    chunk_set_id_for_card, prepare, prepare_observed,
};
use maestro_canonicalization::{CHUNKER_VERSION, PREPARATION_PROFILE, TokenCounter as _};
use maestro_kernel::{
    chunk_set::ChunkSetState,
    document::Outcome,
    gateway::Role,
    scope::{Right, ScopeSet},
    store::Database,
};
use serde_json::json;
use std::ops::ControlFlow;

/// The corpus the tests prepare: a guide of two sections of 400 words each,
/// which the chunker keeps apart, and a paragraph of 900 words, which it
/// splits.
fn corpus(scratch: &Scratch) {
    let sections = format!(
        "# Guide\n\n## One\n\n{}\n\n## Two\n\n{}\n",
        words("one", 400),
        words("two", 400)
    );
    let long = format!("# Long\n\n{}\n", words("long", 900));
    scratch.corpus(&[("guide.md", &sections), ("long.md", &long)]);
}

/// A scratch kernel holding [`corpus`], imported and accepted, with the
/// scopes that read it.
fn imported(scratch: &Scratch) -> (Database, ScopeSet) {
    corpus(scratch);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    (database, scopes)
}

#[test]
fn a_chunk_set_records_its_profile_and_its_counter() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let set = chunk_set_of(&database, &scopes, &report);
    assert_eq!(set.chunk_profile, "mapped-structural-chunks/2");
    assert_eq!(set.chunk_profile, CHUNKER_VERSION);
    assert_eq!(set.counter_contract_id, tokenizer.contract_id());
    assert_eq!(set.collection_id, COLLECTION);
    assert_eq!(set.state, ChunkSetState::Complete);
    assert!(set.id.starts_with("chunk-set-"));
    let manifest_digest = set.manifest_digest.clone().unwrap();
    assert_eq!(
        database.artifact(&manifest_digest).unwrap().unwrap().pins,
        1
    );
    let manifest = manifest_of(&database, &set);
    assert_eq!(manifest["schema"], "maestro-chunk-set/1");
    assert_eq!(manifest["chunk_set"], json!(set.id));
    assert_eq!(manifest["chunk_profile"], CHUNKER_VERSION);
    assert_eq!(manifest["preparation_profile"], PREPARATION_PROFILE);
    assert_eq!(manifest["counter"], json!(tokenizer.contract_id()));
}

#[test]
fn the_report_counts_the_chunks_and_their_tokens_as_the_manifest_does() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let chunks = chunks_of(&database, &scopes, &report);
    // The chunker keeps sections apart: the guide's title alone, then each
    // section under its heading path; the long document's heading alone,
    // then its paragraph of 900 tokens in two pieces under that heading.
    assert_eq!(chunks.len(), 6);
    let tokens: u64 = chunks.iter().map(|chunk| chunk.token_count).sum();
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(
        [manifest["chunks"].clone(), manifest["tokens"].clone()],
        [json!(6), json!(tokens)]
    );
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "collection": COLLECTION,
            "chunk_set": report.chunk_set,
            "chunk_profile": CHUNKER_VERSION,
            "counter": tokenizer.contract_id(),
            "eligible": 2,
            "left_out": 0,
            "prepared": 2,
            "duplicates": 0,
            "near_duplicate_groups": 0,
            "chunks": 6,
            "tokens": tokens,
            "refused": 0,
            "refusals": [],
            "left_out_documents": [],
        })
    );
}

#[test]
fn each_chunks_prepared_input_is_stored_as_counted_and_pinned_by_its_chunk() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let chunks = chunks_of(&database, &scopes, &report);
    assert_eq!(chunks.len(), 6);
    for chunk in chunks {
        let artifact = database.artifact(&chunk.digest).unwrap().unwrap();
        assert_eq!(artifact.media, "text/plain; charset=utf-8");
        assert_eq!(artifact.pins, 1);
        let prepared = String::from_utf8(database.get(&chunk.digest).unwrap()).unwrap();
        let counted = tokenizer.token_ids(&prepared).unwrap().len();
        assert_eq!(chunk.token_count, u64::try_from(counted).unwrap());
        assert!(chunk.token_count <= 700);
        // The span covers source text the prepared input holds, as it is.
        let excerpt = database
            .resolve(&scopes, &report.chunk_set, &chunk.id)
            .unwrap();
        let first_word = excerpt.text.split_whitespace().next().unwrap();
        assert!(prepared.contains(first_word), "{first_word} in {prepared}");
    }
}

#[test]
fn the_document_format_counts_toward_the_700_token_chunk_limit() {
    let scratch = Scratch::new();
    let markdown = format!("# {}\n\ntail\n", words("heading", 699));
    scratch.corpus(&[("formatted.md", &markdown)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);

    let artifact = super::qualification::qualification_value(0, MODEL_FILE);
    let profile = TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    let card = super::qualification::v2_card(&profile, MODEL_FILE);
    let port = Goldens::new();
    let tokenizer = RouterTokenizer::qualify_with_profile(port.clone(), card, &profile).unwrap();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();

    assert_eq!(report.refused, 1);
    assert!(
        report.refusals[0]
            .reason
            .contains("does not fit in 700 tokens with its context")
    );
    assert!(port.texts().iter().any(|input| {
        input.starts_with("doc: ")
            && input.contains("tail")
            && input.split_whitespace().count() > 700
    }));
}

#[test]
fn a_card_identifies_the_chunk_set_without_qualifying_the_router() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let card = card(Role::Embedder, MODEL_FILE, BUILD);
    let (_, tokenizer) = tokenizer();
    assert_eq!(
        chunk_set_id_for_card(&database, &scopes, Preparation::of(COLLECTION), &card).unwrap(),
        chunk_set_id(&database, &scopes, Preparation::of(COLLECTION), &tokenizer).unwrap()
    );
}

#[test]
fn the_same_input_gives_the_same_chunk_set_and_chunk_ids() {
    let (first, second) = (Scratch::new(), Scratch::new());
    let mut prepared = Vec::new();
    for scratch in [&first, &second] {
        let (database, scopes) = imported(scratch);
        let (_, tokenizer) = tokenizer();
        let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
        prepared.push((
            report.chunk_set.clone(),
            chunks_of(&database, &scopes, &report),
        ));
    }
    assert_eq!(prepared[0], prepared[1]);
    assert_eq!(prepared[0].1.len(), 6);
    assert!(
        prepared[0]
            .1
            .iter()
            .all(|chunk| chunk.id.starts_with("chunk-"))
    );
}

#[test]
fn a_rerun_of_a_complete_set_changes_nothing_and_counts_nothing() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (port, tokenizer) = tokenizer();
    let first = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let rows =
        ["chunk_sets", "chunks", "occurrences", "artifacts"].map(|table| scratch.rows(table));
    let calls = port.texts().len();
    let second = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(second, first);
    assert_eq!(port.texts().len(), calls);
    assert_eq!(
        ["chunk_sets", "chunks", "occurrences", "artifacts"].map(|table| scratch.rows(table)),
        rows
    );
}

#[test]
fn another_counter_makes_another_chunk_set_and_leaves_the_first_as_it_is() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (_, tokenizer) = tokenizer();
    let first = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let before = chunks_of(&database, &scopes, &first);
    let other =
        RouterTokenizer::qualify(Goldens::new(), card(Role::Embedder, OTHER_FILE, BUILD)).unwrap();
    let second = prepare(&database, &scopes, COLLECTION, &other).unwrap();
    assert_ne!(second.chunk_set, first.chunk_set);
    assert_eq!(second.counter, other.contract_id());
    assert_eq!(chunks_of(&database, &scopes, &first), before);
    let after = chunks_of(&database, &scopes, &second);
    assert_eq!(after.len(), before.len());
    // The counter's contract ID is in every chunk ID.
    for (old, new) in before.iter().zip(&after) {
        assert_ne!(old.id, new.id);
        assert_eq!(old.digest, new.digest);
    }
    assert_eq!(scratch.rows("chunk_sets"), 2);
}

/// A page with a heading's copy-link label and a collapsed image's label
/// before its placeholder: page chrome under the complete-ideas profile.
const CHROME_PAGE: &str =
    "# Restart Link copied to clipboard\n\nClosed\n\n<!-- image -->\n\nRestart the service.\n";

#[test]
fn another_chunking_profile_makes_another_chunk_set_and_leaves_the_first_as_it_is() {
    let scratch = Scratch::new();
    corpus(&scratch);
    scratch.corpus(&[("chrome.md", CHROME_PAGE)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let first = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert!(first.chrome.is_empty());
    let first_set = chunk_set_of(&database, &scopes, &first);
    assert!(manifest_of(&database, &first_set).get("chrome").is_none());
    let before = chunks_of(&database, &scopes, &first);
    let ideas = Preparation {
        collection: COLLECTION,
        profile: ChunkProfile::CompleteIdeas,
    };
    let named = chunk_set_id(&database, &scopes, ideas, &tokenizer).unwrap();
    let mut unobserved = |_: &_| ControlFlow::Continue(());
    let second = prepare_observed(&database, &scopes, ideas, &tokenizer, &mut unobserved).unwrap();
    assert_eq!(second.chunk_set, named);
    assert_ne!(second.chunk_set, first.chunk_set);
    assert_eq!(second.chunk_profile, "mapped-structural-chunks/3");
    let set = chunk_set_of(&database, &scopes, &second);
    assert_eq!(
        (set.chunk_profile.as_str(), set.state),
        ("mapped-structural-chunks/3", ChunkSetState::Complete)
    );
    let manifest = manifest_of(&database, &set);
    assert_eq!(manifest["chunk_profile"], "mapped-structural-chunks/3");
    assert_eq!(
        manifest["preparation_profile"],
        "canonical-context-parts/v2"
    );
    let chrome = json!({
        "heading_suffix": {"units": 1, "bytes": 25},
        "label_before_image": {"units": 1, "bytes": 6},
        "markup": {"units": 1, "bytes": 15},
    });
    assert_eq!(manifest["chrome"], chrome);
    assert_eq!(json!(second.chrome), chrome);
    let chrome_page = revision_of(&database, &scopes, "chrome.md");
    let inputs: Vec<String> = chunks_of(&database, &scopes, &second)
        .iter()
        .filter(|chunk| chunk.revision_id == chrome_page)
        .map(|chunk| String::from_utf8(database.get(&chunk.digest).unwrap()).unwrap())
        .collect();
    assert_eq!(inputs, ["Restart\n\nRestart the service."]);
    assert_eq!(chunks_of(&database, &scopes, &first), before);
    let after = chunks_of(&database, &scopes, &second);
    assert!(
        after
            .iter()
            .all(|chunk| before.iter().all(|old| old.id != chunk.id))
    );
    assert_eq!(scratch.rows("chunk_sets"), 2);
}

#[test]
fn the_chunk_set_a_preparation_builds_is_named_before_it_starts() {
    let scratch = Scratch::new();
    corpus(&scratch);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let [guide, long] = ["guide.md", "long.md"].map(|path| revision_of(&database, &scopes, path));
    decide(&database, &guide, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let named = chunk_set_id(&database, &scopes, Preparation::of(COLLECTION), &tokenizer).unwrap();
    assert!(named.starts_with("chunk-set-"));
    assert_eq!(scratch.rows("chunk_sets"), 0, "naming it records nothing");
    // A revision accepted since, or another counter, names another set.
    decide(&database, &long, Outcome::Accepted);
    let accepted =
        chunk_set_id(&database, &scopes, Preparation::of(COLLECTION), &tokenizer).unwrap();
    assert_ne!(accepted, named);
    let other =
        RouterTokenizer::qualify(Goldens::new(), card(Role::Embedder, OTHER_FILE, BUILD)).unwrap();
    assert_ne!(
        chunk_set_id(&database, &scopes, Preparation::of(COLLECTION), &other).unwrap(),
        accepted
    );
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(report.chunk_set, accepted);
    // As the preparation does, it reads the whole collection or nothing.
    let source = "workspace/default/collection/notes/source/docs"
        .parse()
        .unwrap();
    database
        .grant("partial", &source, Right::Read, "test")
        .unwrap();
    let partial = database.visible("partial").unwrap();
    assert!(matches!(
        chunk_set_id(&database, &partial, Preparation::of(COLLECTION), &tokenizer),
        Err(Error::NotVisible(_))
    ));
}
