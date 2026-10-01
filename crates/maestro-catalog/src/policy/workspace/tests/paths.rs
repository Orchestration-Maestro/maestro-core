//! Real held-parent path decisions and immutable deny data.
use crate::policy::workspace::{Access, CheckedTrust, TrustBoundaries, WorkspaceTrust};
use maestro_filesystem::Directory;
use maestro_kernel::paths::Environment;
use maestro_test_scratch::scratch_directory;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::process::Command;
use std::{
    fs,
    io::Write as _,
    path::{Component, MAIN_SEPARATOR_STR, Path, PathBuf},
    slice::from_ref,
};

/// Preserve user spelling: verbatim `PathBuf::push` normalizes dot components.
pub(super) fn literal_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut spelling = path.as_os_str().to_owned();
    spelling.push(MAIN_SEPARATOR_STR);
    spelling.push(suffix);
    PathBuf::from(spelling)
}

struct Roots(Vec<PathBuf>);
impl WorkspaceTrust for Roots {
    fn containing_root(&self, start: &Path) -> Option<PathBuf> {
        self.0
            .iter()
            .filter(|root| start.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
    }
}

struct Fixture {
    home: PathBuf,
    project: PathBuf,
    outside: PathBuf,
    boundaries: TrustBoundaries,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        let home = scratch_directory().unwrap().canonicalize().unwrap();
        let project = home.join("project");
        let outside = home.join("project-other");
        for path in [&project, &outside, &home.join("kernel")] {
            fs::create_dir(path).unwrap();
        }
        let boundaries = TrustBoundaries::new(&home, &[home.join("kernel")]).unwrap();
        Self {
            roots: Roots(vec![project.clone()]),
            home,
            project,
            outside,
            boundaries,
        }
    }
    fn trust(&self) -> CheckedTrust<'_> {
        CheckedTrust::new(&self.roots, &self.boundaries)
    }
    fn file(&self, relative: &str) {
        let path = self.home.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"unchanged").unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.home).unwrap();
    }
}

#[test]
fn inside_reads_writes_and_outside_reads_but_not_writes() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    fixture.file("project-other/plain");
    for root in [&fixture.project, &fixture.outside] {
        let read = fixture
            .trust()
            .authorize(root, Path::new("plain"), Access::Read)
            .unwrap();
        assert_eq!(read.open_read().unwrap().metadata().unwrap().len(), 9);
        assert!(read.create_new().is_err());
    }
    assert!(
        fixture
            .trust()
            .authorize(&fixture.outside, Path::new("plain"), Access::Write)
            .is_err()
    );
    assert!(
        fixture
            .trust()
            .authorize(&fixture.outside, Path::new("new"), Access::Write)
            .is_err()
    );
    let write = fixture
        .trust()
        .authorize(&fixture.project, Path::new("new"), Access::Write)
        .unwrap();
    assert!(!fixture.project.join("new").exists());
    assert!(write.open_read().is_err());
    write.create_new().unwrap().write_all(b"allowed").unwrap();
    assert!(write.open_read().is_err());
    fixture.file("project/read-only");
    let read = fixture
        .trust()
        .authorize(&fixture.project, Path::new("read-only"), Access::Read)
        .unwrap();
    fs::remove_file(fixture.project.join("read-only")).unwrap();
    assert!(read.create_new().is_err());
    assert!(!fixture.project.join("read-only").exists());
    assert_eq!(fs::read(fixture.project.join("new")).unwrap(), b"allowed");
    assert_eq!(
        fs::read(fixture.outside.join("plain")).unwrap(),
        b"unchanged"
    );
    assert!(!fixture.outside.join("new").exists());
}

#[test]
fn nested_roots_and_new_file_parents_are_held_without_effects() {
    let mut fixture = Fixture::new();
    fs::create_dir(fixture.project.join("nested")).unwrap();
    fixture.roots.0.push(fixture.project.join("nested"));
    let allowed = fixture
        .trust()
        .authorize(&fixture.project, Path::new("nested/new"), Access::Write)
        .unwrap();
    allowed.revalidate().unwrap();
    assert!(!fixture.project.join("nested/new").exists());
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("missing/new"), Access::Write)
            .is_err()
    );
    assert!(!fixture.project.join("missing").exists());
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("nested/missing"), Access::Read)
            .is_err()
    );
}

