//! Lock-only C04 ownership admission and non-authority output drift.
use super::{
    inventory::Fixture,
    session_lock::{admit, initialized},
};
use crate::limits::Limits;
use std::{fs, path::PathBuf};

/// The fixture's init plan commits one record; preference drafts are not applied.
fn ownership_path(fixture: &Fixture) -> PathBuf {
    fs::read_dir(fixture.project.join(".maestro-files"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ownership-")
        })
        .unwrap()
}

#[test]
fn session_output_drift_is_a_note_without_deletion() {
    for drift in ["changed", "missing", "unowned"] {
        let fixture = initialized();
        let relative = ".github/copilot-instructions.md";
        let output = fixture.project.join(relative);
        match drift {
            "changed" => fs::write(&output, b"user bytes").unwrap(),
            "missing" => fs::remove_file(&output).unwrap(),
            _ => {
                let path = ownership_path(&fixture);
                let mut record: toml::Value =
                    toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                record["files"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|file| file["path"].as_str() != Some(relative));
                fs::write(path, toml::to_string(&record).unwrap()).unwrap();
            }
        }
        let admitted = admit(&fixture, &Limits::PRODUCTION).unwrap();
        let note = admitted.discovery.note.unwrap();
        assert!(note.contains(relative), "{note}");
        assert!(note.contains("maestro init"), "{note}");
        if drift == "changed" {
            assert_eq!(fs::read(&output).unwrap(), b"user bytes");
        }
    }
}

#[test]
fn session_missing_or_malformed_ownership_refuses_with_recovery() {
    for malformed in [false, true] {
        let fixture = initialized();
        let path = ownership_path(&fixture);
        if malformed {
            fs::write(path, b"malformed record").unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
        let error = admit(&fixture, &Limits::PRODUCTION).unwrap_err();
        assert!(error.contains(".maestro/authoring.lock.json"), "{error}");
        assert!(
            error.contains("restore the file from version control"),
            "{error}"
        );
        assert!(
            error.contains("remove .maestro and the generated outputs"),
            "{error}"
        );
        assert!(error.contains("maestro init"), "{error}");
    }
}

#[test]
fn session_ambiguous_ownership_refuses() {
    let fixture = initialized();
    let path = ownership_path(&fixture);
    let mut record: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let id = format!("sha256:{}", "a".repeat(64));
    record["id"] = toml::Value::String(id.clone());
    let second = path
        .parent()
        .unwrap()
        .join(format!("ownership-{}.toml", id.replace(':', "-")));
    fs::write(&second, toml::to_string(&record).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(second, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let error = admit(&fixture, &Limits::PRODUCTION).unwrap_err();
    assert!(
        error.contains("more than one committed ownership candidate"),
        "{error}"
    );
    assert!(error.contains(".maestro/authoring.lock.json"), "{error}");
    assert!(error.contains("then run maestro init"), "{error}");
}

#[test]
fn session_ownership_without_lock_entry_refuses() {
    let fixture = initialized();
    let path = ownership_path(&fixture);
    let mut record: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    record["files"]
        .as_array_mut()
        .unwrap()
        .retain(|file| file["path"].as_str() != Some(".maestro/authoring.lock.json"));
    fs::write(path, toml::to_string(&record).unwrap()).unwrap();
    let error = admit(&fixture, &Limits::PRODUCTION).unwrap_err();
    assert!(
        error.contains("no committed ownership record with lock entry"),
        "{error}"
    );
    assert!(error.contains("then run maestro init"), "{error}");
}
