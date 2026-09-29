//! Running the command line: its arguments parsed, the kernel opened for the
//! local principal, the command run, and its exit code, 0 when it is done, 1
//! when the operation failed and 2 for a usage error or a refused input.

use super::{
    args::{
        Arguments, CollectionCommand, ConfigCommand, EvalCommand, GraphCommand, GraphEvalCommand,
        JobCommand, KnowledgeCommand, Noun, PublishArguments, Target,
    },
    ask, backup, collection,
    config::{self, Change, Places},
    eval, graph, health, import, model,
    output::{Output, diagnose},
    prepare, publish, quality, retrieve, search, setup, status, verify, wait,
};
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::{GetRequest, RequestError, SearchRequest, operations::KnowledgeError},
    mcp::run::run as run_mcp,
    settings::{Compute, KnowledgeSettings, Session},
};
use clap::Parser as _;
use maestro_knowledge::answer::{AskBudget, AskRequest};
use maestro_settings::{LayerName, Registry, parse_flags};
use std::process::ExitCode;

/// Runs the command the process's arguments name, and returns its exit code.
pub(crate) fn main() -> ExitCode {
    match Arguments::try_parse() {
        Ok(arguments) => run(&arguments),
        Err(error) => usage(&error),
    }
}

/// Prints what clap has to say, help or a usage error, and returns its exit
/// code: 0 for help and the version, 2 for a usage error.
fn usage(error: &clap::Error) -> ExitCode {
    // Help goes to stdout and a usage error to stderr; neither has
    // anywhere else to go if its stream is gone.
    drop(error.print());
    u8::try_from(error.exit_code()).map_or(ExitCode::from(2), ExitCode::from)
}

/// Runs the command `arguments` name, and prints why it stopped short if it
/// did.
fn run(arguments: &Arguments) -> ExitCode {
    let output = Output::new(arguments.json);
    match dispatch(arguments, output) {
        Ok(code) => code,
        Err(failure) => {
            diagnose(&failure.to_string());
            failure.code()
        }
    }
}

/// Runs the command `arguments` name, once its `--set` flags are checked;
/// `setup`, `backup` and `restore` open no kernel for writing, and `status`
/// and `doctor` never create or migrate it.
fn dispatch(arguments: &Arguments, output: Output) -> Result<ExitCode, Failure> {
    let registry = Registry::built_in().map_err(|error| Failure::failed_by(&error))?;
    parse_flags(&registry, &arguments.set).map_err(|error| Failure::refused_by(&error))?;
    match &arguments.noun {
        Noun::Model(command) => model::run(&Kernel::open()?, output, command),
        Noun::Knowledge(KnowledgeCommand::Collections) => {
            retrieve::collections(output, Kernel::open)
        }
        Noun::Knowledge(KnowledgeCommand::Get {
            chunk_id,
            section_id,
            collection,
            generation,
        }) => get_command(
            output,
            chunk_id.as_deref(),
            section_id.as_deref(),
            collection.as_deref(),
            *generation,
        ),
        Noun::Knowledge(
            command @ (KnowledgeCommand::Search { .. } | KnowledgeCommand::Ask { .. }),
        ) => {
            let settings = Session::for_cli(&arguments.set)?.knowledge()?;
            retrieval(output, command, &settings)
        }
        Noun::Knowledge(
            command @ (KnowledgeCommand::Prepare { .. } | KnowledgeCommand::Publish { .. }),
        ) => {
            let settings = Session::for_cli(&arguments.set)?.knowledge()?;
            modelled(output, command, &settings, Kernel::open)
        }
        Noun::Knowledge(command) => knowledge(&Kernel::open()?, output, command),
        Noun::Mcp { workspace } => {
            let settings = Session::for_mcp(workspace.as_deref(), &arguments.set)?.knowledge()?;
            let (model_port, qdrant) = search::ports()?;
            run_mcp(model_port, qdrant, settings)?;
            Ok(ExitCode::SUCCESS)
        }
        Noun::Config(command) => config_command(output, command, &arguments.set),
        Noun::Eval(EvalCommand::Ladder { manifest }) => eval::run(output, manifest),
        Noun::Eval(EvalCommand::Graph(GraphEvalCommand::Draft { manifest })) => {
            eval::draft_graph(output, manifest)
        }
        Noun::Eval(EvalCommand::Graph(GraphEvalCommand::Check { manifest })) => {
            eval::check_graph(output, manifest)
        }
        Noun::Job(JobCommand::Wait { id }) => wait::run(&Kernel::open()?, output, *id),
        Noun::Setup { yes } => setup::run(output, *yes),
        Noun::Status => health::status::run(output),
        Noun::Doctor => health::doctor::run(output, &arguments.set),
        Noun::Backup { to } => backup::run_backup(output, to),
        Noun::Restore { from } => backup::run_restore(output, from),
    }
}

