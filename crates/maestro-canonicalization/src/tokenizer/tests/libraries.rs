//! Tests of the library inventory: each platform's naming, its aliases and where they resolve.
//! Windows builds have no aliases, so its fixture needs no link and runs on every platform.
use super::super::artifacts::{verify_libraries, version_aliases};
use super::Scratch;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn each_platforms_library_names_give_their_version_aliases() {
    for (name, aliases) in [
        ("libggml.so.0.22.0", &["libggml.so", "libggml.so.0"][..]),
        ("libllama-cli-impl.so", &[]),
        (
            "libllama.0.3.0.dylib",
            &["libllama.dylib", "libllama.0.dylib"],
        ),
        ("libllama.0.dylib", &["libllama.dylib", "libllama.0.dylib"]),
        ("libllama-cli-impl.dylib", &[]),
        (
            "libllama.0.3.0.DYLIB",
            &["libllama.DYLIB", "libllama.0.DYLIB"],
        ),
        ("llama.dll", &[]),
        ("LLAMA.DLL", &[]),
        ("libllama.so.0.dll", &[]),
    ] {
        assert_eq!(version_aliases(name).unwrap(), aliases, "{name}");
    }
    for name in ["libggml.sox", "libggml.a", "llama.exe", "libggml"] {
        assert!(version_aliases(name).is_err(), "{name}");
    }
}

#[test]
fn a_windows_library_directory_holds_exactly_its_dlls() {
    let scratch = Scratch::new();
    let libraries = ["ggml.dll", "llama.dll"].map(|name| scratch.0.join(name));
    for path in &libraries {
        fs::write(path, b"abc").unwrap();
    }
    // Executables and import files sit beside the libraries in a build's `bin`.
    for other in ["llama-tokenize.exe", "llama.lib", "binding.json"] {
        fs::write(scratch.0.join(other), b"abc").unwrap();
    }
    verify_libraries(&libraries).unwrap();
    let extra = scratch.0.join("EXTRA.DLL");
    fs::write(&extra, b"abc").unwrap();
    assert!(verify_libraries(&libraries).is_err());
    fs::remove_file(extra).unwrap();
    let missing = [libraries[0].clone(), scratch.0.join("absent.dll")];
    assert!(verify_libraries(&missing).is_err());
    verify_libraries(&libraries).unwrap();
}

#[cfg(unix)]
#[test]
fn library_aliases_cannot_redirect_away_from_pinned_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("libfixture.so.1.2");
    fs::write(&path, b"abc").unwrap();
    let base = scratch.0.join("libfixture.so");
    symlink(&path, &base).unwrap();
    symlink(&path, scratch.0.join("libfixture.so.1")).unwrap();
    let libraries = [path.clone()];
    verify_libraries(&libraries).unwrap();
    fs::remove_file(&base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
    let outside = Scratch::new();
    let replacement = outside.0.join("libfixture.so.1.2");
    fs::write(&replacement, b"abc").unwrap();
    symlink(&replacement, &base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
    fs::remove_file(&base).unwrap();
    symlink(&path, &base).unwrap();
    let extra = scratch.0.join("libextra.so");
    fs::write(&extra, b"abc").unwrap();
    assert!(verify_libraries(&libraries).is_err());
    fs::remove_file(extra).unwrap();
    verify_libraries(&libraries).unwrap();
    // The pinned file itself may not be a link, even to the same bytes.
    fs::remove_file(&path).unwrap();
    symlink(&replacement, &path).unwrap();
    assert!(verify_libraries(&libraries).is_err());
}

#[cfg(unix)]
#[test]
fn macos_dylib_aliases_resolve_to_their_pinned_files() {
    let scratch = Scratch::new();
    let versioned = scratch.0.join("libfixture.1.2.dylib");
    let plain = scratch.0.join("libplain.dylib");
    for path in [&versioned, &plain] {
        fs::write(path, b"abc").unwrap();
    }
    let base = scratch.0.join("libfixture.dylib");
    symlink(&versioned, &base).unwrap();
    symlink("libfixture.1.2.dylib", scratch.0.join("libfixture.1.dylib")).unwrap();
    let libraries = [versioned, plain];
    verify_libraries(&libraries).unwrap();
    fs::remove_file(&base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
    symlink(&libraries[1], &base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
}

#[cfg(unix)]
#[test]
fn a_library_directory_reached_through_a_link_is_checked_alike() {
    let scratch = Scratch::new();
    let real = scratch.0.join("real");
    fs::create_dir(&real).unwrap();
    fs::write(real.join("libfixture.so.1.2"), b"abc").unwrap();
    for alias in ["libfixture.so", "libfixture.so.1"] {
        symlink("libfixture.so.1.2", real.join(alias)).unwrap();
    }
    let linked = scratch.0.join("linked");
    symlink(&real, &linked).unwrap();
    let libraries = [linked.join("libfixture.so.1.2")];
    verify_libraries(&libraries).unwrap();
    let outside = scratch.0.join("libfixture.so.1.2");
    fs::write(&outside, b"abc").unwrap();
    fs::remove_file(real.join("libfixture.so.1")).unwrap();
    symlink(&outside, real.join("libfixture.so.1")).unwrap();
    assert!(verify_libraries(&libraries).is_err());
}
