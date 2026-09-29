//! Registering and listing scoped model cards.

use super::{args::ModelCommand, collection, output::Output};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    artifact::Digest,
    gateway::{CardError, Role, RouterClient},
    model::{
        CardRecord, Error as ModelError, EvaluationDisposition, EvaluationMode, EvaluationRecord,
        ModelCardRegistrationError, NewModelSelection, SelectedModelCard, register_model_card,
    },
    store::Error as StoreError,
};
use maestro_knowledge::eval::{RerankerEvaluation, evaluate_and_record_reranker_health};
use serde::Serialize;
use std::{env, path::Path, process::ExitCode};
use tokio::runtime::Builder;
use ulid::Ulid;

/// JSON schema emitted after model-card registration.
const REGISTER_SCHEMA: &str = "maestro-cli/model-register/1";
/// JSON schema emitted when listing model registry state.
const LIST_SCHEMA: &str = "maestro-cli/model-list/1";

/// Runs a model registry command.
pub(super) fn run(
    kernel: &Kernel,
    output: Output,
    command: &ModelCommand,
) -> Result<ExitCode, Failure> {
    match command {
        ModelCommand::Register {
            collection,
            card,
            evidence,
            gguf,
        } => register(
            kernel,
            output,
            collection,
            RegistrationFiles {
                card,
                evidence,
                gguf,
            },
        ),
        ModelCommand::Check { collection, digest } => check(kernel, output, collection, digest),
        ModelCommand::Select {
            collection,
            role,
            digest,
        } => select(kernel, output, collection, role, digest),
        ModelCommand::List { collection, role } => {
            list(kernel, output, collection, role.as_deref())
        }
    }
}

/// Input paths required to register a v2 card.
#[derive(Clone, Copy)]
struct RegistrationFiles<'a> {
    /// The v2 card JSON document.
    card: &'a Path,
    /// The digest-named directory of pinned evidence files.
    evidence: &'a Path,
    /// The GGUF file pinned by the card.
    gguf: &'a Path,
}

/// Registers a v2 card, returning its digest; repeated registrations are unchanged.
fn register(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    files: RegistrationFiles<'_>,
) -> Result<ExitCode, Failure> {
    collection::declared(kernel, collection_id)?;
    let (record, outcome) = register_model_card(
        &kernel.database,
        &kernel.scopes,
        collection_id,
        &kernel.artifacts,
        files.card,
        files.evidence,
        Some(files.gguf),
    )
    .map_err(|error| registration_failure(&error))?;
    let digest = record.digest.as_str();
    let document = RegisteredCard {
        schema: REGISTER_SCHEMA,
        collection: collection_id,
        digest,
        role: record.role,
    };
    output.result(
        &document,
        &format!(
            "model card {} {outcome} for {collection_id} ({})",
            digest, record.role
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Runs the real two-pair reranker health gate and records its card-bound receipt.
fn check(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    digest_text: &str,
) -> Result<ExitCode, Failure> {
    collection::declared(kernel, collection_id)?;
    let digest = Digest::parse(digest_text).map_err(|error| Failure::refused_by(&error))?;
    let card = kernel
        .database
        .model_card(&kernel.scopes, collection_id, &digest)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused(format!(
                "no model card {} is registered for {collection_id}",
                digest.as_str()
            ))
        })?;
    if card.fields().role != Role::Reranker {
        let qualification = if card.fields().role == Role::Embedder {
            "embedder qualification runs through prepare"
        } else {
            "answerer qualification has no registration check path"
        };
        return Err(Failure::refused(format!(
            "cannot check {} card: {qualification}",
            card.fields().role
        )));
    }
    let router_url =
        super::health::router_url(env::var_os(super::health::ROUTER_VARIABLE).as_deref())
            .map_err(Failure::refused)?;
    let client = RouterClient::new(router_url).map_err(|error| Failure::failed_by(&error))?;
    let run_id = Ulid::generate().to_string();
    let evaluation = RerankerEvaluation {
        collection_id,
        run_id: &run_id,
        generation_id: None,
        manifest: &[],
    };
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Failure::failed_by(&error))?;
    let (health, record) = runtime
        .block_on(evaluate_and_record_reranker_health(
            &kernel.database,
            &kernel.scopes,
            &card,
            &client,
            &evaluation,
        ))
        .map_err(|error| Failure::failed_by(&error))?;
    let disposition = disposition(health.disposition());
    let reason = health.failure();
    let document = CheckedCard {
        schema: "maestro-cli/model-check/1",
        collection: collection_id,
        digest: digest.as_str(),
        evaluation: record.id.to_string(),
        disposition,
        eligible: health.disposition() == EvaluationDisposition::Eligible,
        reason,
        report_digest: record.report_digest.as_str(),
    };
    let text = if let Some(reason) = reason {
        format!("model card {} is not eligible: {reason}", digest.as_str())
    } else {
        format!(
            "model card {} is eligible; evaluation {}",
            digest.as_str(),
            record.id
        )
    };
    output.result(&document, &text)?;
    if matches!(
        health.disposition(),
        EvaluationDisposition::Failed | EvaluationDisposition::Interrupted
    ) {
        return Err(Failure::failed(text));
    }
    Ok(ExitCode::SUCCESS)
}

