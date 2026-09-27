//! Local-path recognition shared by card identity validation.

/// Finds a local path token in prose while allowing single-segment slash tokens.
pub(super) fn contains_machine_path(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        let word = word.trim_matches(|character: char| {
            character.is_ascii_punctuation() && !matches!(character, '/' | '\\' | ':')
        });
        word.to_ascii_lowercase().starts_with("file://")
            || is_windows_drive_path(word)
            || word.starts_with(r"\\")
            || word.starts_with("//")
            || is_multi_segment_absolute_path(word)
    })
}

/// Identifies an absolute filesystem path or file URI in a path-bearing field.
pub(super) fn is_absolute_machine_path(value: &str) -> bool {
    let value = value.trim();
    value.to_ascii_lowercase().starts_with("file://")
        || value.starts_with('/')
        || value.starts_with('\\')
        || is_windows_drive_path(value)
}

/// Identifies local paths and multi-segment path values in runtime arguments.
pub(super) fn is_machine_path(value: &str) -> bool {
    is_absolute_machine_path(value)
        || value
            .split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .count()
            >= 2
}

/// Identifies a rooted Windows drive path, in either separator convention.
fn is_windows_drive_path(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(
        (bytes.next(), bytes.next(), bytes.next()),
        (Some(drive), Some(b':'), Some(b'/' | b'\\')) if drive.is_ascii_alphabetic()
    )
}

/// Recognizes a rooted path with at least two nonempty path segments.
fn is_multi_segment_absolute_path(value: &str) -> bool {
    value
        .strip_prefix('/')
        .or_else(|| value.strip_prefix('\\'))
        .is_some_and(|path| {
            path.split(['/', '\\'])
                .filter(|segment| !segment.is_empty())
                .count()
                >= 2
        })
}
