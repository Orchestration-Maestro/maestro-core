//! Publication checkpoints and held-source/target substitution proofs.
#[cfg(windows)]
use super::Command;
#[cfg(unix)]
use super::symlink;
use super::{Access, Cell, Fixture, Path, fs};

#[test]
fn publication_rechecks_both_paths_and_rolls_back_only_the_created_link() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("temporary"), b"bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("temporary"), Access::Read)
        .unwrap();
    let publish = fixture
        .trust()
        .authorize(&fixture.root, Path::new("published"), Access::Write)
        .unwrap();
    fixture.approved.set(false);
    let writer_calls = Cell::new(0);
    assert!(
        publish
            .publish_from_with(&source, b"bytes", || writer_calls
                .set(writer_calls.get() + 1))
            .is_err()
    );
    assert_eq!(writer_calls.get(), 0);
    assert!(!fixture.root.join("published").exists());
    fixture.approved.set(true);
    assert!(
        publish
            .publish_from_with(&source, b"bytes", || fixture.approved.set(false))
            .is_err()
    );
    assert!(!fixture.root.join("published").exists());
    assert_eq!(fs::read(fixture.root.join("temporary")).unwrap(), b"bytes");
}

#[test]
fn publication_refuses_different_held_parents_before_linking_any_name() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("other")).unwrap();
    fs::write(
        fixture.root.join("other/temporary"),
        b"different parent bytes",
    )
    .unwrap();
    fs::write(fixture.root.join("temporary"), b"bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("temporary"), Access::Read)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new("other/published"), Access::Write)
        .unwrap();
    let calls = Cell::new(0);
    assert!(
        target
            .publish_from_with(&source, b"bytes", || calls.set(calls.get() + 1))
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(!fixture.root.join("other/published").exists());
    assert_eq!(fs::read(fixture.root.join("temporary")).unwrap(), b"bytes");
}

#[cfg(unix)]
#[test]
fn publication_rechecks_secret_source_before_and_after_link_effect() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("temporary"), b"bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("temporary"), Access::Read)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new("published"), Access::Write)
        .unwrap();
    let calls = Cell::new(0);
    let rebind = || {
        symlink(
            fixture.root.join("temporary"),
            fixture.scratch.path.join(".netrc"),
        )
        .unwrap();
    };
    assert!(
        target
            .publish_before_for_test(&source, b"bytes", rebind, || calls.set(calls.get() + 1))
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(!fixture.root.join("published").exists());
    fs::remove_file(fixture.scratch.path.join(".netrc")).unwrap();
    assert!(target.publish_from_with(&source, b"bytes", rebind).is_err());
    assert!(!fixture.root.join("published").exists());
    assert_eq!(fs::read(fixture.root.join("temporary")).unwrap(), b"bytes");
}

#[test]
fn read_capabilities_cannot_be_promoted_to_new_effects() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("ordinary"), b"bytes").unwrap();
    let read = fixture
        .trust()
        .authorize(&fixture.root, Path::new("ordinary"), Access::Read)
        .unwrap();
    assert!(read.write_new(b"overwrite").is_err());
    assert!(read.create_directory_for_test(|| {}).is_err());
    assert!(read.remove_verified(b"bytes", None).is_err());
    assert!(read.publish_from(&read, b"bytes").is_err());
    let write_source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("ordinary"), Access::Write)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new("published"), Access::Write)
        .unwrap();
    assert!(target.publish_from(&write_source, b"bytes").is_err());
    assert!(!fixture.root.join("published").exists());
    assert_eq!(fs::read(fixture.root.join("ordinary")).unwrap(), b"bytes");
}

#[test]
fn publication_refuses_regular_source_swap_after_held_read_before_link() {
    assert_source_swap(false);
}

#[test]
fn publication_refuses_symlink_source_swap_after_held_read_before_link() {
    assert_source_swap(true);
}

/// Source swapping is exercised independently for a regular replacement and a symlink.
fn assert_source_swap(symlinked: bool) {
    let fixture = Fixture::new();
    let source_name = if symlinked {
        "source-link"
    } else {
        "source-file"
    };
    let target_name = if symlinked {
        "target-link"
    } else {
        "target-file"
    };
    fs::write(fixture.root.join(source_name), b"original bytes").unwrap();
    fs::write(fixture.root.join("neighbour"), b"neighbour bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new(source_name), Access::Read)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new(target_name), Access::Write)
        .unwrap();
    let calls = Cell::new(0);
    let result = target.publish_before_for_test(
        &source,
        b"original bytes",
        || {
            fs::rename(
                fixture.root.join(source_name),
                fixture.root.join(format!("{source_name}-moved")),
            )
            .unwrap();
            if symlinked {
                file_link(
                    &fixture.root.join("neighbour"),
                    &fixture.root.join(source_name),
                );
            } else {
                fs::write(fixture.root.join(source_name), b"replacement bytes").unwrap();
            }
        },
        || calls.set(calls.get() + 1),
    );
    assert!(result.is_err(), "source-object substitution must refuse");
    assert_eq!(
        calls.get(),
        0,
        "source substitution must precede publication"
    );
    assert!(!fixture.root.join(target_name).exists());
    assert_eq!(
        fs::read(fixture.root.join(format!("{source_name}-moved"))).unwrap(),
        b"original bytes"
    );
}

/// Native file symlink proof is mandatory on the physical Windows host, never skipped.
fn file_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .args(["/C", "mklink"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "host must permit file-symlink proof: {output:?}"
        );
    }
}

#[test]
fn publication_post_compare_preserves_a_replacement_of_the_published_link() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("temporary"), b"bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("temporary"), Access::Read)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new("published"), Access::Write)
        .unwrap();
    let result = target.publish_from_with(&source, b"bytes", || {
        fs::rename(
            fixture.root.join("published"),
            fixture.root.join("link-moved"),
        )
        .unwrap();
        fs::write(fixture.root.join("published"), b"replacement").unwrap();
    });
    assert!(result.is_err(), "post-link compare must refuse replacement");
    assert_eq!(
        fs::read(fixture.root.join("published")).unwrap(),
        b"replacement"
    );
    assert_eq!(fs::read(fixture.root.join("temporary")).unwrap(), b"bytes");
}

#[test]
fn publication_after_source_open_rechecks_deny_before_reading_or_linking() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("temporary"), b"bytes").unwrap();
    let source = fixture
        .trust()
        .authorize(&fixture.root, Path::new("temporary"), Access::Read)
        .unwrap();
    let target = fixture
        .trust()
        .authorize(&fixture.root, Path::new("published"), Access::Write)
        .unwrap();
    let before_link = Cell::new(0);
    let result = target.publish_after_open_for_test(
        &source,
        b"bytes",
        || {
            file_link(
                &fixture.root.join("temporary"),
                &fixture.scratch.path.join(".netrc"),
            );
        },
        || before_link.set(before_link.get() + 1),
    );
    assert!(result.is_err());
    assert_eq!(
        before_link.get(),
        0,
        "deny must stop before the read/link checkpoint"
    );
    assert!(!fixture.root.join("published").exists());
    assert_eq!(fs::read(fixture.root.join("temporary")).unwrap(), b"bytes");
}
