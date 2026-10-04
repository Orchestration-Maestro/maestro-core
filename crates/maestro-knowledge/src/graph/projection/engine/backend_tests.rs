//! Private native adapter entry points into the unchanged fake/native contract.

use super::backend::{Backend, Publication};
use super::tests::{Fixture, config, scope};
#[cfg(not(windows))]
use crate::graph::projection::EdgeFamily;
#[cfg(windows)]
use crate::graph::projection::tests::contract_reads;
use crate::graph::projection::{
    content,
    tests::contract,
    writer::{ProjectionBackend, ProjectionBackendReader, receipt_from_verification},
};
use lbug::RootDirectory;
use maestro_kernel::{
    artifact::Digest,
    scope::{Right, Scope},
    store::Database,
};
#[cfg(not(windows))]
use std::slice;
use std::{fs, path::PathBuf};

/// Test-only no-overwrite installation; E08b owns the real anchored lifecycle.
struct Install(PathBuf);
impl Publication for Install {
    fn install(&mut self, staging: &str, published: &str) -> Result<(), String> {
        fs::hard_link(self.0.join(staging), self.0.join(published))
            .map_err(|error| error.to_string())?;
        fs::remove_file(self.0.join(staging)).map_err(|error| error.to_string())
    }
}

fn backend(fixture: &Fixture) -> Backend<Install> {
    Backend::new(
        RootDirectory::open(&fixture.path).unwrap(),
        RootDirectory::open(&fixture.path).unwrap(),
        "staging.lbdb".into(),
        (config(), contract::pins()),
        Install(fixture.path.clone()),
    )
}

fn kernel(fixture: &Fixture) -> Database {
    let kernel_path = fixture.path.join("kernel");
    fs::create_dir(&kernel_path).unwrap();
    let kernel = Database::open_in(&kernel_path).unwrap();
    kernel
        .grant(
            "reader",
            &"workspace/default".parse::<Scope>().unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
    kernel
}

#[cfg(not(windows))]
fn shared_contract(rollback: bool) {
    let fixture = Fixture::new();
    let kernel = kernel(&fixture);
    let scopes = kernel.visible("reader").unwrap();
    let denied = kernel.visible("denied").unwrap();
    let scope = scope();
    let (edges, facts) = contract::ordered_rows(&scope);
    let contract = contract::fixture(&scopes, &denied, &scope, &edges, &facts);
    let mut backend = backend(&fixture);
    if rollback {
        contract::run(&mut backend, &contract);
    } else {
        contract::run_verified(&mut backend, &contract);
    }
    let file_name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    assert!(fixture.path.join(file_name).is_file());
    assert!(!fixture.path.join("staging.lbdb").exists());
}

#[cfg(not(windows))]
#[test]
fn native_shared_full_backend_contract() {
    shared_contract(true);
}

#[cfg(not(windows))]
#[test]
fn native_shared_verification_publication_and_pinned_reads() {
    shared_contract(false);
}

#[cfg(not(windows))]
#[test]
fn native_receipt_opens_only_matching_physical_file_and_content() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    backend.create_unpublished(&scope).unwrap();
    let edge = contract::edge(1, EdgeFamily::KnowledgeClaim);
    backend
        .write_batch(&scope, slice::from_ref(&edge), &[])
        .unwrap();
    let build = backend.verify_unpublished(&scope).unwrap();
    let set = Digest::of(b"set");
    let name = content::basename(&scope, &set).unwrap();
    backend.publish_unpublished(&scope, &name).unwrap();
    let receipt =
        receipt_from_verification(&scope, set, name.clone(), &build, &contract::pins()).unwrap();
    let reader = backend.open_published(&scope, &receipt).unwrap();
    assert_eq!(
        reader.edges_adjacent(edge.family, &edge.target).unwrap(),
        [edge]
    );
    let mut wrong = receipt.clone();
    wrong.identity.content_digest = Digest::of(b"wrong");
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.entity_fact_count += 1;
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.claim_set_id = Digest::of(b"other set");
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.collection_id = "other".into();
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.generation_id += 1;
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.schema_version = "unknown".into();
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.knowledge_edge_count += 1;
    assert!(backend.open_published(&scope, &wrong).is_err());
    wrong = receipt.clone();
    wrong.identity.catalog_dependency_edge_count += 1;
    assert!(backend.open_published(&scope, &wrong).is_err());
    drop(reader);
    let before = fs::read(fixture.path.join(&name)).unwrap();
    fs::rename(fixture.path.join(&name), fixture.path.join("other.lbdb")).unwrap();
    assert!(backend.open_published(&scope, &receipt).is_err());
    assert_eq!(fs::read(fixture.path.join("other.lbdb")).unwrap(), before);
}

