//! Authoring projection dispatch; host rendering and controlled effects remain replaceable ports.
use super::check::today;
use crate::{
    cli::{output::Output, session, trust, trust_path},
    failure::Failure,
    presentation::{message::Message, messages::MessageKey},
};
use clap::Args;
use maestro_catalog::{
    bootstrap::AreaInventories,
    hosts::{self, SourceSnapshot},
    policy::workspace::{CheckedTrust, JournalTrust, WorkspaceTrust as _},
    source::{Known, builtin, frozen_rows},
};
use maestro_kernel::workspace::{Answer, WorkspaceAnswer, WorkspaceAuthority};
use serde_json::json;
use std::{
    io,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Explicit authoring source selection and exact target; no implicit HOME write target exists.
#[derive(Debug, Args)]
pub(in crate::cli) struct Request {
    /// Exact native host; unsupported/ambiguous names refuse without fallback.
    #[arg(long)]
    host: String,
    /// Existing isolated project directory; only this target may receive owned writes.
    #[arg(long, value_name = "DIR")]
    target: PathBuf,
    /// Checked authoring catalog directory, not installed admission.
    #[arg(long, value_name = "DIR")]
    catalog_dir: PathBuf,
    /// Explicit preset selection; repeat for a checked closure.
    #[arg(long, required = true)]
    preset: Vec<String>,
    /// Apply the displayed owned-file transition; never implicitly approve trust.
    #[arg(long)]
    apply: bool,
    /// Preview owned-only removal; --apply is still necessary for effects.
    #[arg(long)]
    remove: bool,
}

/// Project one exact host through its registered adapter and C04/C05j owned-effect ports.
///
/// # Errors
/// Refuses invalid sources, hosts, targets, shadows, drift, missing trust and effect failures.
pub(in crate::cli) fn run(output: Output, request: &Request) -> Result<ExitCode, Failure> {
    let adapter = hosts::adapter(&request.host).map_err(Failure::refused)?;
    let root = trust::boundaries()?
        .canonical_root(&request.target)
        .map_err(Failure::refused)?;
    let settings = maestro_settings::Registry::built_in().map_err(Failure::failed)?;
    let rows = frozen_rows();
    let provider = AreaInventories::new(
        &request.catalog_dir,
        builtin().map_err(Failure::failed)?,
        Known {
            rows: &rows,
            settings: &settings,
            today: today()?,
        },
    )
    .map_err(Failure::refused)?
    .with_compiled_backends(session::compiled_backends());
    let snapshot = SourceSnapshot::resolve(&provider, &request.preset).map_err(Failure::refused)?;
    let boundaries = trust::boundaries()?;
    let database = trust::existing_database()?;
    let authority = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&authority, &boundaries);
    let mut preview = adapter
        .preview(&root, &snapshot, request.remove, &checked)
        .map_err(|error| record_failure(output, &error))?;
    if !request.apply {
        return show(output, &preview);
    }
    require_exact_trust(output, &root)?;
    // Rebind authority, never the reviewed source or proposed file bytes.
    let database = trust::existing_database()?;
    let authority = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&authority, &boundaries);
    preview
        .apply(&root, &checked)
        .map_err(|error| record_failure(output, &error))?;
    preview.registered = !request.remove;
    show(output, &preview)
}

/// Print exactly one machine document; text mode keeps the same honest state labels.
fn show(output: Output, value: &impl serde::Serialize) -> Result<ExitCode, Failure> {
    let text = serde_json::to_string_pretty(value).map_err(Failure::failed)?;
    output.result(value, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// Drift and failed effects are recorded separately from registration and observation.
fn record_failure(output: Output, error: &io::Error) -> Failure {
    let reason = error.to_string();
    let stale = reason.contains("changed") || reason.contains("ownership");
    let document = json!({"schema":"maestro-copilot-preview/1", "registered":false,
        "observed":false, "stale":if stale { vec![reason.clone()] } else { vec![] },
        "failed":if stale { vec![] } else { vec![reason.clone()] }});
    match show(output, &document) {
        Ok(_) => Failure::refused(reason),
        Err(failure) => failure,
    }
}

/// Ask once through C05h for the exact target, never an ancestor or an automatic HOME grant.
fn require_exact_trust(output: Output, root: &Path) -> Result<(), Failure> {
    let boundaries = trust::boundaries()?;
    let database = trust::existing_database()?;
    let authority = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&authority, &boundaries);
    if checked.containing_root(root).as_deref() == Some(root) {
        return Ok(());
    }
    let suggestion = || trust_path::suggestion_message(root);
    if output.is_json() {
        return Err(Failure::refused_message(
            Message::new(MessageKey::CatalogTargetUntrusted, &[])
                .with_message("instruction", suggestion()),
        ));
    }
    let prompt = output.wording(
        MessageKey::CatalogTargetApprove,
        &[("path", &trust_path::visible_path(root))],
    )?;
    let confirmation = trust::approve(root, None, &prompt)
        .map_err(|error| {
            Failure::refused_message(
                Message::new(
                    MessageKey::DiagnosticInstruction,
                    &[("error", &error.to_string())],
                )
                .with_message("instruction", suggestion()),
            )
        })?
        .ok_or_else(|| {
            Failure::refused_message(
                Message::new(MessageKey::CatalogTargetDeclined, &[])
                    .with_message("instruction", suggestion()),
            )
        })?;
    trust::database()?
        .record_workspace_answer(&WorkspaceAnswer {
            path: root.to_path_buf(),
            answer: Answer::Approved { confirmation },
        })
        .map_err(Failure::refused)?;
    Ok(())
}
