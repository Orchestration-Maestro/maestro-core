//! Tests of the artifact store: digests, writes, repairs and refusals, with
//! the same outcomes on Linux, macOS and Windows.

use super::{Digest, Error, Store, store::NEXT_TEMPORARY};
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::{
    env, error,
    fs::{self, File},
    path::{Path, PathBuf},
    process,
    sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};

/// SHA-256 of the empty input.
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// SHA-256 of `abc`.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-artifact-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn stored(root: &Path, hex: &str) -> PathBuf {
    root.join("sha256")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(hex)
}

/// Waits for every other writer, then puts `abc`.
fn put_together(store: &Store, start: &Barrier) -> Result<Digest, Error> {
    start.wait();
    store.put(b"abc")
}

#[test]
fn a_digest_is_the_sha256_of_the_bytes_in_lowercase_hex() {
    assert_eq!(Digest::of(b"").as_str(), EMPTY);
    assert_eq!(Digest::of(b"abc").as_str(), ABC);
}

#[test]
fn only_64_lowercase_hex_characters_parse_as_a_digest() {
    assert_eq!(Digest::parse(ABC).unwrap(), Digest::of(b"abc"));
    let refused = [
        ABC[..63].to_owned(),
        format!("{ABC}0"),
        ABC.to_uppercase(),
        "g".repeat(64),
        format!("../{}", &ABC[3..]),
        String::new(),
    ];
    for text in refused {
        assert!(
            matches!(Digest::parse(&text), Err(invalid) if invalid.text() == text),
            "{text:?} was accepted"
        );
    }
}

#[test]
fn put_stores_the_bytes_under_their_digest_and_get_returns_them() {
    let scratch = Scratch::new();
    let store = Store::new(&scratch.0);
    let digest = store.put(b"abc").unwrap();
    assert_eq!(digest.as_str(), ABC);
    assert_eq!(fs::read(stored(&scratch.0, ABC)).unwrap(), b"abc");
    assert_eq!(store.get(&digest).unwrap(), b"abc");
    assert_eq!(store.put(b"").unwrap().as_str(), EMPTY);
    assert!(store.get(&Digest::of(b"")).unwrap().is_empty());
}

#[test]
fn a_second_put_of_the_same_bytes_writes_nothing() {
    let scratch = Scratch::new();
    let store = Store::new(&scratch.0);
    store.put(b"abc").unwrap();
    let path = stored(&scratch.0, ABC);
    // A time no write could give the file: a rewrite would replace it.
    let planted = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(planted)
        .unwrap();
    assert_eq!(store.put(b"abc").unwrap().as_str(), ABC);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), planted);
    let entries = fs::read_dir(path.parent().unwrap()).unwrap().count();
    assert_eq!(entries, 1, "no temporary file is left behind");
}

#[test]
fn get_refuses_bytes_that_no_longer_match_and_put_repairs_them() {
    let scratch = Scratch::new();
    let store = Store::new(&scratch.0);
    let digest = store.put(b"abc").unwrap();
    let artifact = stored(&scratch.0, ABC);
    // A second name for the same file: a write in place shows through it.
    let witness = scratch.0.join("witness");
    fs::hard_link(&artifact, &witness).unwrap();
    fs::write(&artifact, b"tampered").unwrap();
    assert!(matches!(
        store.get(&digest),
        Err(Error::Corrupt { expected, found })
            if expected == digest && found == Digest::of(b"tampered")
    ));
    store.put(b"abc").unwrap();
    assert_eq!(store.get(&digest).unwrap(), b"abc");
    assert_eq!(
        fs::read(&witness).unwrap(),
        b"tampered",
        "the repair is a new file renamed into place, never a write in place"
    );
}

#[test]
fn a_put_that_cannot_move_its_bytes_into_place_leaves_no_temporary_file() {
    let scratch = Scratch::new();
    let artifact = stored(&scratch.0, ABC);
    fs::create_dir_all(artifact.join("occupied")).unwrap();
    let error = Store::new(&scratch.0).put(b"abc").unwrap_err();
    assert!(
        matches!(&error, Error::Io { path, .. } if *path == artifact),
        "{error}"
    );
    assert!(artifact.is_dir());
    let names: Vec<_> = fs::read_dir(artifact.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, [artifact.file_name().unwrap()]);
}

#[test]
fn a_put_whose_temporary_name_is_taken_fails_and_leaves_that_file_alone() {
    let scratch = Scratch::new();
    let artifact = stored(&scratch.0, ABC);
    let directory = artifact.parent().unwrap();
    fs::create_dir_all(directory).unwrap();
    // Tests running beside this one take numbers too, far fewer than 64.
    let next = NEXT_TEMPORARY.load(Ordering::Relaxed);
    let taken: Vec<PathBuf> = (next..next + 64)
        .map(|number| directory.join(format!(".tmp-{}-{number}", process::id())))
        .collect();
    for path in &taken {
        fs::write(path, b"another writer").unwrap();
    }
    let error = Store::new(&scratch.0).put(b"abc").unwrap_err();
    assert!(
        matches!(&error, Error::Io { path, .. } if taken.contains(path)),
        "{error}"
    );
    for path in &taken {
        assert_eq!(
            fs::read(path).unwrap(),
            b"another writer",
            "{}",
            path.display()
        );
    }
    assert!(!artifact.exists());
}