#[cfg(windows)]
#[test]
fn windows_backend_refuses_create_before_native_io() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    assert!(
        backend
            .create_unpublished(&scope())
            .unwrap_err()
            .contains("unavailable on Windows")
    );
    assert_eq!(fs::read_dir(&fixture.path).unwrap().count(), 0);
}

#[cfg(not(windows))]
#[test]
fn poison_survives_verification_reopen_and_refuses_verify_publish_and_write() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    backend.create_unpublished(&scope).unwrap();
    backend.verify_unpublished(&scope).unwrap();
    backend.poison_for_test().unwrap();
    let before = fs::read(fixture.path.join("staging.lbdb")).unwrap();
    let name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    for error in [
        backend.verify_unpublished(&scope).unwrap_err(),
        backend.publish_unpublished(&scope, &name).unwrap_err(),
        backend.write_batch(&scope, &[], &[]).unwrap_err(),
    ] {
        assert!(error.contains("poisoned"), "{error}");
    }
    assert!(!fixture.path.join(name).exists());
    assert_eq!(fs::read(fixture.path.join("staging.lbdb")).unwrap(), before);
}

#[cfg(not(windows))]
#[test]
fn backend_refuses_wrong_scope_unverified_or_stale_publication_and_recreation() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    let mut other = scope.clone();
    other.generation_id += 1;
    let name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    assert!(backend.verify_unpublished(&scope).is_err());
    assert!(backend.write_batch(&scope, &[], &[]).is_err());
    assert!(backend.publish_unpublished(&scope, &name).is_err());
    backend.create_unpublished(&scope).unwrap();
    assert!(backend.create_unpublished(&scope).is_err());
    assert!(backend.publish_unpublished(&scope, &name).is_err());
    backend.verify_unpublished(&scope).unwrap();
    assert!(backend.verify_unpublished(&other).is_err());
    assert!(backend.write_batch(&other, &[], &[]).is_err());
    assert!(backend.publish_unpublished(&other, &name).is_err());
    assert!(
        backend
            .publish_unpublished(&scope, "../outside.lbdb")
            .is_err()
    );
    backend.write_batch(&scope, &[], &[]).unwrap();
    assert!(backend.publish_unpublished(&scope, &name).is_err());
    assert!(!fixture.path.join(name).exists());
}

#[cfg(not(windows))]
#[test]
fn publication_failure_never_overwrites_or_reopens_a_writer() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    let name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    fs::write(fixture.path.join(&name), b"existing file").unwrap();
    backend.create_unpublished(&scope).unwrap();
    backend.verify_unpublished(&scope).unwrap();
    assert!(backend.publish_unpublished(&scope, &name).is_err());
    assert_eq!(
        fs::read(fixture.path.join(&name)).unwrap(),
        b"existing file"
    );
    assert!(fixture.path.join("staging.lbdb").is_file());
    assert!(backend.write_batch(&scope, &[], &[]).is_err());
    assert!(backend.verify_unpublished(&scope).is_err());
    assert!(backend.publish_unpublished(&scope, &name).is_err());
}

#[cfg(windows)]
#[test]
fn windows_native_immutable_fixture_runs_shared_ordered_pinned_reader_contract() {
    use super::{
        schema::tests::install_reader_fixture, transaction::tests::populate_reader_fixture,
    };
    use lbug::Connection;

    let fixture = Fixture::new();
    let kernel = kernel(&fixture);
    let scopes = kernel.visible("reader").unwrap();
    let denied = kernel.visible("denied").unwrap();
    let scope = scope();
    let (edges, facts) = contract::ordered_rows(&scope);
    let contract = contract::fixture(&scopes, &denied, &scope, &edges, &facts);
    {
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        install_reader_fixture(&connection, &scope, &contract::pins());
        populate_reader_fixture(&connection, &scope, &edges, &facts);
        connection.query("CHECKPOINT").unwrap();
    }
    let set = Digest::of(b"set");
    let name = content::basename(&scope, &set).unwrap();
    Install(fixture.path.clone())
        .install("rows.lbdb", &name)
        .unwrap();
    super::open::tests::private_windows_fixture(&fixture.path);
    let backend = backend(&fixture);
    let receipt = receipt_from_verification(
        &scope,
        set,
        name.clone(),
        &contract.expected,
        &contract::pins(),
    )
    .unwrap();
    let before = fs::read(fixture.path.join(&name)).unwrap();
    contract_reads::assert_reads(&backend, &contract, &receipt);
    assert_eq!(
        backend
            .open_published(&scope, &receipt)
            .unwrap()
            .verification()
            .unwrap(),
        contract.expected
    );
    assert_eq!(fs::read(fixture.path.join(name)).unwrap(), before);
}

