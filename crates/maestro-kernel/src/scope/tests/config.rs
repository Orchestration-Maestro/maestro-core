//! `config.toml`: the local principal's grants, checked whole when read, and
//! reconciled with the database in one write that journals each change.

use super::support::{CTM, Scratch, audit, read, scope};
use crate::{
    journal::Event,
    scope::{CONFIG_FILE, Config, ConfigError, LOCAL, Right},
};
use std::{error, fs};

/// A collection the tests grant beside [`CTM`].
const GARDEN: &str = "workspace/default/collection/garden";
/// A source of [`GARDEN`].
const SEEDS: &str = "workspace/default/collection/garden/source/seed-catalog";
/// The type of the event of a grant added.
const ADDED: &str = "maestro.kernel.grant.added.v1";
/// The type of the event of a grant revoked.
const REVOKED: &str = "maestro.kernel.grant.revoked.v1";

/// A `config.toml` that lets the local principal read `paths`.
fn reading(paths: &[&str]) -> String {
    let quoted: Vec<String> = paths.iter().map(|path| format!("'{path}'")).collect();
    format!("[access]\nread = [{}]\n", quoted.join(", "))
}

/// The type, scope and actor of each of `events`.
fn changes(events: &[Event]) -> Vec<(&str, &str, &str)> {
    events
        .iter()
        .map(|event| {
            let actor = event.data["actor"].as_str().unwrap();
            (event.r#type.as_str(), event.scope.as_str(), actor)
        })
        .collect()
}

#[test]
fn the_access_section_lists_the_scopes_the_local_principal_reads() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let config: Config = reading(&[CTM, SEEDS]).parse().unwrap();
    database.apply_config(&config).unwrap();
    let local = database.visible(LOCAL).unwrap();
    assert!(local.covers(&scope(CTM)));
    assert!(local.covers(&scope(SEEDS)));
    assert!(!local.covers(&scope(GARDEN)));
    assert_eq!(LOCAL, "local");
}

#[test]
fn a_missing_or_empty_file_grants_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    assert_eq!(CONFIG_FILE, "config.toml");
    let missing = Config::load(&scratch.0).unwrap();
    assert_eq!(database.apply_config(&missing).unwrap(), []);
    assert!(database.visible(LOCAL).unwrap().is_empty());
    for text in ["", "[access]\n", "[access]\nread = []\n"] {
        scratch.configure(text);
        assert_eq!(Config::load(&scratch.0).unwrap(), missing, "{text:?}");
    }
}

#[test]
fn a_file_removed_revokes_what_it_granted() {
    let scratch = Scratch::new();
    let database = scratch.open();
    scratch.configure(&reading(&[CTM]));
    let granted = database
        .apply_config(&Config::load(&scratch.0).unwrap())
        .unwrap();
    assert_eq!(changes(&granted), [(ADDED, CTM, CONFIG_FILE)]);
    fs::remove_file(scratch.0.join(CONFIG_FILE)).unwrap();
    let revoked = database
        .apply_config(&Config::load(&scratch.0).unwrap())
        .unwrap();
    assert_eq!(changes(&revoked), [(REVOKED, CTM, CONFIG_FILE)]);
    assert!(database.visible(LOCAL).unwrap().is_empty());
}

#[test]
fn loading_journals_each_change_once_and_nothing_the_second_time() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let load = |text: &str| {
        scratch.configure(text);
        database
            .apply_config(&Config::load(&scratch.0).unwrap())
            .unwrap()
    };
    let first = load(&reading(&[GARDEN, CTM]));
    assert_eq!(
        changes(&first),
        [(ADDED, CTM, CONFIG_FILE), (ADDED, GARDEN, CONFIG_FILE)]
    );
    assert_eq!(load(&reading(&[GARDEN, CTM])), []);
    let changed = load(&reading(&[SEEDS, GARDEN]));
    assert_eq!(
        changes(&changed),
        [(ADDED, SEEDS, CONFIG_FILE), (REVOKED, CTM, CONFIG_FILE)]
    );
    let reordered = load(&reading(&[GARDEN, SEEDS, GARDEN]));
    assert_eq!(reordered, [], "order and repetition change nothing");
    let auditor = audit(&database);
    let journal = read(&database, &auditor, "principal/local");
    assert_eq!(journal, [first, changed].concat());
    let local = database.visible(LOCAL).unwrap();
    assert!(local.covers(&scope(GARDEN)));
    assert!(local.covers(&scope(SEEDS)));
    assert!(!local.covers(&scope(CTM)));
}