/// Selects the latest eligible real evaluation of exactly the requested card.
fn select(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    role_text: &str,
    digest_text: &str,
) -> Result<ExitCode, Failure> {
    collection::declared(kernel, collection_id)?;
    let role = roles(Some(role_text))?
        .into_iter()
        .next()
        .ok_or_else(|| Failure::refused("no model role was selected"))?;
    let digest = Digest::parse(digest_text).map_err(|error| Failure::refused_by(&error))?;
    let card = kernel
        .database
        .model_card(&kernel.scopes, collection_id, &digest)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused(format!(
                "no model card {} is registered for {collection_id}",
                digest.as_str()
            ))
        })?;
    if card.fields().role != role {
        return Err(Failure::refused(format!(
            "model card {} has role {}, not requested role {role}",
            digest.as_str(),
            card.fields().role
        )));
    }
    let card_record = kernel
        .database
        .model_cards(&kernel.scopes, collection_id, role)
        .map_err(|error| Failure::failed_by(&error))?
        .into_iter()
        .find(|record| record.digest == digest)
        .ok_or_else(|| {
            Failure::refused(format!("model card {} is not registered", digest.as_str()))
        })?;
    let evaluation = kernel
        .database
        .model_evaluations(&kernel.scopes, collection_id)
        .map_err(|error| Failure::failed_by(&error))?
        .into_iter()
        .rev()
        .find(|record| {
            record.card_id == card_record.id
                && record.role == role
                && record.mode == EvaluationMode::Real
        })
        .ok_or_else(|| {
            Failure::refused(format!(
                "selection requires a real evaluation of the exact card and role; check {} first",
                digest.as_str()
            ))
        })?;
    if evaluation.disposition != EvaluationDisposition::Eligible {
        return Err(Failure::refused(format!(
            "the latest real evaluation of card {} is {}, not eligible",
            digest.as_str(),
            disposition(evaluation.disposition)
        )));
    }
    let selected = kernel
        .database
        .record_model_selection(
            &kernel.scopes,
            &NewModelSelection {
                collection_id,
                role,
                card_id: card_record.id,
                evaluation_id: evaluation.id,
                selected_by: "maestro-cli",
                reason: "explicit maestro model select",
            },
        )
        .map_err(|error| model_failure(&error))?;
    let document = SelectedCard {
        schema: "maestro-cli/model-select/1",
        collection: collection_id,
        digest: digest.as_str(),
        role,
        selection: selected.id.to_string(),
        evaluation: evaluation.id.to_string(),
    };
    output.result(
        &document,
        &format!("model card {} selected for {role}", digest.as_str()),
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Selection output, bound to the exact real eligible evaluation.
#[derive(Serialize)]
struct SelectedCard<'a> {
    /// Versioned CLI document schema.
    schema: &'static str,
    /// Collection selected.
    collection: &'a str,
    /// Selected card digest.
    digest: &'a str,
    /// Selected role.
    role: Role,
    /// Recorded selection identity.
    selection: String,
    /// Exact selected evaluation identity.
    evaluation: String,
}

