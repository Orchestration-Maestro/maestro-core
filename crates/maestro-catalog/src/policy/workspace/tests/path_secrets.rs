//! Secret-data, platform-binding and strict-schema path contracts.
use super::{Access, Environment, Fixture, Path, TrustBoundaries, fs};

#[test]
fn secret_classes_deny_even_inside_trust_with_readable_neighbours() {
    let mut fixture = Fixture::new();
    // Trust a container of a synthetic HOME, never HOME itself.
    let home = fixture.project.join("user-home");
    fs::create_dir(&home).unwrap();
    fixture.boundaries = TrustBoundaries::new(&home, &[fixture.home.join("kernel")]).unwrap();
    for (secret, neighbour) in [
        (".ssh/id_ed25519", "ssh-notes"),
        (".gnupg/private-keys-v1.d/key", "gpg-notes"),
        (".aws/credentials", ".aws-notes/credentials"),
        (".azure/accessTokens.json", "azure-notes"),
        (".kube/config", ".kube/notes"),
        (".docker/config.json", ".docker/notes"),
        (
            ".config/gcloud/credentials.db",
            ".config/gcloud-notes/notes",
        ),
        (".config/gh/hosts.yml", ".config/gh/notes"),
        (".netrc", "netrc-notes"),
        (".password-store/key.gpg", "password-notes"),
        (
            ".local/share/keyrings/login.keyring",
            ".local/share/keyrings-notes/notes",
        ),
        (".env", "env"),
        ("nested/.env.production", "nested/.environment"),
    ] {
        for name in [secret, neighbour] {
            let path = home.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"unchanged").unwrap();
        }
        for access in [Access::Read, Access::Write] {
            assert!(
                fixture
                    .trust()
                    .authorize(
                        &fixture.project,
                        &Path::new("user-home").join(secret),
                        access
                    )
                    .is_err(),
                "{secret}"
            );
            assert!(
                fixture
                    .trust()
                    .authorize(
                        &fixture.project,
                        &Path::new("user-home").join(neighbour),
                        access
                    )
                    .is_ok(),
                "{neighbour}"
            );
        }
    }
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project,
                Path::new("user-home/.ssh/new"),
                Access::Write
            )
            .is_err()
    );
}

#[test]
fn platform_bindings_map_home_xdg_and_windows_locations_without_environment_mutation() {
    let mut fixture = Fixture::new();
    let mut environment = Environment::default();
    environment.xdg_config_home = Some(fixture.project.join("config").into_os_string());
    environment.xdg_data_home = Some(fixture.project.join("data").into_os_string());
    environment.app_data = Some(fixture.project.join("roaming").into_os_string());
    environment.local_app_data = Some(fixture.project.join("local").into_os_string());
    fixture.boundaries =
        TrustBoundaries::with_environment(&fixture.home, &[], &environment).unwrap();
    for (secret, neighbour) in [
        ("config/gh/hosts.yml", "config/gh/notes"),
        ("data/keyrings/login", "data/keyrings-notes/login"),
        ("roaming/gnupg/private", "roaming/gnupg-notes/private"),
        (
            "local/Microsoft/Credentials/private",
            "local/Microsoft/Credentials-notes/private",
        ),
    ] {
        fixture.file(&format!("project/{secret}"));
        fixture.file(&format!("project/{neighbour}"));
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new(secret), Access::Read)
                .is_err(),
            "{secret}"
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new(neighbour), Access::Read)
                .is_ok(),
            "{neighbour}"
        );
    }
}

