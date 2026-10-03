//! Only trusted default children transport their real LLVM exit profiles.
use std::{
    env, fs,
    io::copy,
    path::{Path, PathBuf},
    process::Command,
};

/// Give the restricted trusted child a private destination in its existing work view.
pub(super) fn configure(command: &mut Command, root: &Path) -> Option<PathBuf> {
    let parent = PathBuf::from(env::var_os("LLVM_PROFILE_FILE")?)
        .parent()?
        .to_path_buf();
    let profiles = root.join("work/llvm-profiles");
    fs::create_dir(&profiles).unwrap();
    command.env("LLVM_PROFILE_FILE", profiles.join("child-%p-%m.profraw"));
    Some(parent)
}

/// Copy only fresh, regular, non-symlink profiles before the probe's scratch is removed.
pub(super) fn collect(root: &Path, parent: Option<&Path>) {
    let Some(parent) = parent else {
        return;
    };
    let profiles = root.join("work/llvm-profiles");
    let mut count = 0;
    for entry in fs::read_dir(profiles).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_file());
        assert_eq!(path.extension().unwrap(), "profraw");
        assert!(fs::metadata(&path).unwrap().len() > 0);
        let name = format!(
            "{}-{}",
            root.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            entry.file_name().to_string_lossy()
        );
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(name))
            .unwrap();
        let mut source = fs::File::open(path).unwrap();
        copy(&mut source, &mut destination).unwrap();
        count += 1;
    }
    assert!(count > 0, "trusted child exited without its LLVM profile");
}
