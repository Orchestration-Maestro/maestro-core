//! Shared admission, private path IO and loopback normalization seams.

use super::super::{
    graph_output::Code,
    private_run::{PrivateRun, endpoint},
};
use maestro_kernel::{artifact::Digest, paths::Environment};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{cell::Cell, fs, path::PathBuf};

/// Synthetic binding receipt with no process-wide environment changes.
struct Fixture {
    /// Isolated filesystem owner.
    root: PathBuf,
    /// Editable strict receipt.
    value: Value,
    /// Environment supplied to admission.
    environment: Environment,
}

impl Fixture {
    /// Creates all declared roots, as restore does before command admission.
    fn new() -> Self {
        let root = scratch_directory().unwrap();
        for path in [
            "data/maestro",
            "config/maestro",
            "live-data",
            "live-config",
            "storage",
            "live-storage",
            "private",
            "outside",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(root.join("data").into_os_string());
        environment.xdg_config_home = Some(root.join("config").into_os_string());
        let value = json!({
            "scratch_data":root.join("data"), "scratch_config":root.join("config"),
            "live_data":root.join("live-data"), "live_config":root.join("live-config"),
            "scratch_storage":root.join("storage"),"live_storage":root.join("live-storage"),
            "scratch_endpoint":"http://127.0.0.1:16334","live_endpoint":"http://127.0.0.1:6334",
            "private_root":root.join("private"),"output":root.join("private"),
            "backup_id":"backup-1","backup_digest":Digest::of(b"backup").as_str(),
            "approval":{"scope":"synthetic",
            "target":"graph-evaluation",
            "expires_unix":4_102_444_800_u64,
                "evidence_digest":Digest::of(b"approval").as_str()}
        });
        Self {
            root,
            value,
            environment,
        }
    }

    /// Parses through the same strict contract as each graph command.
    fn receipt(&self) -> PrivateRun {
        serde_json::from_value(self.value.clone()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn graph_eval_admission_spy_never_opens_default_or_invalid_bindings() {
    let mut fixture = Fixture::new();
    let calls = Cell::new(0);
    let admitted = fixture
        .receipt()
        .check_bindings(
            "synthetic",
            &fixture.environment,
            Some("http://localhost:16334"),
        )
        .unwrap();
    admitted
        .open_with(|data, config| {
            calls.set(calls.get() + 1);
            assert_eq!(
                data,
                fs::canonicalize(fixture.root.join("data/maestro")).unwrap()
            );
            assert_eq!(
                config,
                fs::canonicalize(fixture.root.join("config/maestro")).unwrap()
            );
            Ok(())
        })
        .unwrap();
    for scope in ["synthetic", "not-approved"] {
        fixture.value["live_data"] = fixture.value["scratch_data"].clone();
        let refused = fixture.receipt().check_bindings(
            scope,
            &fixture.environment,
            Some("http://localhost:16334"),
        );
        if let Ok(run) = refused {
            run.open_with(|_, _| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap();
        }
    }
    assert_eq!(calls.get(), 1);
}

#[test]
fn graph_eval_private_paths_reject_escape_and_overwrite_and_preserve_bytes() {
    let fixture = Fixture::new();
    let run = fixture
        .receipt()
        .check_bindings(
            "synthetic",
            &fixture.environment,
            Some("http://127.0.0.1:16334"),
        )
        .unwrap();
    let path = fixture.root.join("private/result.json");
    let bytes = b"private-question-quote-sentinel";
    assert_eq!(run.write(&path, bytes).unwrap(), Digest::of(bytes));
    assert_eq!(fs::read(run.input(&path).unwrap()).unwrap(), bytes);
    assert_eq!(run.write(&path, b"overwrite"), Err(Code::Output));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        run.write(&fixture.root.join("outside/result.json"), bytes),
        Err(Code::Isolation)
    );
    fs::write(fixture.root.join("outside/source"), bytes).unwrap();
    assert_eq!(
        run.input(&fixture.root.join("private/../outside/source")),
        Err(Code::Isolation)
    );
}

#[test]
fn graph_eval_loopback_guard_normalizes_aliases_and_refuses_egress() {
    for address in [
        "http://localhost:6334/",
        "http://127.0.0.1:6334",
        "http://[::1]:6334",
    ] {
        assert_eq!(endpoint(address), Ok(6334));
    }
    for address in [
        "http://example.com:6334",
        "http://192.0.2.1:6334",
        "http://127.0.0.1:0",
        "https://localhost:6334",
        "http://localhost:6334/path",
    ] {
        assert_eq!(endpoint(address), Err(Code::Isolation));
    }
}

#[test]
fn graph_eval_scope_expiry_and_every_required_binding_are_checked() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .receipt()
            .check_bindings(
                "unapproved",
                &fixture.environment,
                Some("http://127.0.0.1:16334")
            )
            .is_err()
    );
    for (field, replacement) in [
        ("scratch_data", json!("")),
        ("live_config", json!("relative")),
        ("scratch_storage", fixture.value["live_storage"].clone()),
        ("output", fixture.value["live_data"].clone()),
        ("backup_digest", json!("not-a-digest")),
    ] {
        let mut value = fixture.value.clone();
        value[field] = replacement;
        let receipt: PrivateRun = serde_json::from_value(value).unwrap();
        assert!(
            receipt
                .check_bindings(
                    "synthetic",
                    &fixture.environment,
                    Some("http://127.0.0.1:16334")
                )
                .is_err(),
            "{field}"
        );
    }
    let mut value = fixture.value.clone();
    value["approval"]["expires_unix"] = 0.into();
    let receipt: PrivateRun = serde_json::from_value(value).unwrap();
    assert!(
        receipt
            .check_bindings(
                "synthetic",
                &fixture.environment,
                Some("http://127.0.0.1:16334")
            )
            .is_err()
    );
}

// Unix symlinks need no administrator privilege; Windows path admission uses
// the same canonicalization, and its runtime symlink checks belong to native CI.
#[cfg(unix)]
#[test]
fn graph_eval_refuses_a_scratch_database_symlink_to_live_authority() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let live = fixture.root.join("live-data/kernel.sqlite3");
    fs::write(&live, b"live-authority-sentinel").unwrap();
    symlink(&live, fixture.root.join("data/maestro/kernel.sqlite3")).unwrap();
    assert!(
        fixture
            .receipt()
            .check_bindings(
                "synthetic",
                &fixture.environment,
                Some("http://127.0.0.1:16334")
            )
            .is_err()
    );
    assert_eq!(fs::read(live).unwrap(), b"live-authority-sentinel");
}

#[test]
fn graph_eval_hard_link_is_refused_before_open() {
    let fixture = Fixture::new();
    let live = fixture.root.join("live-data/kernel.sqlite3");
    fs::write(&live, b"live-authority").unwrap();
    fs::hard_link(&live, fixture.root.join("data/maestro/kernel.sqlite3")).unwrap();
    let calls = Cell::new(0);
    let admitted = fixture.receipt().check_bindings(
        "synthetic",
        &fixture.environment,
        Some("http://localhost:16334"),
    );
    if let Ok(run) = &admitted {
        run.open_with(|_, _| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(calls.get(), 0);
    assert!(matches!(admitted, Err(Code::Isolation)));
    assert_eq!(fs::read(live).unwrap(), b"live-authority");
}

#[test]
fn graph_eval_process_endpoint_must_match_address_not_only_port() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.receipt().check_bindings(
            "synthetic",
            &fixture.environment,
            Some("http://127.0.0.2:16334")
        ),
        Err(Code::Isolation)
    ));
}