/// Names a health disposition for CLI output.
const fn disposition(value: EvaluationDisposition) -> &'static str {
    match value {
        EvaluationDisposition::Eligible => "eligible",
        EvaluationDisposition::Ineligible => "ineligible",
        EvaluationDisposition::Blocked => "blocked",
        EvaluationDisposition::Failed => "failed",
        EvaluationDisposition::Interrupted => "interrupted",
    }
}

/// A reranker check's immutable recorded result.
#[derive(Serialize)]
struct CheckedCard<'a> {
    /// Versioned CLI document schema.
    schema: &'static str,
    /// Collection checked.
    collection: &'a str,
    /// Exact card digest checked.
    digest: &'a str,
    /// Recorded health-evaluation identity.
    evaluation: String,
    /// Health gate disposition.
    disposition: &'static str,
    /// Whether the card passed the gate.
    eligible: bool,
    /// First qualification failure, if any.
    reason: Option<&'a str>,
    /// Content digest of the recorded health report.
    report_digest: &'a str,
}

/// Lists cards, evaluations and one current selection for each requested role.
fn list(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
    role: Option<&str>,
) -> Result<ExitCode, Failure> {
    collection::declared(kernel, collection_id)?;
    let roles = roles(role)?;
    let mut cards = Vec::new();
    let mut selections = Vec::new();
    for &role in &roles {
        cards.extend(
            kernel
                .database
                .model_cards(&kernel.scopes, collection_id, role)
                .map_err(|error| Failure::failed_by(&error))?
                .iter()
                .map(CardSummary::from),
        );
        if let Some(selected) = kernel
            .database
            .selected_model_card(&kernel.scopes, collection_id, role)
            .map_err(|error| Failure::failed_by(&error))?
        {
            selections.push(SelectionSummary::from(&selected));
        }
    }
    let evaluations = kernel
        .database
        .model_evaluations(&kernel.scopes, collection_id)
        .map_err(|error| Failure::failed_by(&error))?
        .iter()
        .filter(|evaluation| roles.contains(&evaluation.role))
        .map(|evaluation| {
            let card = cards
                .iter()
                .find(|card| card.id == evaluation.card_id.to_string())
                .ok_or_else(|| Failure::failed("evaluation card is missing from model list"))?;
            Ok(EvaluationSummary::from(evaluation, card))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let document = ModelList {
        schema: LIST_SCHEMA,
        collection: collection_id,
        cards,
        evaluations,
        selections,
    };
    let mut lines = vec![format!("models for {collection_id}")];
    lines.extend(
        document
            .cards
            .iter()
            .map(|card| format!("card {} {}", card.role, card.digest)),
    );
    lines.extend(document.evaluations.iter().map(|evaluation| {
        format!(
            "evaluation {} {} {} {} {}",
            evaluation.id,
            evaluation.role,
            evaluation.digest,
            evaluation.mode,
            evaluation.disposition
        )
    }));
    lines.extend(document.selections.iter().map(|selection| {
        format!(
            "selection {} {} evaluation {}",
            selection.role, selection.digest, selection.evaluation
        )
    }));
    let text = lines.join("\n");
    output.result(&document, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// Maps registration validation errors to refusals and I/O/storage errors to failures.
fn registration_failure(error: &ModelCardRegistrationError) -> Failure {
    match error {
        ModelCardRegistrationError::Unauthorized | ModelCardRegistrationError::Invalid(_) => {
            Failure::refused_by(error)
        }
        ModelCardRegistrationError::Model(error) => model_failure(error),
        ModelCardRegistrationError::Integrity(_)
        | ModelCardRegistrationError::Store(_)
        | ModelCardRegistrationError::Io(_) => Failure::failed_by(&error),
    }
}

/// Maps registry validation refusals while preserving database and artifact failures.
fn model_failure(error: &ModelError) -> Failure {
    match error {
        ModelError::Unauthorized
        | ModelError::Invalid(_)
        | ModelError::Card(CardError::Invalid(_)) => Failure::refused_by(error),
        ModelError::Store(StoreError::Sqlite(rusqlite::Error::SqliteFailure(sqlite, _)))
            if sqlite.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Failure::refused_by(error)
        }
        ModelError::Card(CardError::Store(_)) | ModelError::Integrity(_) | ModelError::Store(_) => {
            Failure::failed_by(error)
        }
    }
}

/// Parses the requested role or returns all roles in stable card order.
fn roles(role: Option<&str>) -> Result<Vec<Role>, Failure> {
    match role {
        None => Ok(Role::ALL.to_vec()),
        Some("embedder") => Ok(vec![Role::Embedder]),
        Some("reranker") => Ok(vec![Role::Reranker]),
        Some("answerer") => Ok(vec![Role::Answerer]),
        Some(other) => Err(Failure::refused(format!(
            "unknown model role {other:?}; expected embedder, reranker or answerer"
        ))),
    }
}

/// Registration output.
#[derive(Serialize)]
struct RegisteredCard<'a> {
    /// Versioned CLI document schema.
    schema: &'static str,
    /// Owning collection.
    collection: &'a str,
    /// Registered card digest.
    digest: &'a str,
    /// Registered card role.
    role: Role,
}

/// Model-list output.
#[derive(Serialize)]
struct ModelList<'a> {
    /// Versioned CLI document schema.
    schema: &'static str,
    /// Collection listed.
    collection: &'a str,
    /// Registered candidate card metadata.
    cards: Vec<CardSummary>,
    /// Recorded card evaluations.
    evaluations: Vec<EvaluationSummary>,
    /// Current selection per role.
    selections: Vec<SelectionSummary>,
}

/// Public card metadata for list output.
#[derive(Serialize)]
struct CardSummary {
    /// Registration identifier, used to resolve evaluation card digests.
    #[serde(skip)]
    id: String,
    /// Card digest.
    digest: String,
    /// Card role.
    role: Role,
}

impl From<&CardRecord> for CardSummary {
    fn from(record: &CardRecord) -> Self {
        Self {
            id: record.id.to_string(),
            digest: record.digest.as_str().to_owned(),
            role: record.role,
        }
    }
}

/// Public evaluation metadata for list output.
#[derive(Serialize)]
struct EvaluationSummary {
    /// Evaluation identifier.
    id: String,
    /// Card digest evaluated.
    digest: String,
    /// Evaluated role.
    role: Role,
    /// Evaluation mode.
    mode: &'static str,
    /// Qualification disposition.
    disposition: &'static str,
}

impl EvaluationSummary {
    /// Converts a stored evaluation with its registered card's digest.
    fn from(record: &EvaluationRecord, card: &CardSummary) -> Self {
        Self {
            id: record.id.to_string(),
            digest: card.digest.clone(),
            role: record.role,
            mode: match record.mode {
                EvaluationMode::Real => "real",
                EvaluationMode::Synthetic => "synthetic",
            },
            disposition: disposition(record.disposition),
        }
    }
}

/// Public selected card metadata for list output.
#[derive(Serialize)]
struct SelectionSummary {
    /// Selected card digest.
    digest: String,
    /// Selected role.
    role: Role,
    /// Exact eligible real evaluation.
    evaluation: String,
}

impl From<&SelectedModelCard> for SelectionSummary {
    fn from(selected: &SelectedModelCard) -> Self {
        Self {
            digest: selected.card.digest().as_str().to_owned(),
            role: selected.selection.role,
            evaluation: selected.evaluation.id.to_string(),
        }
    }
}
