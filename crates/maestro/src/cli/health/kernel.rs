//! The kernel's checks: its configuration files, its database, opened only
//! when it exists and lacks no migration this build carries, then checked
//! whole, and its artifact tree. A check never creates the kernel nor
//! migrates it: a database that lacks a migration is reported, never opened.
//! Once its database opens, `config.toml`'s grants are applied to the local
//! principal, as every command does.

use super::check::Check;
use crate::cli::failure::chain;
use maestro_kernel::{
    binding::{self, Bindings},
    scope::{CONFIG_FILE, Config, LOCAL, ScopeSet},
    store::{self, Database},
};
use std::path::Path;

/// The kernel's database in its data directory.
pub(super) const DATABASE: &str = "kernel.sqlite3";
/// The kernel's artifact tree in its data directory.
pub(super) const ARTIFACTS: &str = "artifacts";

/// The kernel's database, as its check opened it.
#[derive(Debug)]
pub(super) struct Opened {
    /// The database.
    pub(super) database: Database,
    /// What the local principal reads once `config.toml`'s grants are
    /// applied; none without a valid `config.toml`.
    pub(super) scopes: Option<ScopeSet>,
}

/// The check of `config.toml` in `config_dir`, and what it holds when it is
/// valid; a missing file is valid and grants nothing.
pub(super) fn config_check(config_dir: &Path) -> (Check, Option<Config>) {
    let path = config_dir.join(CONFIG_FILE);
    let target = path.display().to_string();
    match Config::load(config_dir) {
        Ok(config) => (Check::passed("config", &target, "valid"), Some(config)),
        Err(error) => (
            Check::failed(
                "config",
                &target,
                chain(&error),
                format!(
                    "fix {target}: it holds only `[access]`, whose `read` lists scopes such \
                     as 'workspace/default'"
                ),
            ),
            None,
        ),
    }
}

/// The check of `bindings.toml` in `config_dir`; a missing file is valid
/// and binds nothing.
pub(super) fn bindings_check(config_dir: &Path) -> Check {
    let target = config_dir.join(binding::FILE_NAME).display().to_string();
    match Bindings::load(config_dir) {
        Ok(_) => Check::passed("bindings", &target, "valid"),
        Err(error) => Check::failed(
            "bindings",
            &target,
            chain(&error),
            format!(
                "fix {target}: each name binds an absolute path, such as \
                 corpus_root = '/srv/corpus'"
            ),
        ),
    }
}

/// The check of the kernel's database in the data directory `data`: it
/// must exist, lack no migration this build carries, open and pass SQLite's
/// quick check, and `config`'s grants, when given, must apply. Returns the
/// database it opened when it passed.
pub(super) fn database_check(data: &Path, config: Option<&Config>) -> (Check, Option<Opened>) {
    let path = data.join(DATABASE);
    let target = path.display().to_string();
    let failed =
        |problem: String, next: String| (Check::failed("database", &target, problem, next), None);
    if !path.exists() {
        return failed(
            "no kernel database yet".to_owned(),
            "add a collection with `maestro knowledge collection add <collection.json>`, \
             which creates it"
                .to_owned(),
        );
    }
    let database = match open(data, &target) {
        Ok(database) => database,
        Err((problem, next)) => return failed(problem, next),
    };
    match database.quick_check() {
        Ok(problems) if problems.is_empty() => {}
        Ok(problems) => {
            return failed(
                format!("the kernel database is damaged: {}", problems.join("; ")),
                format!("restore {target} from a backup: it no longer reads back whole"),
            );
        }
        Err(error) => {
            return failed(
                format!("the kernel database cannot be checked: {}", chain(&error)),
                restore(&target),
            );
        }
    }
    match config.map(|config| apply(&database, config)).transpose() {
        Ok(scopes) => (
            Check::passed("database", &target, "intact"),
            Some(Opened { database, scopes }),
        ),
        Err(error) => failed(
            format!(
                "the grants of config.toml cannot be applied: {}",
                chain(&error)
            ),
            restore(&target),
        ),
    }
}

/// The kernel's database in the data directory `data`, opened only when it
/// lacks no migration this build carries, so that a check never migrates
/// it; or why it was not opened, and the next action, the file being
/// `target`.
fn open(data: &Path, target: &str) -> Result<Database, (String, String)> {
    let pending = store::pending_migrations(data).map_err(|error| unopened(&error, target))?;
    if !pending.is_empty() {
        return Err((
            format!(
                "the kernel database lacks {}, which this maestro applies when a command \
                 opens it",
                pending.join(", ")
            ),
            "any `maestro knowledge` command applies it, or use the maestro that last opened \
             it"
            .to_owned(),
        ));
    }
    Database::open_in(data).map_err(|error| unopened(&error, target))
}

/// Why the kernel's database, the file `target`, cannot be opened, as
/// `error` says, and the next action.
fn unopened(error: &store::Error, target: &str) -> (String, String) {
    match error {
        store::Error::UnknownMigration(_) => (
            error.to_string(),
            "use a maestro as new as the one that migrated it, or a newer maestro: this one \
             would misread its tables"
                .to_owned(),
        ),
        _ => (
            format!("the kernel database cannot be opened: {}", chain(error)),
            restore(target),
        ),
    }
}

/// The next action for the kernel's database, the file `target`, when it
/// cannot be opened, read or written.
fn restore(target: &str) -> String {
    format!(
        "check the permissions of {target} and of its directory, and that the disk has room; \
         restore the file from a backup if it is damaged"
    )
}

/// Applies `config` to the local principal in `database`, and returns what
/// the principal then reads.
fn apply(database: &Database, config: &Config) -> Result<ScopeSet, store::Error> {
    database.apply_config(config)?;
    database.visible(LOCAL)
}

/// The check of the artifact tree in the data directory `data`: each
/// artifact the database `opened` records must be in it, intact.
pub(super) fn artifacts_check(data: &Path, opened: Option<&Opened>) -> Check {
    let target = data.join(ARTIFACTS).display().to_string();
    let first = "fix the kernel database first, as its check says";
    let Some(opened) = opened else {
        return Check::failed(
            "artifacts",
            &target,
            "the artifact tree cannot be checked without the kernel database",
            first,
        );
    };
    match opened.database.check_artifacts() {
        Err(error) => Check::failed(
            "artifacts",
            &target,
            format!("the artifact records cannot be read: {}", chain(&error)),
            first,
        ),
        Ok(check) => match check.missing.first().or_else(|| check.damaged.first()) {
            None => Check::passed(
                "artifacts",
                &target,
                format!("{} artifacts, each intact", check.recorded),
            ),
            Some(broken) => Check::failed(
                "artifacts",
                &target,
                format!(
                    "{} of {} artifacts missing and {} damaged, the first sha256:{}",
                    check.missing.len(),
                    check.recorded,
                    check.damaged.len(),
                    broken.as_str()
                ),
                format!("restore the missing and damaged files of {target} from a backup"),
            ),
        },
    }
}
