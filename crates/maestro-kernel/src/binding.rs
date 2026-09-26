//! Named bindings: the local paths that the logical names of committed files
//! stand for, so that no committed file holds a machine path (ADR-0014). A
//! collection's declaration names its corpus `corpus_root`; this machine's
//! `bindings.toml`, in the kernel's configuration directory
//! ([`config_dir`](crate::paths::config_dir)), says where that is:
//!
//! ```toml
//! corpus_root = '/srv/corpora/handbook'
//! ```
//!
//! A literal string, in single quotes, keeps a Windows path's backslashes as
//! written. The file is checked whole when it is read: each binding must hold
//! an absolute path, so a caller resolves every name it needs before any work
//! starts. A missing file binds nothing.

use std::{
    collections::BTreeMap,
    error, fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

/// The bindings file's name in the kernel's configuration directory.
pub const FILE_NAME: &str = "bindings.toml";

/// This machine's bindings: each name with the absolute path it stands for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings(BTreeMap<String, PathBuf>);

impl Bindings {
    /// The bindings of [`FILE_NAME`] in `config_dir`; none when the file does
    /// not exist.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the file exists but cannot be read, and the refusals
    /// of [`Bindings::from_str`] for its text.
    pub fn load(config_dir: &Path) -> Result<Self, Error> {
        let path = config_dir.join(FILE_NAME);
        match fs::read_to_string(&path) {
            Ok(text) => text.parse(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io { path, source }),
        }
    }

    /// The absolute path `name` stands for.
    ///
    /// # Errors
    ///
    /// [`Error::Missing`], naming the binding, when nothing binds `name`.
    pub fn path(&self, name: &str) -> Result<&Path, Error> {
        self.0
            .get(name)
            .map(PathBuf::as_path)
            .ok_or_else(|| Error::Missing(name.to_owned()))
    }
}

impl FromStr for Bindings {
    type Err = Error;

    /// The bindings a bindings file holds.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] when the text is not TOML, a key bound twice
    /// included, and [`Error::NotAbsolute`], naming the binding, when a value
    /// is not a string holding an absolute path.
    fn from_str(text: &str) -> Result<Self, Error> {
        let table = text
            .parse::<toml::Table>()
            .map_err(|error| Error::Invalid(error.to_string()))?;
        table
            .into_iter()
            .map(|(name, value)| match value {
                toml::Value::String(path) if Path::new(&path).is_absolute() => {
                    Ok((name, PathBuf::from(path)))
                }
                _ => Err(Error::NotAbsolute(name)),
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

/// Why a name could not be resolved, or the bindings file could not be read.
#[derive(Debug)]
pub enum Error {
    /// Nothing binds this name.
    Missing(String),
    /// The binding of this name does not hold an absolute path.
    NotAbsolute(String),
    /// The file is not TOML; the parser's reason.
    Invalid(String),
    /// The file exists but cannot be read.
    Io {
        /// The bindings file.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(name) => write!(
                formatter,
                "nothing binds `{name}`: {FILE_NAME} in the kernel's configuration \
                 directory must give it an absolute path"
            ),
            Self::NotAbsolute(name) => write!(
                formatter,
                "the binding `{name}` in {FILE_NAME} does not hold an absolute path"
            ),
            Self::Invalid(reason) => write!(formatter, "{FILE_NAME} is not valid TOML: {reason}"),
            Self::Io { path, .. } => write!(formatter, "cannot read {}", path.display()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Missing(_) | Self::NotAbsolute(_) | Self::Invalid(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        fs::{self, File},
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    /// A new, empty directory for one test.
    fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-binding-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    /// A path that is absolute on the host running the test.
    fn absolute(name: &str) -> PathBuf {
        env::temp_dir().join(name)
    }

    /// The line of a bindings file that binds `name` to `path`, as a TOML
    /// literal string so a Windows path keeps its backslashes.
    fn line(name: &str, path: &Path) -> String {
        format!("{name} = '{}'\n", path.display())
    }

    #[test]
    fn a_bound_name_gives_its_absolute_path() {
        let corpus = absolute("corpus");
        let notes = absolute("notes");
        let text = line("corpus_root", &corpus) + &line("notes_root", &notes);
        let bindings: Bindings = text.parse().unwrap();
        assert_eq!(bindings.path("corpus_root").unwrap(), corpus);
        assert_eq!(bindings.path("notes_root").unwrap(), notes);
    }

    #[test]
    fn a_name_nothing_binds_is_refused_naming_it() {
        let bindings: Bindings = line("corpus_root", &absolute("corpus")).parse().unwrap();
        let refusal = bindings.path("archive_root").unwrap_err();
        assert!(
            matches!(&refusal, Error::Missing(name) if name == "archive_root"),
            "{refusal:?}"
        );
        assert!(refusal.to_string().contains("`archive_root`"), "{refusal}");
    }

    #[test]
    fn a_binding_that_is_not_an_absolute_path_is_refused_naming_it() {
        for text in [
            "corpus_root = 'relative/corpus'\n",
            "corpus_root = ''\n",
            "corpus_root = 7\n",
            "[corpus_root]\nroot = 'relative'\n",
        ] {
            let refusal = text.parse::<Bindings>().unwrap_err();
            assert!(
                matches!(&refusal, Error::NotAbsolute(name) if name == "corpus_root"),
                "{text}: {refusal:?}"
            );
            assert!(refusal.to_string().contains("`corpus_root`"), "{refusal}");
        }
    }

    #[test]
    fn a_file_that_is_not_toml_is_refused_a_key_bound_twice_included() {
        let twice =
            line("corpus_root", &absolute("first")) + &line("corpus_root", &absolute("second"));
        for text in [twice.as_str(), "corpus_root = \n"] {
            let refusal = text.parse::<Bindings>().unwrap_err();
            assert!(matches!(refusal, Error::Invalid(_)), "{text}: {refusal:?}");
            assert!(refusal.to_string().contains(FILE_NAME), "{refusal}");
        }
    }

    #[test]
    fn the_file_in_the_configuration_directory_is_read() {
        let directory = scratch();
        let corpus = absolute("corpus");
        fs::write(
            directory.join("bindings.toml"),
            line("corpus_root", &corpus),
        )
        .unwrap();
        let bindings = Bindings::load(&directory).unwrap();
        assert_eq!(bindings.path("corpus_root").unwrap(), corpus);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_missing_file_binds_nothing() {
        let directory = scratch();
        let bindings = Bindings::load(&directory).unwrap();
        assert_eq!(bindings, Bindings::default());
        assert!(matches!(
            bindings.path("corpus_root"),
            Err(Error::Missing(_))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_file_that_cannot_be_read_is_refused_with_its_path() {
        let directory = scratch();
        let file = directory.join("bindings.toml");
        fs::create_dir(&file).unwrap();
        let refusal = Bindings::load(&directory).unwrap_err();
        assert!(
            matches!(&refusal, Error::Io { path, .. } if *path == file),
            "{refusal:?}"
        );
        assert!(error::Error::source(&refusal).is_some());
        assert!(refusal.to_string().contains("bindings.toml"), "{refusal}");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_file_the_kernel_can_open_is_not_mistaken_for_a_missing_one() {
        let directory = scratch();
        File::create(directory.join("bindings.toml")).unwrap();
        assert_eq!(Bindings::load(&directory).unwrap(), Bindings::default());
        fs::remove_dir_all(directory).unwrap();
    }
}
