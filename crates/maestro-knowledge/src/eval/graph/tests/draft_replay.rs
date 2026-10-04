//! Durable reservations prevent duplicate calls and budget resets after interruption.
use super::draft::{DraftModel, budget, card, window};
use crate::eval::graph::{
    draft::{DraftError, DraftRequest},
    draft_progress::{DraftJournal, DraftOutcome, DraftReceipt, DraftRun, resume_draft},
};
use maestro_kernel::artifact::Digest;
use std::sync::atomic::Ordering;

/// In-memory durable boundary with an injected terminal-write failure.
#[derive(Default)]
struct Journal {
    entries: Vec<DraftReceipt>,
    fail_finish: bool,
}
impl DraftJournal for Journal {
    fn receipts(&self) -> &[DraftReceipt] {
        &self.entries
    }
    fn append(&mut self, receipt: DraftReceipt) -> Result<(), DraftError> {
        if self.fail_finish && !self.entries.is_empty() {
            return Err(DraftError::Journal);
        }
        self.entries.push(receipt);
        Ok(())
    }
}

#[tokio::test]
async fn draft_replay_never_calls_the_model_or_duplicates_saved_items() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"approved inventory and run limits"),
        max_windows: 2,
        max_tokens: 6144,
    };
    let model = DraftModel::valid();
    let mut journal = Journal::default();
    let first = resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    let replay = resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    assert_eq!(first.suite, replay.suite);
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
    assert_eq!(journal.entries.len(), 2);
}

#[tokio::test]
async fn draft_interrupted_reservation_is_retained_and_not_retried() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 2,
        max_tokens: 6144,
    };
    let model = DraftModel::valid();
    let mut journal = Journal {
        fail_finish: true,
        ..Journal::default()
    };
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Journal
    );
    journal.fail_finish = false;
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Interrupted
    );
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
    assert_eq!(journal.entries.len(), 2);
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Interrupted
    );
    assert_eq!(journal.entries.len(), 2);
}

#[tokio::test]
async fn draft_replay_rejects_changed_inputs_and_cumulative_budget_overflow() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 1,
        max_tokens: 6144,
    };
    let model = DraftModel::valid();
    let mut journal = Journal::default();
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    let mut changed = window.clone();
    changed.span[1] -= 1;
    let mut request = DraftRequest {
        window: &changed,
        ..request
    };
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Journal
    );
    let mut second = window.clone();
    second.id = "q-2".into();
    request.window = &second;
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Budget
    );
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn draft_replay_refuses_a_corrupted_saved_candidate_instead_of_exporting_it() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 1,
        max_tokens: 3072,
    };
    let model = DraftModel::valid();
    let mut journal = Journal::default();
    resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    if let DraftOutcome::Draft(candidate) = &mut journal.entries.last_mut().unwrap().outcome {
        candidate.labels = "PRIVATE corrupted".into();
    } else {
        panic!("missing candidate");
    }
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Journal
    );
}

#[tokio::test]
async fn draft_replay_still_checks_actual_prompt_and_card_pins() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let mut request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 1,
        max_tokens: 3072,
    };
    let model = DraftModel::valid();
    let mut journal = Journal::default();
    resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    request.prompt = "changed prompt";
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Prompt
    );
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn draft_replay_refuses_token_only_exhaustion_with_windows_remaining() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"token-only run"),
        max_windows: 2,
        max_tokens: 3072,
    };
    let model = DraftModel::valid();
    let mut journal = Journal::default();
    resume_draft(&model, &request, &run, &mut journal)
        .await
        .unwrap();
    let mut second = window.clone();
    second.id = "q-2".into();
    let request = DraftRequest {
        window: &second,
        ..request
    };
    assert_eq!(
        resume_draft(&model, &request, &run, &mut journal)
            .await
            .unwrap_err(),
        DraftError::Budget
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
    assert_eq!(journal.entries.len(), 2);
}

#[tokio::test]
async fn draft_journal_terminal_identity_and_transition_fields_are_independent() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 1,
        max_tokens: 3072,
    };
    let mut journal = Journal::default();
    resume_draft(&DraftModel::valid(), &request, &run, &mut journal)
        .await
        .unwrap();
    let saved = journal.entries.clone();
    for defect in ["transition", "input", "tokens", "id", "family"] {
        journal.entries = saved.clone();
        match defect {
            "transition" => journal.entries[0].outcome = journal.entries[1].outcome.clone(),
            "input" => journal.entries[0].input = Digest::of(b"changed reservation"),
            "tokens" => journal.entries[1].tokens += 1,
            _ => corrupt_candidate(&mut journal.entries[1], defect),
        }
        assert_eq!(
            resume_draft(&DraftModel::valid(), &request, &run, &mut journal)
                .await
                .err(),
            Some(DraftError::Journal),
            "{defect}"
        );
    }
}

#[tokio::test]
async fn draft_journal_rejects_each_existing_population_over_budget_before_replay() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let mut run = DraftRun {
        digest: Digest::of(b"run"),
        max_windows: 1,
        max_tokens: 3072,
    };
    let mut journal = Journal::default();
    resume_draft(&DraftModel::valid(), &request, &run, &mut journal)
        .await
        .unwrap();
    for (windows, tokens) in [(0, 3072), (1, 3071)] {
        run.max_windows = windows;
        run.max_tokens = tokens;
        assert_eq!(
            resume_draft(&DraftModel::valid(), &request, &run, &mut journal)
                .await
                .unwrap_err(),
            DraftError::Budget
        );
    }
}

/// Change one saved envelope identity without changing its valid label or suite.
fn corrupt_candidate(receipt: &mut DraftReceipt, field: &str) {
    let DraftOutcome::Draft(candidate) = &mut receipt.outcome else {
        panic!("draft")
    };
    if field == "id" {
        candidate.id = "q-2".into();
    } else {
        candidate.family = "f-2".into();
    }
}
