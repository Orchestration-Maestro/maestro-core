//! CLI setup, scoped execution and exit-code mapping for search.

use super::{
    super::{health, output::Output},
    presentation::{CliError, deadline_error, search_document, search_text, write_error},
};
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::{
        RequestError, SearchRequest,
        operations::{KnowledgeError, ensure_current_scopes, search_with},
    },
    settings::KnowledgeSettings,
};
use maestro_kernel::{evidence::Bundle, gateway::RouterClient, scope::ScopeSet, store::Database};
use maestro_knowledge::{
    index::Qdrant,
    search::{SourceClassifier, classify_revision},
};
use std::{collections::BTreeMap, env, process::ExitCode};
use tokio::{runtime::Builder, time::Instant};

/// Builds the configured model-router and Qdrant clients without contacting either service.
pub(in crate::cli) fn ports() -> Result<(RouterClient, Qdrant), Failure> {
    let router_url = health::router_url(env::var_os(health::ROUTER_VARIABLE).as_deref())
        .map_err(|_| Failure::refused(format!("{} is not a URL", health::ROUTER_VARIABLE)))?;
    let router = RouterClient::new(router_url).map_err(|error| Failure::failed_by(&error))?;
    let qdrant_url = health::qdrant_url(env::var_os(health::QDRANT_VARIABLE).as_deref());
    let qdrant = Qdrant::new(&qdrant_url).map_err(|error| Failure::failed_by(&error))?;
    Ok((router, qdrant))
}

/// Executes one search under `settings` and prints the same bundle returned
/// by MCP.
pub(in crate::cli) fn run(
    output: Output,
    request: &SearchRequest,
    settings: &KnowledgeSettings,
) -> Result<ExitCode, Failure> {
    let kernel = match Kernel::open() {
        Ok(kernel) => kernel,
        Err(Failure::Refused(_)) => {
            return write_error(
                output,
                CliError {
                    code: "invalid_configuration",
                    message: "local access configuration is invalid",
                },
                2,
            );
        }
        Err(Failure::Failed(_)) => {
            return write_error(
                output,
                CliError {
                    code: "kernel_unavailable",
                    message: "the local knowledge store is unavailable",
                },
                1,
            );
        }
    };
    let (model_port, qdrant) = match ports() {
        Ok(ports) => ports,
        Err(Failure::Refused(_)) => {
            return write_error(
                output,
                CliError {
                    code: "invalid_configuration",
                    message: "search service configuration is invalid",
                },
                2,
            );
        }
        Err(Failure::Failed(_)) => {
            return write_error(
                output,
                CliError {
                    code: "search_service_unavailable",
                    message: "the search services could not be configured",
                },
                1,
            );
        }
    };
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::failed("could not start the search runtime"))?;
    match runtime.block_on(search_with(kernel, request, settings, &model_port, &qdrant)) {
        Ok(mut scoped) => {
            let deadline = scoped.data.deadline;
            if Instant::now() >= deadline {
                return deadline_error(output);
            }
            let document = search_document(&scoped.data.bundle).map_err(knowledge_failure)?;
            if Instant::now() >= deadline {
                return deadline_error(output);
            }
            let labels = if output.is_json() {
                BTreeMap::new()
            } else {
                passage_labels(
                    scoped.data.source_classes.as_deref(),
                    &scoped.data.bundle,
                    &scoped.kernel.database,
                    &scoped.scopes,
                )
            };
            let text = search_text(&scoped.data.bundle, &document, &labels);
            ensure_current_scopes(&mut scoped.kernel, &scoped.scopes).map_err(knowledge_failure)?;
            if Instant::now() >= deadline {
                return deadline_error(output);
            }
            if document.error.is_some() {
                output.refusal(
                    &document,
                    "response_too_large: search response is too large",
                )?;
                Ok(ExitCode::from(1))
            } else {
                output.result(&document, &text)?;
                Ok(ExitCode::SUCCESS)
            }
        }
        Err(error) => {
            let (code, message, exit) = error_parts(error);
            write_error(output, CliError { code, message }, exit)
        }
    }
}

/// Writes a strict CLI refusal before any kernel or model work.
pub(in crate::cli) fn invalid_request(
    output: Output,
    error: RequestError,
) -> Result<ExitCode, Failure> {
    write_error(
        output,
        CliError {
            code: error.code(),
            message: error.message(),
        },
        2,
    )
}

/// Maps application failures to the stable CLI error tuple.
fn error_parts(error: KnowledgeError) -> (&'static str, &'static str, u8) {
    match error {
        KnowledgeError::Refused { code, message } => (code, message, 2),
        KnowledgeError::Failed { code, message } => (code, message, 1),
    }
}

/// Converts a typed operation failure into the CLI's diagnostic boundary.
fn knowledge_failure(error: KnowledgeError) -> Failure {
    match error {
        KnowledgeError::Refused { message, .. } => Failure::refused(message),
        KnowledgeError::Failed { message, .. } => Failure::failed(message),
    }
}

/// The source label of each classified passage document of `bundle`, by
/// id, read through `scopes`; none without a source classifier.
pub(super) fn passage_labels(
    classifier: Option<&dyn SourceClassifier>,
    bundle: &Bundle,
    database: &Database,
    scopes: &ScopeSet,
) -> BTreeMap<String, String> {
    let Some(classifier) = classifier else {
        return BTreeMap::new();
    };
    bundle
        .passages
        .iter()
        .filter_map(|passage| {
            classify_revision(classifier, database, scopes, &passage.revision_id)
                .map(|found| (passage.document_id.clone(), found.label))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CliError, KnowledgeError, Output, error_parts, write_error};
    use serde_json::Value;
    use std::{
        env,
        process::{Command, ExitCode},
    };

    #[test]
    fn search_cli_failures_deadline_errors_preserve_exit_codes() {
        const CHILD: &str = "MAESTRO_TEST_SEARCH_DEADLINE_CHILD";
        if env::var_os(CHILD).is_some() {
            let (code, message, exit) = error_parts(KnowledgeError::Failed {
                code: "deadline_exceeded",
                message: "search exceeded its accepted deadline",
            });
            println!(); // Separate the document from the test harness's prefix.
            let status = write_error(Output::new(true), CliError { code, message }, exit)
                .expect("deadline output");
            assert_eq!(status, ExitCode::from(1));
            return;
        }

        // Capture the real JSON writer in a separate process, without replacing
        // stdout or depending on a search completing before a wall-clock cutoff.
        let output = Command::new(env::current_exe().unwrap())
            .args([
                concat!(
                    "cli::search::execution::tests::",
                    "search_cli_failures_deadline_errors_preserve_exit_codes"
                ),
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let documents = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect::<Vec<_>>();
        assert_eq!(documents.len(), 1, "{stdout}");
        assert_eq!(documents[0]["error"]["code"], "deadline_exceeded");
        assert_eq!(documents[0]["schema"], "maestro-cli/knowledge-search/1");
    }
}