/// Runs `knowledge search` or `knowledge ask` under `settings`: a flag the
/// command line gives wins for its own request, and the settings give the
/// rest.
fn retrieval(
    output: Output,
    command: &KnowledgeCommand,
    settings: &KnowledgeSettings,
) -> Result<ExitCode, Failure> {
    match command {
        KnowledgeCommand::Search {
            collection,
            query,
            version,
            max_passages,
            evidence_bytes,
            deadline_ms,
        } => {
            let defaults = settings.search_budget;
            let request = SearchRequest {
                collection: collection.clone(),
                query: query.clone(),
                version: version.clone(),
                max_passages: max_passages.unwrap_or(defaults.k),
                evidence_bytes: evidence_bytes.unwrap_or(defaults.evidence_bytes),
                deadline_ms: deadline_ms.unwrap_or(defaults.deadline_ms),
            };
            match SearchRequest::from_cli(request) {
                Ok(request) => search::run(output, &request, settings),
                Err(error) => search::invalid_request(output, error),
            }
        }
        KnowledgeCommand::Ask {
            collection,
            question,
            model,
            version,
            k: max_passages,
            evidence_bytes,
            search_deadline_ms,
            output_tokens,
            explain,
        } => {
            let defaults = settings.ask_budget;
            let request = AskRequest {
                collection: collection.clone(),
                question: question.clone(),
                model: model.clone().unwrap_or_else(|| settings.model.clone()),
                version: version.clone(),
                budget: AskBudget {
                    k: max_passages.unwrap_or(defaults.k),
                    evidence_bytes: evidence_bytes.unwrap_or(defaults.evidence_bytes),
                    search_deadline_ms: search_deadline_ms.unwrap_or(defaults.search_deadline_ms),
                    output_tokens: output_tokens.or(defaults.output_tokens),
                },
            };
            ask::run(output, &request, *explain, settings, Kernel::open)
        }
        _ => Err(Failure::failed(
            "knowledge retrieval bypassed its scoped dispatch path",
        )),
    }
}

/// Runs `knowledge prepare` or `knowledge publish` under `settings`, in the
/// kernel `open_kernel` opens; refused with `models_off`, before the kernel
/// opens, when models are off: both call the model router.
fn modelled(
    output: Output,
    command: &KnowledgeCommand,
    settings: &KnowledgeSettings,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let (name, message) = if matches!(command, KnowledgeCommand::Prepare { .. }) {
        (
            "prepare",
            "models.compute is off: prepare calls the model router",
        )
    } else {
        (
            "publish",
            "models.compute is off: publish calls the model router",
        )
    };
    if settings.compute == Compute::Off {
        let code = "models_off";
        let error = KnowledgeError::Refused { code, message };
        return ask::refusal(
            output,
            &format!("maestro-cli/knowledge-{name}-error/1"),
            error,
        );
    }
    let kernel = open_kernel()?;
    match command {
        KnowledgeCommand::Prepare {
            collection,
            card,
            chunk_profile,
        } => {
            let profile = match chunk_profile.as_deref() {
                Some(profile) => profile.to_owned(),
                None => {
                    prepare::profile_for_collection(&kernel, collection, settings.chunk_profile)?
                        .chunker_version()
                        .to_owned()
                }
            };
            prepare::run(&kernel, output, collection, card, Some(&profile))
        }
        KnowledgeCommand::Publish { arguments } => {
            let profile = if arguments.chunk_set.is_none() {
                Some(match arguments.chunk_profile.as_deref() {
                    Some(profile) => profile.to_owned(),
                    None => prepare::profile_for_collection(
                        &kernel,
                        &arguments.collection,
                        settings.chunk_profile,
                    )?
                    .chunker_version()
                    .to_owned(),
                })
            } else {
                arguments.chunk_profile.clone()
            };
            let arguments = PublishArguments {
                collection: arguments.collection.clone(),
                card: arguments.card.clone(),
                chunk_set: arguments.chunk_set.clone(),
                chunk_profile: profile,
                again: arguments.again,
            };
            publish::run(&kernel, output, &arguments)
        }
        _ => Err(Failure::failed(
            "a model command bypassed its dispatch path",
        )),
    }
}