#[test]
fn concurrent_puts_of_the_same_bytes_all_succeed_and_leave_one_file() {
    let scratch = Scratch::new();
    let store = Store::new(&scratch.0);
    let start = Barrier::new(8);
    let digests: Vec<_> = thread::scope(|scope| {
        let writers: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| put_together(&store, &start)))
            .collect();
        writers
            .into_iter()
            .map(|writer| writer.join().unwrap())
            .collect()
    });
    assert!(
        digests
            .iter()
            .all(|digest| matches!(digest, Ok(found) if found.as_str() == ABC)),
        "{digests:?}"
    );
    assert_eq!(store.get(&Digest::of(b"abc")).unwrap(), b"abc");
    let artifact = stored(&scratch.0, ABC);
    assert_eq!(fs::read_dir(artifact.parent().unwrap()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn the_store_creates_what_is_missing_for_its_owner_only() {
    let scratch = Scratch::new();
    let root = scratch.0.join("share").join("maestro");
    Store::new(&root).put(b"abc").unwrap();
    let artifact = stored(&root, ABC);
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&artifact), 0o600);
    for directory in artifact.ancestors().skip(1).take(5) {
        assert_eq!(mode(directory), 0o700, "{}", directory.display());
    }
}

// Windows creates a symbolic link only with a privilege CI lacks; its
// junctions are directories, which the directory test covers.
#[cfg(unix)]
#[test]
fn get_refuses_a_link_and_put_replaces_it_with_the_bytes() {
    let scratch = Scratch::new();
    let store = Store::new(&scratch.0);
    let digest = store.put(b"abc").unwrap();
    let artifact = stored(&scratch.0, ABC);
    let elsewhere = scratch.0.join("elsewhere");
    fs::rename(&artifact, &elsewhere).unwrap();
    symlink(&elsewhere, &artifact).unwrap();
    assert!(matches!(store.get(&digest), Err(Error::Io { path, .. }) if path == artifact));
    store.put(b"abc").unwrap();
    assert!(fs::symlink_metadata(&artifact).unwrap().is_file());
    assert_eq!(store.get(&digest).unwrap(), b"abc");
    assert_eq!(
        fs::read(&elsewhere).unwrap(),
        b"abc",
        "the link's target is kept"
    );
}

#[test]
fn a_relative_root_is_resolved_once_against_the_current_directory() {
    let absolute = env::current_dir().unwrap().join("artifacts");
    assert_eq!(
        format!("{:?}", Store::new("artifacts")),
        format!("{:?}", Store::new(absolute))
    );
}

#[test]
fn get_of_a_digest_never_stored_is_missing() {
    let scratch = Scratch::new();
    let unknown = Digest::of(b"never stored");
    let result = Store::new(&scratch.0).get(&unknown);
    assert!(matches!(result, Err(Error::Missing(found)) if found == unknown));
}

#[test]
fn a_root_that_cannot_hold_directories_is_an_io_error_naming_its_path() {
    let scratch = Scratch::new();
    let file = scratch.0.join("not-a-directory");
    fs::write(&file, b"").unwrap();
    let error = Store::new(&file).put(b"abc").unwrap_err();
    let Error::Io { path, source } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(*path, file.join("sha256"));
    // The reason is the error's source, printed once by whoever walks the chain.
    assert_eq!(
        error.to_string(),
        format!("cannot access {}", path.display())
    );
    let reason = error::Error::source(&error).map(ToString::to_string);
    assert_eq!(reason, Some(source.to_string()));
}

#[test]
fn a_stored_path_that_is_a_directory_is_an_io_error_not_a_missing_artifact() {
    let scratch = Scratch::new();
    fs::create_dir_all(stored(&scratch.0, ABC)).unwrap();
    let result = Store::new(&scratch.0).get(&Digest::of(b"abc"));
    assert!(matches!(result, Err(Error::Io { .. })));
}

#[test]
fn every_error_says_what_went_wrong() {
    let invalid = Digest::parse("xyz").unwrap_err().to_string();
    assert!(
        invalid.contains("\"xyz\"") && invalid.contains("not a SHA-256"),
        "{invalid}"
    );
    let missing = Error::Missing(Digest::of(b"abc"));
    assert!(missing.to_string().contains(ABC), "{missing}");
    assert!(error::Error::source(&missing).is_none());
    let corrupt = Error::Corrupt {
        expected: Digest::of(b"abc"),
        found: Digest::of(b""),
    }
    .to_string();
    assert!(
        corrupt.contains(ABC) && corrupt.contains(EMPTY) && corrupt.contains("no longer"),
        "{corrupt}"
    );
}
