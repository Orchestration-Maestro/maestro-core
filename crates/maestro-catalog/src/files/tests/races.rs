use super::support::Scratch;
use crate::files::{FileInput, FilePlan, apply};
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};

#[test]
fn simultaneous_creates_never_replace_the_winning_bytes() {
    let scratch = Scratch::new();
    let first =
        FilePlan::preview(&scratch.path, [FileInput::new("same", b"first".to_vec())]).unwrap();
    let second =
        FilePlan::preview(&scratch.path, [FileInput::new("same", b"second".to_vec())]).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let (left, right) = thread::scope(|scope| {
        let first_barrier = Arc::clone(&barrier);
        let first_root = &scratch.path;
        let left = scope.spawn(move || {
            first_barrier.wait();
            apply(first_root, &first)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_root = &scratch.path;
        let right = scope.spawn(move || {
            second_barrier.wait();
            apply(second_root, &second)
        });
        (left.join().unwrap(), right.join().unwrap())
    });
    assert_ne!(left.is_ok(), right.is_ok());
    let stored = fs::read(scratch.path.join("same")).unwrap();
    assert!(stored == b"first" || stored == b"second");
}

#[test]
fn ancestor_link_swap_cannot_redirect_a_write_outside_the_root() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new();
        let outside = Scratch::new();
        fs::create_dir(scratch.path.join("parent")).unwrap();
        let plan = FilePlan::preview(
            &scratch.path,
            [FileInput::new("parent/file", b"planned".to_vec())],
        )
        .unwrap();
        fs::rename(scratch.path.join("parent"), scratch.path.join("moved")).unwrap();
        symlink(&outside.path, scratch.path.join("parent")).unwrap();
        assert!(apply(&scratch.path, &plan).is_err());
        assert!(!outside.path.join("file").exists());
    }
}
