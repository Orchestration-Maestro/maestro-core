//! Where the kernel keeps its data: `$XDG_DATA_HOME/maestro` when that names an
//! absolute directory, on every platform; otherwise
//! `$HOME/.local/share/maestro` on Linux and macOS, and
//! `%LOCALAPPDATA%\maestro` on Windows.

use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
};

/// No variable names an absolute directory for the kernel's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoDataHome;

impl fmt::Display for NoDataHome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "no data directory: neither XDG_DATA_HOME nor the platform's data home \
             (HOME on Linux and macOS, LOCALAPPDATA on Windows) names an absolute path",
        )
    }
}

impl Error for NoDataHome {}

/// The variables the kernel's directories come from. The binary reads them
/// once, with [`Environment::current`]; a test or a caller names its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// `XDG_DATA_HOME`, honoured on every platform.
    pub xdg_data_home: Option<OsString>,
    /// `HOME`, whose `.local/share` is the data home on Linux and macOS.
    pub home: Option<OsString>,
    /// `LOCALAPPDATA`, the data home on Windows.
    pub local_app_data: Option<OsString>,
}

impl Environment {
    /// The values of the running process.
    #[must_use]
    pub fn current() -> Self {
        Self {
            xdg_data_home: env::var_os("XDG_DATA_HOME"),
            home: env::var_os("HOME"),
            local_app_data: env::var_os("LOCALAPPDATA"),
        }
    }
}

/// The kernel's data directory on the platform this build runs on.
///
/// An unset, empty or relative `XDG_DATA_HOME` is ignored, as the XDG Base
/// Directory specification requires, and the platform's data home takes its
/// place.
///
/// # Errors
///
/// [`NoDataHome`] when no variable the platform reads names an absolute path.
pub fn data_dir(environment: &Environment) -> Result<PathBuf, NoDataHome> {
    data_dir_on(Family::current(), environment)
}

/// The operating-system families whose conventions for data differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// Linux and macOS.
    Unix,
    /// Windows.
    Windows,
}

impl Family {
    /// The family this build runs on.
    const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// [`data_dir`] on `family`: a function of its inputs alone, so every host
/// tests both families.
fn data_dir_on(family: Family, environment: &Environment) -> Result<PathBuf, NoDataHome> {
    let platform_home = || match family {
        Family::Unix => {
            absolute(environment.home.as_ref()).map(|home| home.join(".local").join("share"))
        }
        Family::Windows => absolute(environment.local_app_data.as_ref()).map(Path::to_path_buf),
    };
    absolute(environment.xdg_data_home.as_ref())
        .map(Path::to_path_buf)
        .or_else(platform_home)
        .map(|base| base.join("maestro"))
        .ok_or(NoDataHome)
}

/// `value` as a path, when it is an absolute one on this host.
fn absolute(value: Option<&OsString>) -> Option<&Path> {
    value.map(Path::new).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path that is absolute on the host running the test, whatever it is.
    fn absolute_path(name: &str) -> OsString {
        env::temp_dir().join(name).into_os_string()
    }

    fn environment(
        xdg_data_home: Option<OsString>,
        home: Option<OsString>,
        local_app_data: Option<OsString>,
    ) -> Environment {
        Environment {
            xdg_data_home,
            home,
            local_app_data,
        }
    }

    /// The values of `XDG_DATA_HOME` the specification says to ignore.
    fn ignored_xdg_values() -> [Option<OsString>; 3] {
        [None, Some(OsString::new()), Some(OsString::from("data"))]
    }

    #[test]
    fn an_absolute_xdg_data_home_is_used_on_every_family() {
        let data = absolute_path("data");
        let given = environment(
            Some(data.clone()),
            Some(absolute_path("home")),
            Some(absolute_path("local")),
        );
        let expected = Ok(PathBuf::from(&data).join("maestro"));
        assert_eq!(data_dir_on(Family::Unix, &given), expected);
        assert_eq!(data_dir_on(Family::Windows, &given), expected);
    }

    #[test]
    fn linux_and_macos_fall_back_to_the_home_directory() {
        let home = absolute_path("home");
        let expected = Ok(PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("maestro"));
        for xdg_data_home in ignored_xdg_values() {
            let given = environment(
                xdg_data_home,
                Some(home.clone()),
                Some(absolute_path("local")),
            );
            assert_eq!(data_dir_on(Family::Unix, &given), expected);
        }
    }

    #[test]
    fn windows_falls_back_to_the_local_application_data() {
        let local = absolute_path("local");
        let expected = Ok(PathBuf::from(&local).join("maestro"));
        for xdg_data_home in ignored_xdg_values() {
            let given = environment(
                xdg_data_home,
                Some(absolute_path("home")),
                Some(local.clone()),
            );
            assert_eq!(data_dir_on(Family::Windows, &given), expected);
        }
    }

    #[test]
    fn no_absolute_directory_for_the_family_is_refused() {
        let relative = || Some(OsString::from("relative"));
        let only_home = environment(relative(), Some(absolute_path("home")), relative());
        assert_eq!(
            data_dir_on(Family::Windows, &only_home),
            Err(NoDataHome),
            "HOME does not count on Windows"
        );
        let only_local = environment(relative(), relative(), Some(absolute_path("local")));
        assert_eq!(
            data_dir_on(Family::Unix, &only_local),
            Err(NoDataHome),
            "LOCALAPPDATA does not count on Linux and macOS"
        );
        assert_eq!(
            data_dir_on(Family::Unix, &Environment::default()),
            Err(NoDataHome)
        );
    }

    #[test]
    fn the_running_process_gives_its_own_family_and_variables() {
        assert_eq!(Family::current() == Family::Windows, cfg!(windows));
        let current = Environment::current();
        assert_eq!(current.xdg_data_home, env::var_os("XDG_DATA_HOME"));
        assert_eq!(current.home, env::var_os("HOME"));
        assert_eq!(current.local_app_data, env::var_os("LOCALAPPDATA"));
        assert_eq!(data_dir(&current), data_dir_on(Family::current(), &current));
    }

    #[test]
    fn the_refusal_names_every_variable_it_reads() {
        let message = NoDataHome.to_string();
        for name in ["XDG_DATA_HOME", "(HOME on", "LOCALAPPDATA"] {
            assert!(message.contains(name), "{name} is missing from: {message}");
        }
    }
}
