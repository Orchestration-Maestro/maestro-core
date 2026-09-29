//! The services' checks: Qdrant answering its gRPC health check as the
//! pinned version; the model router listing its catalog, which starts no
//! model; and each role's model card, which the bake-off records (T030).
//! Each waits at most [`PATIENCE`] for an answer.

use super::check::Check;
use crate::{
    cli::setup::{QDRANT, Readiness, SERVICE},
    failure::{Failure, chain},
};
use maestro_kernel::gateway::{Role, RouterClient, Url};
use maestro_knowledge::index::Qdrant as QdrantClient;
use std::{borrow::Cow, ffi::OsStr, future::Future, time::Duration};
use tokio::{runtime::Builder, time};

/// Where the model router answers unless `MAESTRO_ROUTER_URL` says
/// otherwise: its own default address.
pub(super) const DEFAULT_ROUTER: &str = "http://127.0.0.1:8080";
/// The variable that names where the model router answers.
pub(crate) const ROUTER_VARIABLE: &str = "MAESTRO_ROUTER_URL";
/// Where Qdrant's gRPC API answers unless `MAESTRO_QDRANT_URL` says otherwise.
pub(crate) const DEFAULT_QDRANT: &str = "http://127.0.0.1:6334";
/// The variable that names where Qdrant's gRPC API answers.
pub(crate) const QDRANT_VARIABLE: &str = "MAESTRO_QDRANT_URL";
/// How long a check waits for a service to answer.
const PATIENCE: Duration = Duration::from_secs(5);

/// Where Qdrant's gRPC API answers: `variable`, the value of
/// [`QDRANT_VARIABLE`], when set; else [`DEFAULT_QDRANT`].
pub(crate) fn qdrant_url(variable: Option<&OsStr>) -> String {
    variable
        .map_or(Cow::Borrowed(DEFAULT_QDRANT), OsStr::to_string_lossy)
        .into_owned()
}

/// The check of Qdrant's gRPC API at `address`: it must answer as the pinned
/// version. When it does not answer, `readiness` says what setup would still
/// do, which gives the next action.
pub(super) fn qdrant_check(
    address: &str,
    readiness: impl FnOnce() -> Result<Readiness, Failure>,
) -> Check {
    let version = block_on(async {
        let qdrant = QdrantClient::new(address).map_err(|error| chain(&error))?;
        time::timeout(PATIENCE, qdrant.version())
            .await
            .map_err(|_| format!("no answer within {PATIENCE:?}"))?
            .map_err(|error| chain(&error))
    });
    match version {
        Ok(version) if version == QDRANT.version => {
            Check::passed("qdrant", address, format!("Qdrant {version} answers"))
        }
        Ok(version) => Check::failed(
            "qdrant",
            address,
            format!(
                "Qdrant {version} answers, not the pinned {}",
                QDRANT.version
            ),
            "install the pinned version: `maestro setup --yes`",
        ),
        Err(reason) => Check::failed(
            "qdrant",
            address,
            format!("no answer: {reason}"),
            unanswered(readiness()),
        ),
    }
}

/// The next action for a Qdrant that does not answer, from what setup would
/// still do.
fn unanswered(readiness: Result<Readiness, Failure>) -> String {
    match readiness {
        Ok(Readiness::Steps(steps)) if steps.is_empty() => format!(
            "it is set up but does not answer: restart it with `systemctl --user restart \
             {SERVICE}`, and read why in `journalctl --user -u {SERVICE}`"
        ),
        Ok(Readiness::Steps(_)) => "set it up: `maestro setup` shows what it will do, and \
                                    `maestro setup --yes` does it"
            .to_owned(),
        Ok(Readiness::ByHand) => format!(
            "set Qdrant {} up by hand, as `maestro setup` explains",
            QDRANT.version
        ),
        Err(failure) => format!("run `maestro setup`, which says what it lacks: {failure}"),
    }
}

/// Where the model router answers: `variable`, the value of
/// [`ROUTER_VARIABLE`], when it is set, else [`DEFAULT_ROUTER`].
///
/// # Errors
///
/// The text of the variable, when it is no URL.
pub(crate) fn router_url(variable: Option<&OsStr>) -> Result<Url, String> {
    let text = variable.map_or(Cow::Borrowed(DEFAULT_ROUTER), OsStr::to_string_lossy);
    Url::parse(&text).map_err(|_| text.into_owned())
}

/// The check of the model router at `url`: it must list its catalog.
pub(super) fn router_check(url: Result<Url, String>) -> Check {
    let url = match url {
        Ok(url) => url,
        Err(text) => {
            return Check::failed(
                "router",
                &text,
                format!("{ROUTER_VARIABLE} is not a URL"),
                format!("set {ROUTER_VARIABLE} to the router's address, such as {DEFAULT_ROUTER}"),
            );
        }
    };
    let target = url.to_string();
    let listed = RouterClient::new(url)
        .map_err(|error| chain(&error))
        .and_then(|client| {
            block_on(async {
                time::timeout(PATIENCE, client.catalog())
                    .await
                    .map_err(|_| format!("no answer within {PATIENCE:?}"))?
                    .map_err(|error| chain(&error))
            })
        });
    match listed {
        Ok(entries) => Check::passed(
            "router",
            &target,
            format!("{} entries in its catalog", entries.len()),
        ),
        Err(reason) => Check::failed(
            "router",
            &target,
            format!("the model router does not answer with its catalog: {reason}"),
            format!(
                "start maestro-model-router so that it answers at {target}, or set \
                 {ROUTER_VARIABLE} to where it does"
            ),
        ),
    }
}

/// The check of each M1 role's model card: none is recorded before the
/// bake-off records one per role (T030).
pub(super) fn card_checks() -> Vec<Check> {
    // The extractor's check comes with its selection (S2 G20).
    [Role::Embedder, Role::Reranker, Role::Answerer]
        .iter()
        .map(|role| {
            Check::failed(
                "model_card",
                &role.to_string(),
                format!("no model card is recorded for the {role}"),
                "the model bake-off records a card for each role (T030, \
                 `maestro eval bakeoff`)",
            )
        })
        .collect()
}

/// Runs `work` to its end on a runtime of its own.
fn block_on<T>(work: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the runtime of a check: {error}"))?
        .block_on(work)
}