/// Isolates one environment defect from otherwise admitted bindings.
fn refused_environment(field: &str, missing: bool) {
    let mut fixture = Fixture::new();
    let replacement = if missing {
        None
    } else {
        Some(fixture.root.join("outside").into_os_string())
    };
    if field == "data" {
        fixture.environment.xdg_data_home = replacement;
    } else {
        fixture.environment.xdg_config_home = replacement;
    }
    assert!(
        matches!(
            fixture.receipt().check_bindings(
                "synthetic",
                &fixture.environment,
                Some("http://localhost:16334")
            ),
            Err(Code::Isolation)
        ),
        "{field} missing={missing}"
    );
}

#[test]
fn graph_eval_missing_xdg_binding_is_refused() {
    for field in ["data", "config"] {
        refused_environment(field, true);
    }
}

#[test]
fn graph_eval_mismatched_xdg_binding_is_refused() {
    for field in ["data", "config"] {
        refused_environment(field, false);
    }
}

#[test]
fn graph_eval_expiry_equality_is_refused_with_injected_time() {
    let mut fixture = Fixture::new();
    fixture.value["approval"]["expires_unix"] = 100.into();
    let receipt = fixture.receipt();
    assert_eq!(receipt.check_approval_at("synthetic", 99), Ok(()));
    assert_eq!(
        receipt.check_approval_at("synthetic", 100),
        Err(Code::Isolation)
    );
    assert_eq!(
        receipt.check_approval_at("synthetic", 101),
        Err(Code::Isolation)
    );
}