#[test]
fn strict_builtin_data_refuses_unknown_fields_and_invalid_patterns() {
    use crate::policy::workspace::deny::Rules;
    assert!(
        Rules::parse(
            r#"{"schema":"maestro-secret-paths/1","paths":[],"names":[],"override":true}"#
        )
        .is_err()
    );
    assert!(
        Rules::parse(
            r#"{"schema":"maestro-secret-paths/1",
                "paths":[{"base":"home","path":".ssh","unknown":true}],"names":[]}"#
        )
        .is_err()
    );
    for path in ["../secret", "/secret", "", "foo/*/bar"] {
        assert!(
            Rules::parse(&format!(
                r#"{{"schema":"maestro-secret-paths/1",
                    "paths":[{{"base":"home","path":"{path}"}}],"names":[]}}"#
            ))
            .is_err()
        );
    }
    assert!(
        Rules::parse(r#"{"schema":"maestro-secret-paths/1","paths":[],"names":["x*y"]}"#).is_err()
    );
    assert!(
        Rules::parse(
            r#"{"schema":"maestro-secret-paths/1",
                "paths":[{"base":"home","path":"synthetic-secret"}],"names":["synthetic.*"]}"#
        )
        .is_ok()
    );
}

#[test]
fn case_variants_and_directory_ancestors_keep_secret_denies() {
    let mut fixture = Fixture::new();
    let home = fixture.project.join("user-home");
    fs::create_dir(&home).unwrap();
    fixture.boundaries = TrustBoundaries::new(&home, &[]).unwrap();
    fixture.file("project/user-home/.SSH/key");
    fixture.file("project/user-home/.ENV.PRODUCTION");
    fixture.file("project/user-home/.env.directory/plain");
    fixture.file("project/user-home/ssh-notes/key");
    for path in [
        "user-home/.SSH/key",
        "user-home/.ENV.PRODUCTION",
        "user-home/.env.directory/plain",
    ] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new(path), Access::Read)
                .is_err()
        );
    }
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project,
                Path::new("user-home/ssh-notes/key"),
                Access::Read
            )
            .is_ok()
    );
}

#[test]
fn every_checked_location_has_a_denied_leaf_and_an_allowed_neighbour() {
    use serde_json::Value;
    let mut fixture = Fixture::new();
    let home = fixture.project.join("user-home");
    fs::create_dir(&home).unwrap();
    let mut environment = Environment::default();
    environment.xdg_config_home = Some(fixture.project.join("config").into_os_string());
    environment.xdg_data_home = Some(fixture.project.join("data").into_os_string());
    environment.app_data = Some(fixture.project.join("roaming").into_os_string());
    environment.local_app_data = Some(fixture.project.join("local").into_os_string());
    fixture.boundaries = TrustBoundaries::with_environment(&home, &[], &environment).unwrap();
    let data: Value =
        serde_json::from_str(include_str!("../../../../resources/secret-paths.json")).unwrap();
    for location in data["paths"].as_array().unwrap() {
        let binding = location["base"].as_str().unwrap();
        let base = if binding == "home" {
            "user-home"
        } else {
            binding
        };
        let secret = Path::new(base).join(location["path"].as_str().unwrap());
        let neighbour = secret.with_file_name(format!(
            "{}-notes",
            secret.file_name().unwrap().to_str().unwrap()
        ));
        for path in [&secret, &neighbour] {
            let named = fixture.project.join(path);
            fs::create_dir_all(named.parent().unwrap()).unwrap();
            fs::write(named, b"unchanged").unwrap();
        }
        for access in [Access::Read, Access::Write] {
            assert!(
                fixture
                    .trust()
                    .authorize(&fixture.project, &secret, access)
                    .is_err(),
                "{secret:?}"
            );
            assert!(
                fixture
                    .trust()
                    .authorize(&fixture.project, &neighbour, access)
                    .is_ok(),
                "{neighbour:?}"
            );
        }
    }
}

#[test]
fn deny_data_requires_the_supported_schema_version() {
    use crate::policy::workspace::deny::Rules;
    assert!(Rules::parse(r#"{"paths":[],"names":[]}"#).is_err());
    assert!(Rules::parse(r#"{"schema":"maestro-secret-paths/2","paths":[],"names":[]}"#).is_err());
}
