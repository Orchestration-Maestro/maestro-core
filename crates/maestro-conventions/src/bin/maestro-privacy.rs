//! CLI for a private-content fingerprint bank and Git-object scan.

use maestro_conventions::privacy::run;
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if let Ok((code, output)) = run(&arguments) {
        print!("{output}");
        ExitCode::from(code)
    } else {
        eprintln!("privacy operation unavailable; refusing");
        ExitCode::from(2)
    }
}
