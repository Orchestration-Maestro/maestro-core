use crate::{Directory, read::read_limited, tests::scratch};
use std::{
    fs,
    io::{self, Read},
    path::Path,
};

/// An endless reader that errors before returning more than its byte budget.
struct CountingReader {
    /// Bytes still permitted to be requested and returned.
    remaining: usize,
}

impl Read for CountingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.len() > self.remaining {
            return Err(io::Error::other("read exceeded max + 1 bytes"));
        }
        buffer.fill(b'x');
        self.remaining -= buffer.len();
        Ok(buffer.len())
    }
}

#[test]
fn read_limited_never_requests_more_than_limit_plus_one() {
    let error = read_limited(CountingReader { remaining: 5 }, 4).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
    assert_eq!(error.to_string(), "file is larger than 4 bytes");
}

#[test]
fn read_regular_bounded_accepts_at_limit_and_refuses_one_past() {
    let root = scratch();
    fs::write(root.join("file"), b"data").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(directory.read_regular_bounded("file", 4).unwrap(), b"data");
    fs::write(root.join("file"), b"data!").unwrap();
    let error = directory.read_regular_bounded("file", 4).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
    assert_eq!(error.to_string(), "file is larger than 4 bytes");
    // Existing unbounded callers keep their original behavior.
    assert_eq!(directory.read_regular("file").unwrap(), b"data!");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_limited_handles_zero_and_maximum_limits() {
    assert_eq!(read_limited(&b""[..], 0).unwrap(), b"");
    assert_eq!(
        read_limited(&b"x"[..], 0).unwrap_err().kind(),
        io::ErrorKind::FileTooLarge
    );
    assert_eq!(read_limited(&b"data"[..], u64::MAX).unwrap(), b"data");
}

#[test]
fn read_regular_bounded_refuses_non_regular_files_and_links() {
    let root = scratch();
    fs::create_dir(root.join("directory")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(directory.read_regular_bounded("directory", 4).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        fs::write(root.join("target"), b"data").unwrap();
        symlink(root.join("target"), root.join("link")).unwrap();
        assert!(directory.read_regular_bounded("link", 4).is_err());
    }
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}