#[test]
fn traversal_absolute_and_non_regular_targets_refuse_beside_plain_leaf() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    let traversal = literal_suffix(Path::new(".."), "project-other/plain");
    assert!(
        traversal
            .components()
            .any(|part| part == Component::ParentDir)
    );
    for relative in [
        traversal,
        fixture.outside.join("plain"),
        PathBuf::from(""),
        PathBuf::from("nested/../plain"),
        PathBuf::from("./plain"),
        PathBuf::from("plain:stream"),
        PathBuf::from("plain\\child"),
    ] {
        for access in [Access::Read, Access::Write] {
            assert!(
                fixture
                    .trust()
                    .authorize(&fixture.project, &relative, access)
                    .is_err(),
                "{relative:?}"
            );
        }
    }
    fs::create_dir(fixture.project.join("directory")).unwrap();
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("directory"), Access::Read)
            .is_err()
    );
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("directory"), Access::Write)
            .is_err()
    );
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("plain"), Access::Write)
            .is_ok()
    );
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("plain"), Access::Read)
            .is_ok()
    );
}

fn directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    symlink(target, link).unwrap();
    #[cfg(windows)]
    assert!(
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn symlink_or_reparse_escapes_and_linked_base_use_canonical_authority() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    fixture.file("project-other/plain");
    directory_link(&fixture.outside, &fixture.project.join("escape"));
    let held = Directory::open_canonical(&fixture.project).unwrap();
    assert!(held.child("escape").is_err());
    drop(held);
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("escape/plain"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("escape/new"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("plain"), access)
                .is_ok()
        );
    }
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project.join("escape"),
                Path::new("new"),
                Access::Write
            )
            .is_err()
    );
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project.join("escape"),
                Path::new("plain"),
                Access::Read
            )
            .is_ok()
    );
    assert!(!fixture.outside.join("new").exists());
}

#[test]
fn leaf_links_refuse_beside_regular_file() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    #[cfg(unix)]
    symlink(fixture.project.join("plain"), fixture.project.join("link")).unwrap();
    #[cfg(windows)]
    directory_link(&fixture.outside, &fixture.project.join("link"));
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("link"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("plain"), access)
                .is_ok()
        );
    }
}

#[test]
fn link_swap_after_authorization_cannot_redirect_a_file_effect() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("parent")).unwrap();
    let allowed = fixture
        .trust()
        .authorize(&fixture.project, Path::new("parent/new"), Access::Write)
        .unwrap();
    let moved = fixture.project.join("moved");
    #[cfg(unix)]
    {
        fs::rename(fixture.project.join("parent"), &moved).unwrap();
        directory_link(&fixture.outside, &fixture.project.join("parent"));
        assert!(allowed.revalidate().is_err());
        assert!(allowed.create_new().is_err());
        assert!(!moved.join("new").exists());
    }
    #[cfg(windows)]
    {
        assert!(fs::rename(fixture.project.join("parent"), &moved).is_err());
        allowed.revalidate().unwrap();
        allowed.create_new().unwrap();
        assert!(fixture.project.join("parent/new").exists());
    }
    assert!(!fixture.outside.join("new").exists());
    drop(allowed);
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
}

#[test]
fn held_parent_identity_mismatch_refuses_even_without_a_link() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("parent")).unwrap();
    let held = Directory::open_canonical(&fixture.project.join("parent")).unwrap();
    held.verify_named().unwrap();
    #[cfg(unix)]
    {
        fs::rename(
            fixture.project.join("parent"),
            fixture.project.join("moved"),
        )
        .unwrap();
        fs::create_dir(fixture.project.join("parent")).unwrap();
        assert!(held.verify_named().is_err());
    }
    #[cfg(windows)]
    assert!(
        fs::rename(
            fixture.project.join("parent"),
            fixture.project.join("moved")
        )
        .is_err()
    );
    drop(held);
    Directory::open_canonical(&fixture.project)
        .unwrap()
        .verify_named()
        .unwrap();
}

#[test]
fn kernel_storage_refuses_under_broader_trust_for_every_adapter() {
    let mut fixture = Fixture::new();
    fixture.file("project/kernel/private");
    fixture.file("project/plain");
    fixture.boundaries =
        TrustBoundaries::new(&fixture.home, from_ref(&fixture.project.join("kernel"))).unwrap();
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("kernel/private"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("plain"), access)
                .is_ok()
        );
    }
}

#[test]
fn public_regular_open_is_one_leaf_not_a_path() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    fixture.file("project-other/plain");
    let held = Directory::open_canonical(&fixture.project).unwrap();
    for name in [
        "../project-other/plain",
        "nested/plain",
        ".",
        "..",
        "",
        "plain:stream",
        "plain\\child",
    ] {
        assert!(held.open_regular(name).is_err(), "{name}");
    }
    assert!(held.open_regular("plain").is_ok());
}

#[path = "path_refusals.rs"]
mod refusals;

#[path = "path_secrets.rs"]
mod secrets;

#[path = "path_effects.rs"]
mod effects;
