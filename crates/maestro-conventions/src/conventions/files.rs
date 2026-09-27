//! Lists repository files while excluding generated output.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Entries that hold history, build output or mutation test output, never repository content.
/// `.git` is a file in a worktree or submodule.
const SKIPPED: [&str; 4] = [".git", "target", "mutants.out", "mutants.out.old"];

/// The repository root, two levels above this crate.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file under `root` outside `.git`, `target` and mutation output, relative and sorted.
///
/// A walk rather than `git ls-files`: mutation testing runs in a copy without `.git`.
///
/// # Errors
/// Any unreadable directory: a policy that cannot see a file must fail.
pub fn repository_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if SKIPPED.iter().any(|name| entry.file_name() == *name) {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                files.push(
                    path.strip_prefix(root)
                        .map_err(io::Error::other)?
                        .to_path_buf(),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conventions::test_support::scratch;

    #[test]
    fn repository_files_skip_git_and_build_output() -> io::Result<()> {
        let root = scratch("files")?;
        for dir in [
            ".git",
            "target/debug",
            "crates/a/target",
            "crates/a/src",
            "crates/b",
            "mutants.out/log",
            "mutants.out.old",
        ] {
            fs::create_dir_all(root.join(dir))?;
        }
        for file in [
            ".git/HEAD",
            // A worktree's `.git` is a file naming the main checkout.
            "crates/b/.git",
            "target/debug/y",
            "crates/a/target/z",
            "crates/a/src/lib.rs",
            "mutants.out/log/m.log",
            "mutants.out.old/debug.log",
            "README.md",
        ] {
            fs::write(root.join(file), "")?;
        }
        let files = repository_files(&root)?;
        fs::remove_dir_all(&root)?;
        assert_eq!(
            files,
            [
                PathBuf::from("README.md"),
                PathBuf::from("crates/a/src/lib.rs")
            ]
        );
        Ok(())
    }
}
