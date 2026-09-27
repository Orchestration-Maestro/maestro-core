//! Running the command line: its arguments parsed, the kernel opened for the
//! local principal, the command run, and its exit code, 0 when it is done, 1
//! when the operation failed and 2 for a usage error or a refused input.

use super::{
    args::{Arguments, CollectionCommand, JobCommand, KnowledgeCommand, Noun},
    backup, collection, health, import,
    output::{Output, diagnose},
    prepare, publish, quality, retrieve, setup, status, verify, wait,
};
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::{GetRequest, RequestError},
    mcp::run::run as run_mcp,
};
use clap::Parser as _;
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

/// Runs the command `arguments` name; `setup`, `backup` and `restore` open no
/// kernel for writing, and `status` and `doctor` never create or migrate it.
fn dispatch(arguments: &Arguments, output: Output) -> Result<ExitCode, Failure> {
    match &arguments.noun {
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
        Noun::Knowledge(command) => knowledge(&Kernel::open()?, output, command),
        Noun::Mcp => {
            run_mcp()?;
            Ok(ExitCode::SUCCESS)
        }
        Noun::Job(JobCommand::Wait { id }) => wait::run(&Kernel::open()?, output, *id),
        Noun::Setup { yes } => setup::run(output, *yes),
        Noun::Status => health::status::run(output),
        Noun::Doctor => health::doctor::run(output),
        Noun::Backup { to } => backup::run_backup(output, to),
        Noun::Restore { from } => backup::run_restore(output, from),
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
        KnowledgeCommand::Import { collection, again } => {
            import::run(kernel, output, collection, *again)
        }
        KnowledgeCommand::Quality { collection } => quality::run(kernel, output, collection),
        KnowledgeCommand::Prepare { collection, card } => {
            prepare::run(kernel, output, collection, card)
        }
        KnowledgeCommand::Publish { arguments } => publish::run(kernel, output, arguments),
        KnowledgeCommand::Verify { collection } => verify::run(kernel, output, collection),
        KnowledgeCommand::Status { collection } => status::run(kernel, output, collection),
        KnowledgeCommand::Collections | KnowledgeCommand::Get { .. } => Err(Failure::failed(
            "knowledge retrieval bypassed its scoped dispatch path",
        )),
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
