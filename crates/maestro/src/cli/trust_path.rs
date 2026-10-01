//! Shell-safe trust suggestions and lossless conventional Windows path spelling.
use std::path::Path;

/// Quote only characters literal in double quotes across the supported user shells.
fn quote_path(path: &str, windows: bool) -> Option<String> {
    if windows && path.ends_with('\\') {
        return None;
    }
    if !path.chars().all(|character| {
        character.is_alphanumeric()
            || " /._-+,@:~".contains(character)
            || (windows && character == '\\')
    }) {
        return None;
    }
    Some(format!("\"{path}\""))
}

/// Remove a verbatim prefix only when Win32 interpretation preserves every component.
#[cfg(any(windows, test))]
fn plain_windows_path(path: &str) -> Option<String> {
    let plain = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        let mut parts = unc.split('\\');
        if parts.next().is_none_or(str::is_empty) || parts.next().is_none_or(str::is_empty) {
            return None;
        }
        format!(r"\\{unc}")
    } else {
        let drive = path.strip_prefix(r"\\?\")?;
        let mut characters = drive.chars();
        if !characters.next()?.is_ascii_alphabetic()
            || characters.next()? != ':'
            || characters.next()? != '\\'
        {
            return None;
        }
        drive.to_owned()
    };
    if plain.encode_utf16().count() >= 260 {
        return None;
    }
    for component in plain.split('\\') {
        if component.ends_with('.') || component.ends_with(' ') {
            return None;
        }
        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str()) {
            return None;
        }
        if stem.chars().count() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem
                .chars()
                .last()
                .is_some_and(|digit| "123456789¹²³".contains(digit))
        {
            return None;
        }
    }
    Some(plain)
}

/// User-facing spelling; a lossy Windows conversion is never attempted.
pub(super) fn visible_path(path: &Path) -> String {
    let raw = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    if let Some(plain) = plain_windows_path(&raw) {
        return plain;
    }
    raw
}

/// Shell-safe canonical argument, or none for any spelling outside the allow-list.
pub(super) fn quoted_canonical(path: &Path) -> Option<String> {
    quote_path(&visible_path(path), cfg!(windows))
}

