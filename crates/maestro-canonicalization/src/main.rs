//! Local CLI for Markdown canonicalization. No network clients or model calls.
//! Exit codes: 0 usable, 2 retained but invalid, 1 execution or input refusal.
#![forbid(unsafe_code)]
mod cli;
mod local_assets;

use cli::{USAGE, parse_args, run};
use maestro_canonicalization::ValidationStatus;
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse_args(&args).and_then(|args| run(&args)) {
        Ok(status) => {
            if status == ValidationStatus::Failed {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