#[test]
fn graph_eval_approval_fields_are_independent_refusals() {
    let fixture = Fixture::new();
    for (field, value) in [
        ("target", json!("other")),
        ("backup_id", json!("")),
        ("backup_id", json!("unsafe id")),
    ] {
        let mut value_receipt = fixture.value.clone();
        if field == "target" {
            value_receipt["approval"][field] = value;
        } else {
            value_receipt[field] = value;
        }
        let receipt: PrivateRun = serde_json::from_value(value_receipt).unwrap();
        assert_eq!(
            receipt.check_approval_at("synthetic", 1),
            Err(Code::Isolation),
            "{field}"
        );
    }
    let mut value = fixture.value.clone();
    value["approval"]["scope"] = json!("");
    let receipt: PrivateRun = serde_json::from_value(value).unwrap();
    assert_eq!(receipt.check_approval_at("", 1), Err(Code::Isolation));
}

#[test]
fn graph_eval_private_root_cannot_be_an_ancestor_of_authority() {
    let mut fixture = Fixture::new();
    fixture.value["private_root"] = json!(fixture.root);
    assert!(matches!(
        fixture.receipt().check_bindings(
            "synthetic",
            &fixture.environment,
            Some("http://localhost:16334")
        ),
        Err(Code::Isolation)
    ));
}

#[cfg(unix)]
#[test]
fn graph_eval_each_resolved_kernel_child_must_stay_below_its_root() {
    use std::os::unix::fs::symlink;
    for root in ["data", "config"] {
        let fixture = Fixture::new();
        fs::remove_dir(fixture.root.join(root).join("maestro")).unwrap();
        symlink(
            fixture.root.join("outside"),
            fixture.root.join(root).join("maestro"),
        )
        .unwrap();
        assert!(
            matches!(
                fixture.receipt().check_bindings(
                    "synthetic",
                    &fixture.environment,
                    Some("http://localhost:16334")
                ),
                Err(Code::Isolation)
            ),
            "{root}"
        );
    }
}

#[cfg(unix)]
#[test]
fn graph_eval_child_metadata_permission_errors_are_not_missing_files() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    let data = fixture.root.join("data/maestro");
    fs::set_permissions(&data, fs::Permissions::from_mode(0o0)).unwrap();
    let result = fixture.receipt().check_bindings(
        "synthetic",
        &fixture.environment,
        Some("http://localhost:16334"),
    );
    fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(result, Err(Code::Isolation)));
}

#[test]
fn graph_draft_journal_append_accepts_exact_serialized_limit_and_refuses_one_over() {
    use super::super::draft_journal::FileJournal;
    use maestro_knowledge::eval::{
        draft::{DraftCandidate, DraftError},
        draft_progress::{DraftJournal, DraftOutcome, DraftReceipt},
    };
    let fixture = Fixture::new();
    let run = fixture
        .receipt()
        .check_bindings(
            "synthetic",
            &fixture.environment,
            Some("http://localhost:16334"),
        )
        .unwrap();
    let mut journal = FileJournal::open(&run, 1).unwrap();
    let mut receipt = DraftReceipt {
        run: Digest::of(b"run"),
        input: Digest::of(b"input"),
        id: "q-1".into(),
        tokens: 1,
        outcome: DraftOutcome::Draft(DraftCandidate {
            id: "q-1".into(),
            family: "f-1".into(),
            suite: String::new(),
            labels: String::new(),
        }),
    };
    let overhead = serde_json::to_vec(&json!({"schema":"maestro-graph-draft-receipt/1",
        "receipt":receipt}))
    .unwrap()
    .len();
    let DraftOutcome::Draft(candidate) = &mut receipt.outcome else {
        panic!("draft")
    };
    candidate.suite = "x".repeat(16 * 1024 * 1024 - overhead);
    assert_eq!(journal.append(receipt.clone()), Ok(()));
    assert_eq!(
        fs::metadata(run.output.join("draft-receipt-00000000000000000000.json"))
            .unwrap()
            .len(),
        16 * 1024 * 1024
    );
    let DraftOutcome::Draft(candidate) = &mut receipt.outcome else {
        panic!("draft")
    };
    candidate.suite.push('x');
    assert_eq!(journal.append(receipt), Err(DraftError::Journal));
    assert_eq!(journal.receipts().len(), 1);
}
