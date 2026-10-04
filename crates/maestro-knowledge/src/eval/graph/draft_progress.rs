//! Durable per-window reservations: an interrupted model call is never silently repeated.
use super::draft::{
    DraftCandidate, DraftError, DraftRequest, check_candidate, checked_chat, draft_window,
};
use maestro_kernel::{artifact::Digest, gateway::ModelPort};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Immutable run identity and cumulative limits, frozen in the private manifest.
#[derive(Debug, Clone)]
pub struct DraftRun {
    /// Digest binding approval, inventory, card, prompt and all limits.
    pub digest: Digest,
    /// Maximum attempted windows, including failed and interrupted calls.
    pub max_windows: usize,
    /// Maximum reserved input plus output tokens over all attempts.
    pub max_tokens: u64,
}

/// Private durable outcome; only counters and fixed codes may be printed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DraftOutcome {
    /// Reservation committed before any inference, consuming its full token budget.
    Started,
    /// Candidate remains unreviewed even though its anchors were checked.
    Draft(DraftCandidate),
    /// Retained failure; replay returns it rather than attempting inference again.
    Failed(DraftError),
}

/// One durable reservation or its terminal outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftReceipt {
    /// Immutable run identity.
    pub run: Digest,
    /// Exact per-window request identity.
    pub input: Digest,
    /// Host-assigned item/window ID.
    pub id: String,
    /// Conservative input plus output reservation, charged once at start.
    pub tokens: u64,
    /// Started or terminal outcome.
    pub outcome: DraftOutcome,
}

/// Small replaceable private persistence port; append must be durable before returning.
/// The adapter must hold exclusive ownership for the entire read/append run.
pub trait DraftJournal {
    /// All previously committed records in order.
    fn receipts(&self) -> &[DraftReceipt];
    /// Atomically append and synchronize a record, or return a fixed persistence refusal.
    ///
    /// # Errors
    /// Returns [`DraftError::Journal`] when durability cannot be established.
    fn append(&mut self, receipt: DraftReceipt) -> Result<(), DraftError>;
}

/// Resume a window without duplicating an inference call or resetting cumulative limits.
/// Started-only records become retained interrupted failures on replay.
///
/// # Errors
/// Returns fixed candidate/inference errors, or refuses changed receipts
/// and exhausted budgets.
pub async fn resume_draft<P: ModelPort + Sync, J: DraftJournal>(
    port: &P,
    request: &DraftRequest<'_>,
    run: &DraftRun,
    journal: &mut J,
) -> Result<DraftCandidate, DraftError> {
    let input = request_digest(request, run);
    let state = journal_state(journal.receipts(), run)?;
    let families = state
        .latest
        .values()
        .filter_map(|receipt| {
            if receipt.id != request.window.id
                && let DraftOutcome::Draft(candidate) = &receipt.outcome
            {
                Some(candidate.family.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    if let Some(previous) = state.latest.get(&request.window.id) {
        if previous.input != input {
            return Err(DraftError::Journal);
        }
        checked_chat(request)?;
        let previous = (*previous).clone();
        return replay(previous, request, &families, journal);
    }
    let tokens = u64::from(request.budget.input_tokens) + u64::from(request.budget.output_tokens);
    if state.latest.len() >= run.max_windows
        || tokens == 0
        || state
            .tokens
            .checked_add(tokens)
            .is_none_or(|total| total > run.max_tokens)
    {
        return Err(DraftError::Budget);
    }
    let reservation = DraftReceipt {
        run: run.digest.clone(),
        input,
        id: request.window.id.clone(),
        tokens,
        outcome: DraftOutcome::Started,
    };
    journal.append(reservation.clone())?;
    let result = draft_window(port, request, &families).await;
    let outcome = match &result {
        Ok(candidate) => DraftOutcome::Draft(candidate.clone()),
        Err(error) => DraftOutcome::Failed(*error),
    };
    journal.append(DraftReceipt {
        outcome,
        ..reservation
    })?;
    result
}

/// Bind every caller-controlled inference input and cumulative limit to replay identity.
fn request_digest(request: &DraftRequest<'_>, run: &DraftRun) -> Digest {
    let window = request.window;
    let identity = serde_json::json!({
        "run":run.digest, "max_windows":run.max_windows, "max_tokens":run.max_tokens,
        "card":request.card_digest, "prompt":request.prompt_digest,
        "id":window.id, "source_ref":window.source_ref, "revision":window.revision_id,
        "original":window.original, "span":window.span, "budget":request.budget,
    });
    Digest::of(identity.to_string().as_bytes())
}

/// Validated latest outcomes and conservative cumulative charge.
struct JournalState<'a> {
    /// Each unique reservation's latest record.
    latest: BTreeMap<String, &'a DraftReceipt>,
    /// Tokens charged by starts, never released on failure.
    tokens: u64,
}

/// Reject reordered, duplicated, cross-run or changed terminal records.
fn journal_state<'a>(
    receipts: &'a [DraftReceipt],
    run: &DraftRun,
) -> Result<JournalState<'a>, DraftError> {
    let mut state = JournalState {
        latest: BTreeMap::new(),
        tokens: 0,
    };
    for receipt in receipts {
        if receipt.run != run.digest {
            return Err(DraftError::Journal);
        }
        match (&receipt.outcome, state.latest.get(&receipt.id)) {
            (DraftOutcome::Started, None) => {
                state.tokens = state
                    .tokens
                    .checked_add(receipt.tokens)
                    .ok_or(DraftError::Journal)?;
            }
            (DraftOutcome::Draft(_) | DraftOutcome::Failed(_), Some(previous))
                if matches!(previous.outcome, DraftOutcome::Started)
                    && previous.input == receipt.input
                    && previous.tokens == receipt.tokens => {}
            _ => return Err(DraftError::Journal),
        }
        state.latest.insert(receipt.id.clone(), receipt);
    }
    if state.latest.len() > run.max_windows || state.tokens > run.max_tokens {
        return Err(DraftError::Budget);
    }
    Ok(state)
}

/// Revalidate saved candidates; an uncertain previous call becomes a terminal failure.
fn replay<J: DraftJournal>(
    previous: DraftReceipt,
    request: &DraftRequest<'_>,
    families: &BTreeSet<String>,
    journal: &mut J,
) -> Result<DraftCandidate, DraftError> {
    match &previous.outcome {
        DraftOutcome::Draft(candidate) => {
            let output = serde_json::json!({"suite":candidate.suite, "labels":candidate.labels});
            let checked = check_candidate(&output.to_string(), request.window, families)
                .map_err(|_| DraftError::Journal)?;
            if checked.id != candidate.id || checked.family != candidate.family {
                return Err(DraftError::Journal);
            }
            Ok(checked)
        }
        DraftOutcome::Failed(error) => Err(*error),
        DraftOutcome::Started => {
            journal.append(DraftReceipt {
                outcome: DraftOutcome::Failed(DraftError::Interrupted),
                ..previous
            })?;
            Err(DraftError::Interrupted)
        }
    }
}
