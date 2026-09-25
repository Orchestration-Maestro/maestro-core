//! How each platform's dynamic loader is made to load the verified libraries, and only them.
//! Each rule is a value any host can test; the host only chooses which one applies to it.
use crate::error::Error;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

/// A platform's dynamic loader, as far as the counter's library search goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Loader {
    /// Linux's `ld.so`: `LD_LIBRARY_PATH` is searched before the counter's RUNPATH.
    LdSo,
    /// macOS's `dyld`: `DYLD_LIBRARY_PATH` is searched by leaf name before the install name,
    /// `@rpath` included. System Integrity Protection strips it from protected executables and
    /// the hardened runtime ignores it, so the counter must be neither.
    Dyld,
    /// Windows: no variable precedes the executable's own directory, then the system directories
    /// and the current directory; `PATH` comes last. Only a counter beside its libraries loads
    /// the verified ones.
    Windows,
}

impl Loader {
    /// The loader of the platform this crate is built for.
    pub(super) const HOST: Self = if cfg!(windows) {
        Self::Windows
    } else if cfg!(target_os = "macos") {
        Self::Dyld
    } else {
        Self::LdSo
    };

    /// The variable, and its value, that puts the library directory first in the search. On
    /// Windows it is `PATH`, the directory before the profile's `path`. A directory holding the
    /// list separator would be searched as two, so it is refused.
    pub(super) fn search_variable(
        self,
        directory: &Path,
        path: Option<&OsStr>,
    ) -> Result<(&'static str, OsString), Error> {
        let (name, separator) = match self {
            Self::LdSo => ("LD_LIBRARY_PATH", b':'),
            Self::Dyld => ("DYLD_LIBRARY_PATH", b':'),
            Self::Windows => ("PATH", b';'),
        };
        if directory
            .as_os_str()
            .as_encoded_bytes()
            .contains(&separator)
        {
            return Err(Error(
                "tokenizer library directory holds the loader's list separator".into(),
            ));
        }
        let mut value = directory.as_os_str().to_owned();
        if let Some(path) = path.filter(|_| self == Self::Windows) {
            value.push(";");
            value.push(path);
        }
        Ok((name, value))
    }

    /// Refuse a counter whose libraries this loader could find before the verified ones: on
    /// Windows, one outside the library directory. Both are compared resolved.
    pub(super) fn check_counter_location(
        self,
        counter: &Path,
        library_directory: &Path,
    ) -> Result<(), Error> {
        if self != Self::Windows {
            return Ok(());
        }
        let unavailable = |_| Error("tokenizer counter location unavailable".into());
        let counter = fs::canonicalize(counter).map_err(unavailable)?;
        if counter.parent()
            != Some(
                fs::canonicalize(library_directory)
                    .map_err(unavailable)?
                    .as_path(),
            )
        {
            return Err(Error(
                "tokenizer counter outside its library directory, which Windows requires".into(),
            ));
        }
        Ok(())
    }
}
