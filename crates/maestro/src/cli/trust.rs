//! Explicit user-local trust administration, independent of preference discovery.
use super::{output::Output, trust_path};
use crate::failure::Failure;
use clap::Subcommand;
use maestro_catalog::policy::workspace::{TrustBoundaries, confirmation};
use maestro_kernel::{
    paths::{self, Environment},
    store::Database,
    workspace::{Answer, Confirmation, WorkspaceAnswer},
};
use std::{
    env,
    io::{self, BufRead, IsTerminal as _, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

/// User-only workspace trust commands; never registered as MCP tools.
#[derive(Debug, Subcommand)]
pub(super) enum TrustCommand {
    /// Approve an exact canonical directory, with default-no user confirmation.
    Add {
        /// Directory selected explicitly by the user.
        directory: PathBuf,
        /// Repeat the exact canonical absolute path when no terminal is available.
        #[arg(long, value_name = "DIR")]
        confirm_path: Option<PathBuf>,
    },
    /// List approved canonical roots and their journal receipts, not preference files.
    List,
    /// Revoke this root for subsequent controlled effects; preferences stay untouched.
    Remove {
        /// Directory whose approval is revoked.
        directory: PathBuf,
    },
}

/// Platform-resolved mandatory root refusals; HOME never receives an implicit grant.
pub(super) fn boundaries() -> Result<TrustBoundaries, Failure> {
    let environment = Environment::current();
    let home = env::home_dir().ok_or_else(|| Failure::refused("HOME cannot be resolved"))?;
    let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let config = paths::config_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    TrustBoundaries::new(&home, &[data, config]).map_err(|error| Failure::failed_by(&error))
}

/// Open only user-local kernel authority; no authority config or preferences are parsed.
pub(super) fn database() -> Result<Database, Failure> {
    let data =
        paths::data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    Database::open_in(&data).map_err(|error| Failure::failed_by(&error))
}

/// Confirm through trusted terminal IO or the exact repeated canonical path.
pub(super) fn approve(
    root: &Path,
    path: Option<&Path>,
    prompt: &str,
) -> Result<Option<Confirmation>, Failure> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    approve_with_io(
        root,
        path,
        prompt,
        stdin.is_terminal() && stderr.is_terminal(),
        (&mut stdin.lock(), &mut stderr.lock()),
    )
}

/// Shared trusted IO boundary; the caller renders the prompt before approval.
pub(super) fn approve_with_io(
    root: &Path,
    path: Option<&Path>,
    prompt: &str,
    terminal: bool,
    io: (&mut dyn BufRead, &mut dyn Write),
) -> Result<Option<Confirmation>, Failure> {
    let visible = PathBuf::from(trust_path::visible_path(root));
    let canonical = if path.is_some_and(|path| path.as_os_str() == root.as_os_str()) {
        root
    } else {
        visible.as_path()
    };
    confirmation(canonical, path, terminal, prompt, io).map_err(Failure::refused)
}

/// Run administration without reading or rewriting any workspace preference bytes.
pub(super) fn run(output: Output, command: &TrustCommand) -> Result<ExitCode, Failure> {
    let (directory, answer) = match command {
        TrustCommand::Add {
            directory,
            confirm_path,
        } => {
            let root = boundaries()?
                .canonical_root(directory)
                .map_err(Failure::refused)?;
            let prompt = format!("Approve {}? [y/N] ", trust_path::visible_path(&root));
            let answer =
                match approve(&root, confirm_path.as_deref(), &prompt).map_err(|failure| {
                    Failure::refused(format!("{failure}; {}", trust_path::suggestion(&root)))
                })? {
                    Some(confirmation) => Answer::Approved { confirmation },
                    None => Answer::Declined,
                };
            (root, answer)
        }
        TrustCommand::Remove { directory } => (
            directory
                .canonicalize()
                .map_err(|error| Failure::refused_by(&error))?,
            Answer::Removed,
        ),
        TrustCommand::List => {
            let data = paths::data_dir(&Environment::current())
                .map_err(|error| Failure::failed_by(&error))?;
            let records = if data.join("kernel.sqlite3").exists() {
                Database::open_read_only_in(&data)
                    .map_err(Failure::refused)?
                    .trusted_workspaces()
                    .map_err(Failure::refused)?
            } else {
                Vec::new()
            };
            let text = serde_json::to_string_pretty(&records)
                .map_err(|error| Failure::failed_by(&error))?;
            output.result(&records, &text)?;
            return Ok(ExitCode::SUCCESS);
        }
    };
    let record = database()?
        .record_workspace_answer(&WorkspaceAnswer {
            path: directory,
            answer,
        })
        .map_err(Failure::refused)?;
    output.result(
        &record,
        &format!(
            "Recorded workspace answer for {}.",
            record.change.path.display()
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::approve_with_io;
    use crate::{cli::output::Output, presentation::messages::MessageKey};
    use maestro_kernel::workspace::Confirmation;
    use std::path::Path;

    #[test]
    fn terminal_preferences_prompt_is_localized_and_default_no() {
        let root = Path::new("/synthetic/workspace");
        for (language, expected) in [
            ("fr", "Approuver /synthetic/workspace ? [y/N] "),
            ("es", "¿Aprobar /synthetic/workspace? [y/N] "),
        ] {
            for (answer, expected_confirmation) in [
                ("yes\n", Some(Confirmation::Terminal)),
                ("no\n", None),
                ("\n", None),
            ] {
                let output = Output::new(false).with_language(language).unwrap();
                let prompt = output
                    .wording(
                        MessageKey::InitApprovePrompt,
                        &[("path", root.to_str().unwrap())],
                    )
                    .unwrap();
                let mut rendered = Vec::new();
                let actual = approve_with_io(
                    root,
                    None,
                    &prompt,
                    true,
                    (&mut answer.as_bytes(), &mut rendered),
                )
                .unwrap();
                assert_eq!(actual, expected_confirmation);
                assert_eq!(String::from_utf8(rendered).unwrap(), expected);
            }
        }
    }

    #[test]
    fn terminal_trust_prompt_stays_english_and_nonterminal_never_prompts() {
        let root = Path::new("/synthetic/workspace");
        for terminal in [false, true] {
            let mut rendered = Vec::new();
            let actual = approve_with_io(
                root,
                None,
                "Approve /synthetic/workspace? [y/N] ",
                terminal,
                (&mut "yes\n".as_bytes(), &mut rendered),
            );
            if terminal {
                assert_eq!(actual.unwrap(), Some(Confirmation::Terminal));
                assert_eq!(
                    String::from_utf8(rendered).unwrap(),
                    "Approve /synthetic/workspace? [y/N] "
                );
            } else {
                assert!(actual.is_err());
                assert!(rendered.is_empty());
            }
        }
    }
}
