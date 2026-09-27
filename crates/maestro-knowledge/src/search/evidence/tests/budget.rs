use super::super::budget::{
    CounterError, CounterInfo, EvidenceCounter, count_passages, counter_info, serialized_passages,
    verify_counter,
};
use maestro_canonicalization::{Error, TokenCounter};
use maestro_kernel::artifact::Digest;
use maestro_kernel::evidence::{Alternate, Passage, Span};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct CharacterCounter {
    id: &'static str,
    verifications: AtomicUsize,
    calls: AtomicUsize,
    fail_verification: AtomicBool,
    fail_tokenization: AtomicBool,
}

impl CharacterCounter {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            verifications: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            fail_verification: AtomicBool::new(false),
            fail_tokenization: AtomicBool::new(false),
        }
    }
}

impl TokenCounter for CharacterCounter {
    fn contract_id(&self) -> &str {
        self.id
    }

    fn verify(&self) -> Result<(), Error> {
        self.verifications.fetch_add(1, Ordering::Relaxed);
        if self.fail_verification.load(Ordering::Relaxed) {
            return Err(Error("test counter verification failure".to_owned()));
        }
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_tokenization.load(Ordering::Relaxed) {
            return Err(Error("test counter tokenization failure".to_owned()));
        }
        Ok(input.chars().map(u32::from).collect())
    }
}

fn passage() -> Passage {
    Passage {
        n: 1,
        section_id: Some("section-a".to_owned()),
        document_id: "doc-a".to_owned(),
        revision_id: "revision-a".to_owned(),
        title: "Title".to_owned(),
        section_path: vec!["Title".to_owned()],
        version: None,
        source_ref: "corpus-path:doc-a.md".to_owned(),
        span: Span { start: 0, end: 4 },
        digest: Digest::of(b"text"),
        text: "text".to_owned(),
        windowed: false,
        alternates: Vec::<Alternate>::new(),
    }
}

#[test]
fn utf8_counter_counts_compact_non_ascii_passage_json_and_echoes_its_identity() {
    let mut passage = passage();
    passage.digest = Digest::of("café".as_bytes());
    passage.span = Span { start: 0, end: 5 };
    passage.text = "café".to_owned();
    let passages = vec![passage];
    let identity = counter_info(&EvidenceCounter::Utf8Bytes).unwrap();
    let json = serialized_passages(&passages).unwrap();
    let expected_json = serde_json::to_string(&passages).unwrap();

    assert_eq!(identity.counter.as_deref(), Some("evidence-utf8-bytes/1"));
    assert!(identity.estimated);
    assert_eq!(json, expected_json);
    assert!(json.starts_with("[{"));
    assert!(json.contains("\"text\":\"café\""));
    assert!(json.len() > json.chars().count());
    assert_eq!(
        count_passages(&passages, &EvidenceCounter::Utf8Bytes, &identity).unwrap(),
        u32::try_from(expected_json.len()).unwrap()
    );
}

#[test]
fn counter_debug_shows_the_strategy_and_exact_contract() {
    let utf8 = EvidenceCounter::Utf8Bytes;
    assert_eq!(format!("{utf8:?}"), "Utf8Bytes");
    let counter = EvidenceCounter::Exact(Arc::new(CharacterCounter::new("test/characters")));
    assert_eq!(format!("{counter:?}"), "Exact(\"test/characters\")");
}

#[test]
fn empty_passages_cost_zero_without_invoking_an_exact_counter() {
    let counter = Arc::new(CharacterCounter::new("test/characters"));
    let evidence_counter = EvidenceCounter::Exact(counter.clone());
    let identity = counter_info(&evidence_counter).unwrap();
    verify_counter(&evidence_counter, &identity).unwrap();
    assert_eq!(
        count_passages(&[], &evidence_counter, &identity).unwrap(),
        0
    );

    assert_eq!(counter.verifications.load(Ordering::Relaxed), 1);
    assert_eq!(counter.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn exact_counter_uses_token_ids_and_rejects_a_changed_contract() {
    let counter = Arc::new(CharacterCounter::new("test/characters"));
    let evidence_counter = EvidenceCounter::Exact(counter.clone());
    let identity = counter_info(&evidence_counter).unwrap();
    verify_counter(&evidence_counter, &identity).unwrap();
    let passages = vec![passage()];
    let json = serialized_passages(&passages).unwrap();
    assert_eq!(
        count_passages(&passages, &evidence_counter, &identity).unwrap(),
        u32::try_from(json.chars().count()).unwrap()
    );
    verify_counter(&evidence_counter, &identity).unwrap();
    assert_eq!(counter.verifications.load(Ordering::Relaxed), 2);
    assert_eq!(counter.calls.load(Ordering::Relaxed), 1);

    let changed = CounterInfo {
        counter: Some("test/changed".to_owned()),
        estimated: false,
    };
    assert!(matches!(
        verify_counter(&evidence_counter, &changed),
        Err(CounterError::Invalid(
            "token counter contract ID changed during assembly"
        ))
    ));
}

#[test]
fn exact_counter_failures_keep_their_source_errors() {
    let counter = Arc::new(CharacterCounter::new("test/characters"));
    let evidence_counter = EvidenceCounter::Exact(counter.clone());
    let identity = counter_info(&evidence_counter).unwrap();

    counter.fail_verification.store(true, Ordering::Relaxed);
    assert!(matches!(
        verify_counter(&evidence_counter, &identity),
        Err(CounterError::Counter(Error(message)))
            if message == "test counter verification failure"
    ));

    counter.fail_verification.store(false, Ordering::Relaxed);
    counter.fail_tokenization.store(true, Ordering::Relaxed);
    assert!(matches!(
        count_passages(&[passage()], &evidence_counter, &identity),
        Err(CounterError::Counter(Error(message)))
            if message == "test counter tokenization failure"
    ));
    assert_eq!(counter.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn exact_counter_contract_ids_must_not_be_blank() {
    let counter = Arc::new(CharacterCounter::new("  "));
    assert!(matches!(
        counter_info(&EvidenceCounter::Exact(counter)),
        Err(CounterError::Invalid(
            "exact token counter has a blank contract ID"
        ))
    ));
    assert!(matches!(
        counter_info(&EvidenceCounter::Utf8Bytes),
        Ok(CounterInfo {
            counter: Some(_),
            estimated: true,
        })
    ));
}