#[test]
fn loading_leaves_the_grants_of_other_principals_alone() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .grant("agent", &scope(CTM), Right::Read, "operator")
        .unwrap();
    scratch.configure(&reading(&[GARDEN]));
    database
        .apply_config(&Config::load(&scratch.0).unwrap())
        .unwrap();
    assert!(database.visible("agent").unwrap().covers(&scope(CTM)));
    assert!(!database.visible(LOCAL).unwrap().covers(&scope(CTM)));
}

#[test]
fn a_refused_revocation_leaves_the_whole_file_unapplied() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let first = database
        .apply_config(&reading(&[CTM]).parse().unwrap())
        .unwrap();
    scratch
        .outside()
        .execute_batch(
            "CREATE TRIGGER grants_are_kept BEFORE DELETE ON grants
             BEGIN SELECT RAISE(ABORT, 'the test refuses every revocation'); END;",
        )
        .unwrap();
    let refusal = database
        .apply_config(&reading(&[GARDEN]).parse().unwrap())
        .unwrap_err();
    let reason = error::Error::source(&refusal).map(ToString::to_string);
    assert_eq!(reason.as_deref(), Some("the test refuses every revocation"));
    let local = database.visible(LOCAL).unwrap();
    assert!(
        !local.covers(&scope(GARDEN)),
        "the grant of the refused file stays"
    );
    assert!(local.covers(&scope(CTM)));
    let auditor = audit(&database);
    assert_eq!(read(&database, &auditor, "principal/local"), first);
}

#[test]
fn an_unknown_key_is_refused_naming_it_and_the_file() {
    for (text, key) in [
        ("color = 'blue'\n", "color"),
        ("[access]\nwrite = []\n", "access.write"),
        ("[access]\nread = []\n\n[server]\nport = 1\n", "server"),
    ] {
        let refusal = text.parse::<Config>().unwrap_err();
        assert!(
            matches!(&refusal, ConfigError::UnknownKey(found) if found == key),
            "{text}: {refusal:?}"
        );
        assert!(error::Error::source(&refusal).is_none(), "{refusal:?}");
        let message = refusal.to_string();
        assert!(message.contains(CONFIG_FILE), "{message}");
        assert!(message.contains(&format!("`{key}`")), "{message}");
    }
}

#[test]
fn a_scope_off_the_rules_is_refused_naming_it_and_the_file() {
    for path in [
        "workspace/default/project/maestro",
        "workspace/default/collection/CTM",
        "",
    ] {
        let refusal = reading(&[CTM, path]).parse::<Config>().unwrap_err();
        assert!(
            matches!(&refusal, ConfigError::InvalidScope(_)),
            "{path}: {refusal:?}"
        );
        assert!(error::Error::source(&refusal).is_some());
        let message = refusal.to_string();
        assert!(message.contains(CONFIG_FILE), "{message}");
        assert!(message.contains(&format!("`{path}`")), "{message}");
    }
}

#[test]
fn a_file_that_is_not_toml_is_refused_naming_it() {
    for text in [
        "[access\n",
        "access = \n",
        "[access]\nread = []\nread = []\n",
    ] {
        let refusal = text.parse::<Config>().unwrap_err();
        assert!(
            matches!(refusal, ConfigError::NotToml(_)),
            "{text}: {refusal:?}"
        );
        assert!(error::Error::source(&refusal).is_none(), "{refusal:?}");
        let message = refusal.to_string();
        assert!(
            message.starts_with("config.toml is not valid TOML"),
            "{message}"
        );
    }
}

#[test]
fn a_value_of_another_shape_is_refused_naming_its_key() {
    for (text, key) in [
        ("access = 'read'\n", "access"),
        ("[access]\nread = 'workspace/default'\n", "access.read"),
        ("[access]\nread = [7]\n", "access.read"),
        ("[access]\nread = [['workspace/default']]\n", "access.read"),
    ] {
        let refusal = text.parse::<Config>().unwrap_err();
        assert!(
            matches!(&refusal, ConfigError::Shape { key: found, .. } if found == key),
            "{text}: {refusal:?}"
        );
        assert!(error::Error::source(&refusal).is_none(), "{refusal:?}");
        let message = refusal.to_string();
        assert!(message.contains(CONFIG_FILE), "{message}");
        assert!(message.contains(&format!("`{key}`")), "{message}");
    }
}

#[test]
fn a_file_that_cannot_be_read_is_refused_with_its_path() {
    let scratch = Scratch::new();
    let file = scratch.0.join(CONFIG_FILE);
    fs::create_dir(&file).unwrap();
    let refusal = Config::load(&scratch.0).unwrap_err();
    assert!(
        matches!(&refusal, ConfigError::Io { path, .. } if *path == file),
        "{refusal:?}"
    );
    assert!(error::Error::source(&refusal).is_some());
    assert!(refusal.to_string().contains(CONFIG_FILE), "{refusal}");
}