#[cfg(not(windows))]
#[test]
fn native_shared_readers_keep_old_generation_after_later_publication() {
    use crate::graph::projection::tests::contract_reads;

    let fixture = Fixture::new();
    let kernel = kernel(&fixture);
    let scopes = kernel.visible("reader").unwrap();
    let mut first = backend(&fixture);
    let mut second = Backend::new(
        RootDirectory::open(&fixture.path).unwrap(),
        RootDirectory::open(&fixture.path).unwrap(),
        "next-staging.lbdb".into(),
        (config(), contract::pins()),
        Install(fixture.path.clone()),
    );
    contract_reads::pinned_generations(&mut first, &mut second, &scopes);
}

#[cfg(not(windows))]
#[test]
fn native_publication_refuses_other_collection_at_same_generation() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    backend.create_unpublished(&scope).unwrap();
    let baseline = backend.verify_unpublished(&scope).unwrap();
    let mut other = scope.clone();
    other.collection_id = "other".into();
    let name = content::basename(&other, &Digest::of(b"set")).unwrap();
    assert_eq!(
        backend.publish_unpublished(&other, &name).unwrap_err(),
        "native projection has no writable session for this scope"
    );
    assert!(!fixture.path.join(&name).exists());
    assert!(fixture.path.join("staging.lbdb").is_file());
    assert_eq!(backend.verify_unpublished(&scope).unwrap(), baseline);
    let name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    backend.publish_unpublished(&scope, &name).unwrap();
    assert!(fixture.path.join(name).is_file());
    assert!(!fixture.path.join("staging.lbdb").exists());
}

#[cfg(not(windows))]
#[test]
fn native_usable_refuses_poisoned_matching_and_unpoisoned_mismatched_scopes() {
    let fixture = Fixture::new();
    let mut backend = backend(&fixture);
    let scope = scope();
    backend.create_unpublished(&scope).unwrap();
    backend.verify_unpublished(&scope).unwrap();
    let mut other = scope.clone();
    other.generation_id += 1;
    let name = content::basename(&other, &Digest::of(b"set")).unwrap();
    assert_eq!(
        backend.publish_unpublished(&other, &name).unwrap_err(),
        "native projection has no writable session for this scope"
    );
    assert!(!fixture.path.join(name).exists());
    backend.verify_unpublished(&scope).unwrap();
    backend.poison_for_test().unwrap();
    let name = content::basename(&scope, &Digest::of(b"set")).unwrap();
    assert_eq!(
        backend.publish_unpublished(&scope, &name).unwrap_err(),
        "native projection session is poisoned after rollback failure"
    );
    assert!(!fixture.path.join(name).exists());
    assert!(fixture.path.join("staging.lbdb").is_file());
}

/// Verification never invokes filesystem installation in this proof.
#[cfg(unix)]
struct NoInstall;
#[cfg(unix)]
impl Publication for NoInstall {
    fn install(&mut self, _: &str, _: &str) -> Result<(), String> {
        Err("verification must not publish".into())
    }
}

#[cfg(unix)]
#[test]
fn reopened_build_refuses_changed_expected_pins_and_matching_pins_verify() {
    let fixture = Fixture::new();
    let pins = contract::pins();
    let mut backend = Backend::new(
        RootDirectory::open(&fixture.path).unwrap(),
        RootDirectory::open(&fixture.path).unwrap(),
        "staging.lbdb".into(),
        (config(), pins.clone()),
        NoInstall,
    );
    backend.create_unpublished(&scope()).unwrap();
    let mut changed = pins.clone();
    changed[0] = Digest::of(b"changed snapshot").as_str().into();
    backend.replace_expected_pins_for_test(changed);
    assert!(
        backend
            .verify_unpublished(&scope())
            .expect_err("changed input cannot replay durable native stamp")
            .contains("maestro knowledge graph rebuild")
    );
    backend.replace_expected_pins_for_test(pins);
    backend
        .verify_unpublished(&scope())
        .expect("matching inputs verify after independent reopen");
}