/// Runs the knowledge command `command` in `kernel`.
fn knowledge(
    kernel: &Kernel,
    output: Output,
    command: &KnowledgeCommand,
) -> Result<ExitCode, Failure> {
    match command {
        KnowledgeCommand::Collection(CollectionCommand::Add { declaration }) => {
            collection::add(kernel, output, declaration)
        }
        KnowledgeCommand::Graph(GraphCommand::Build { arguments }) => {
            graph::build::run(kernel, output, arguments)
        }
        KnowledgeCommand::Graph(GraphCommand::Attach { build, generation }) => {
            graph::attach::run(kernel, output, *build, *generation)
        }
        KnowledgeCommand::Import { collection, again } => {
            import::run(kernel, output, collection, *again)
        }
        KnowledgeCommand::Quality { collection } => quality::run(kernel, output, collection),
        KnowledgeCommand::Verify { collection } => verify::run(kernel, output, collection),
        KnowledgeCommand::Status { collection } => status::run(kernel, output, collection),
        KnowledgeCommand::Collections
        | KnowledgeCommand::Get { .. }
        | KnowledgeCommand::Search { .. }
        | KnowledgeCommand::Ask { .. }
        | KnowledgeCommand::Prepare { .. }
        | KnowledgeCommand::Publish { .. } => Err(Failure::failed(
            "knowledge retrieval bypassed its scoped dispatch path",
        )),
    }
}

/// Runs the settings command `command`, with the `--set` flags `flags`.
fn config_command(
    output: Output,
    command: &ConfigCommand,
    flags: &[String],
) -> Result<ExitCode, Failure> {
    match command {
        ConfigCommand::Get { key } => config::get(output, &Session::for_cli(flags)?, key),
        ConfigCommand::List => config::list(output, &Session::for_cli(flags)?),
        ConfigCommand::Explain { key } => {
            config::explain(output, &Session::for_cli(flags)?, key.as_deref())
        }
        ConfigCommand::Set { key, value, target } => {
            let change = Change {
                key,
                value: Some(value),
                layer: layer(target),
            };
            config::change(output, change, &Places::current()?, Kernel::open)
        }
        ConfigCommand::Unset { key, target } => {
            let change = Change {
                key,
                value: None,
                layer: layer(target),
            };
            config::change(output, change, &Places::current()?, Kernel::open)
        }
        ConfigCommand::History => config::history(output, Kernel::open),
    }
}

/// The file `target` names: the user file unless --project.
const fn layer(target: &Target) -> LayerName {
    if target.project {
        LayerName::Project
    } else {
        LayerName::User
    }
}

/// Reads the selected exact chunk or canonical section from its source.
fn get_command(
    output: Output,
    chunk_id: Option<&str>,
    section_id: Option<&str>,
    collection: Option<&str>,
    generation: Option<i64>,
) -> Result<ExitCode, Failure> {
    let request = GetRequest::from_cli(
        chunk_id.map(str::to_owned),
        section_id.map(str::to_owned),
        collection.map(str::to_owned),
        generation,
    )
    .map_err(|error: RequestError| Failure::refused(error.message()))?;
    retrieve::get_exact(output, &request, Kernel::open)
}