/// Safe user-facing suggestion; an unrenderable path is shown only as escaped data.
pub(super) fn suggestion(path: &Path) -> String {
    if let Some(quoted) = quoted_canonical(path) {
        format!("run: maestro trust add {quoted} --confirm-path {quoted}")
    } else {
        format!(
            concat!(
                "canonical path (data):\n{:?}\nQuote this path for your shell. ",
                "From inside that folder, run maestro trust add ."
            ),
            visible_path(path)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{plain_windows_path, quote_path, suggestion};
    use std::path::Path;

    #[test]
    fn trust_path_quote_allowlist_has_a_neighbour_for_each_refusal() {
        for (path, accepted) in [
            ("/work/project", true),
            ("/work/my project", true),
            ("/work/équipe", true),
            ("/work/$name", false),
            ("/work/name", true),
            ("/work/`name", false),
            ("/work/name", true),
            ("/work/\"name", false),
            ("/work/name", true),
            ("/work/'name", false),
            ("/work/name", true),
            ("/work/!name", false),
            ("/work/name", true),
            ("/work/%name", false),
            ("/work/name", true),
            ("/work/;name", false),
            ("/work/name", true),
            ("/work/|name", false),
            ("/work/name", true),
            ("/work/\nname", false),
            ("/work/name", true),
            ("/work/”name", false),
            ("/work/name", true),
            ("/work/—name", false),
            ("/work/-name", true),
            ("/work/\\name", false),
            ("/work/name", true),
        ] {
            assert_eq!(
                quote_path(path, false),
                accepted.then(|| format!("\"{path}\"")),
                "{path:?}"
            );
        }
        for (path, accepted) in [
            (r"C:\work\name", true),
            (r"C:\work\name\", false),
            (r"C:\work\name", true),
        ] {
            assert_eq!(
                quote_path(path, true),
                accepted.then(|| format!("\"{path}\"")),
                "{path:?}"
            );
        }
    }

    #[test]
    fn trust_path_suggestion_never_embeds_a_refused_path_in_a_command() {
        for (path, accepted) in [("/work/my project", true), ("/work/$name", false)] {
            let message = suggestion(Path::new(path));
            assert_eq!(message.contains("run: maestro trust add"), accepted);
            assert_eq!(message.contains("canonical path (data):"), !accepted);
        }
    }

    #[cfg(unix)]
    #[test]
    fn trust_path_non_utf8_names_are_data_not_lossy_commands() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt as _, path::PathBuf};
        let path = PathBuf::from(OsString::from_vec(b"/work/\xff".to_vec()));
        let message = suggestion(&path);
        assert!(!message.contains("run: maestro trust add"));
        assert!(suggestion(Path::new("/work/name")).contains("run: maestro trust add"));
    }

    #[test]
    fn trust_path_verbatim_mapping_is_lossless_or_refuses_with_neighbours() {
        let long = format!(r"\\?\C:\{}", "a".repeat(257));
        let short = format!(r"\\?\C:\{}", "a".repeat(256));
        for (path, expected) in [
            (r"C:\work\project", None),
            (r"\\?\C:\work\project", Some(r"C:\work\project")),
            (
                r"\\?\UNC\server\share\project",
                Some(r"\\server\share\project"),
            ),
            (r"\\?\UNC\server", None),
            (r"\\?\UNC\server\share", Some(r"\\server\share")),
            (r"\\?\UNC\\share", None),
            (r"\\?\UNC\server\share", Some(r"\\server\share")),
            (r"\\?\1:\work", None),
            (r"\\?\C:\work", Some(r"C:\work")),
            (r"\\?\C:/work", None),
            (r"\\?\C:\work", Some(r"C:\work")),
            (r"\\?\C:", None),
            (r"\\?\C:\work", Some(r"C:\work")),
            (r"\\?\Volume{synthetic}\project", None),
            (r"\\?\C:\project", Some(r"C:\project")),
            (r"\\?\C:\work.\project", None),
            (r"\\?\C:\work\project", Some(r"C:\work\project")),
            (r"\\?\C:\work \project", None),
            (r"\\?\C:\work\project", Some(r"C:\work\project")),
            (r"\\?\C:\CON", None),
            (r"\\?\C:\CONSOLE", Some(r"C:\CONSOLE")),
            (r"\\?\C:\con.txt", None),
            (r"\\?\C:\console.txt", Some(r"C:\console.txt")),
            (r"\\?\C:\PRN", None),
            (r"\\?\C:\PRNT", Some(r"C:\PRNT")),
            (r"\\?\C:\AUX", None),
            (r"\\?\C:\AUXILIARY", Some(r"C:\AUXILIARY")),
            (r"\\?\C:\NUL", None),
            (r"\\?\C:\NULL", Some(r"C:\NULL")),
            (r"\\?\C:\COM1.txt", None),
            (r"\\?\C:\COM¹", None),
            (r"\\?\C:\COM¹.txt", None),
            (r"\\?\C:\COM²", None),
            (r"\\?\C:\COM².txt", None),
            (r"\\?\C:\COM³", None),
            (r"\\?\C:\COM³.txt", None),
            (r"\\?\C:\LPT¹", None),
            (r"\\?\C:\LPT¹.txt", None),
            (r"\\?\C:\LPT²", None),
            (r"\\?\C:\LPT².txt", None),
            (r"\\?\C:\LPT³", None),
            (r"\\?\C:\LPT³.txt", None),
            (r"\\?\C:\COM0.txt", Some(r"C:\COM0.txt")),
            (r"\\?\C:\LPT9", None),
            (r"\\?\C:\LPT0", Some(r"C:\LPT0")),
            (&long, None),
            (&short, Some(&short[4..])),
        ] {
            assert_eq!(plain_windows_path(path).as_deref(), expected, "{path:?}");
        }
    }
}
