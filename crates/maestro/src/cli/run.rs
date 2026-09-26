//! Running the command line: its arguments parsed, the kernel opened for the
//! local principal, the command run, and its exit code, 0 when it is done, 1
//! when the operation failed and 2 for a usage error or a refused input.

use super::{
    args::{Arguments, CollectionCommand, JobCommand, KnowledgeCommand, Noun},
    collection,
    failure::Failure,
    health, import,
    kernel::Kernel,
    output::{Output, diagnose},
    quality, setup, status, wait,
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

/// Runs the command `arguments` name, in the kernel it opens; `setup`,
/// `status` and `doctor` open none, since the first installs beside the
/// kernel and the others look at it without creating or migrating it.
fn dispatch(arguments: &Arguments, output: Output) -> Result<ExitCode, Failure> {
    match &arguments.noun {
        Noun::Knowledge(command) => knowledge(&Kernel::open()?, output, command),
        Noun::Job(JobCommand::Wait { id }) => wait::run(&Kernel::open()?, output, *id),
        Noun::Setup { yes } => setup::run(output, *yes),
        Noun::Status => health::status::run(output),
        Noun::Doctor => health::doctor::run(output),
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
        KnowledgeCommand::Import { collection } => import::run(kernel, output, collection),
        KnowledgeCommand::Quality { collection } => quality::run(kernel, output, collection),
        KnowledgeCommand::Status { collection } => status::run(kernel, output, collection),
    }
}
