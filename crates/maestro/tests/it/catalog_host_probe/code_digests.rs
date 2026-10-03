//! The digest of the code a pinned host runs, not of its launcher: a native
//! host is its executable, a script host is the whole npm package holding the
//! script, and Copilot's npm loader launches a platform binary of its own.

use super::host_sandbox::Sandbox;
use maestro_kernel::artifact::Digest;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// What a pin's digest covers.
pub(super) enum Code {
    /// The resolved executable, a native binary.
    Executable,
    /// The npm package holding the resolved script, as [`tree_digest`].
    PackageTree,
    /// One file of the npm package holding the resolved script, at the
    /// relative path the function gives: the binary its loader launches.
    PackageFile(fn() -> PathBuf),
}

/// The file or package `code` covers for `executable`, and its digest.
pub(super) fn covered(code: &Code, executable: &Path) -> (PathBuf, Digest) {
    let file = |path: PathBuf| {
        let digest = Digest::of(&fs::read(&path).unwrap());
        (path, digest)
    };
    match code {
        Code::Executable => file(executable.to_owned()),
        Code::PackageTree => {
            let root = package_root(executable);
            let digest = tree_digest(&root);
            (root, digest)
        }
        Code::PackageFile(relative) => file(package_root(executable).join(relative())),
    }
}

/// The nearest directory above `script` holding a `package.json`.
fn package_root(script: &Path) -> PathBuf {
    script
        .ancestors()
        .skip(1)
        .find(|directory| directory.join("package.json").is_file())
        .unwrap_or_else(|| panic!("no npm package holds {}", script.display()))
        .to_owned()
}

/// The SHA-256 of a package tree, as this shell recipe run in `root` prints:
///
/// ```sh
/// find . -type f -not -path './node_modules/*' | LC_ALL=C sort \
///   | xargs sha256sum | sha256sum
/// ```
///
/// Regular files only (no symbolic link is followed or counted), the
/// top-level `node_modules` left out, paths sorted bytewise.
pub(super) fn tree_digest(root: &Path) -> Digest {
    let mut files = Vec::new();
    collect_files(root, ".", &mut files);
    files.retain(|(name, _)| !name.starts_with("./node_modules/"));
    files.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    let listing = files
        .iter()
        .fold(String::new(), |mut listing, (name, path)| {
            let digest = Digest::of(&fs::read(path).unwrap());
            listing.push_str(digest.as_str());
            listing.push_str("  ");
            listing.push_str(name);
            listing.push('\n');
            listing
        });
    Digest::of(listing.as_bytes())
}

/// Adds every regular file under `directory`, named `prefix/...` with `/`
/// separators, without following symbolic links.
fn collect_files(directory: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let name = format!("{prefix}/{}", entry.file_name().to_str().unwrap());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect_files(&entry.path(), &name, files);
        } else if kind.is_file() {
            files.push((name, entry.path()));
        }
    }
}

/// Copilot's platform binary under its npm package, as its loader names it.
pub(super) fn copilot_platform_binary() -> PathBuf {
    let system = match env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let architecture = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    PathBuf::from(format!(
        "node_modules/@github/copilot-{system}-{architecture}/copilot{}",
        env::consts::EXE_SUFFIX
    ))
}

/// A package tree: two files, a file under a nested `node_modules` that
/// counts, and a top-level `node_modules` that does not.
fn package(sandbox: &Sandbox) {
    sandbox.write("project/package.json", "{}\n");
    sandbox.write("project/a.txt", "alpha\n");
    sandbox.write("project/lib/b.js", "beta\n");
    sandbox.write("project/lib/node_modules/z.js", "nested\n");
    sandbox.write("project/node_modules/x/y.js", "excluded\n");
}

#[test]
fn a_tree_digest_matches_the_recorded_shell_recipe() {
    let sandbox = Sandbox::new();
    sandbox.write("project/a.txt", "alpha\n");
    sandbox.write("project/lib/b.js", "beta\n");
    sandbox.write("project/lib/node_modules/z.js", "nested\n");
    sandbox.write("project/node_modules/x/y.js", "excluded\n");
    #[cfg(unix)]
    symlink("a.txt", sandbox.project().join("link")).unwrap();
    // The recipe's output on the same tree, symbolic link included.
    assert_eq!(
        tree_digest(&sandbox.project()).as_str(),
        "1992cb4d59814e7777a54aa3d1f38d8bd1470155cac6a8e239df6c6e67045310"
    );
}

#[test]
fn a_script_host_is_covered_by_its_whole_package() {
    let sandbox = Sandbox::new();
    package(&sandbox);
    let script = sandbox.write("project/dist/bundle/cli.js", "launcher\n");
    let (path, digest) = covered(&Code::PackageTree, &script);
    assert_eq!(path, sandbox.project());
    assert_eq!(digest, tree_digest(&sandbox.project()));
    assert_ne!(digest, Digest::of(b"launcher\n"));
}

#[test]
fn a_loader_host_is_covered_by_the_binary_it_launches() {
    let sandbox = Sandbox::new();
    package(&sandbox);
    let loader = sandbox.write("project/npm-loader.js", "loader\n");
    let binary = sandbox.write("project/node_modules/@scope/host-os/host", "native\n");
    let relative = || "node_modules/@scope/host-os/host".into();
    let (path, digest) = covered(&Code::PackageFile(relative), &loader);
    assert_eq!(path, binary);
    assert_eq!(digest, Digest::of(b"native\n"));
}

#[test]
fn a_native_host_is_covered_by_its_executable() {
    let sandbox = Sandbox::new();
    let executable = sandbox.write("home/host", "native\n");
    let (path, digest) = covered(&Code::Executable, &executable);
    assert_eq!(path, executable);
    assert_eq!(digest, Digest::of(&fs::read(&executable).unwrap()));
    assert_ne!(digest, Digest::of(b""));
}
