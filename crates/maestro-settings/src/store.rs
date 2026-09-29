//! The port a session reads its preferences layers through. Its file
//! adapter is [`FileLayers`](crate::FileLayers); another store (a test's,
//! S3's trust-guarded one) is another [`LayerSource`], and its callers do
//! not change.

use crate::{
    registry::Registry,
    resolve::{Layers, SettingsError},
};

/// The user's preferences file, in the configuration directory. The
/// kernel's `config.toml` beside it holds grants and is never read here.
pub const USER_FILE: &str = "preferences.toml";

/// Where a session's layers come from.
pub trait LayerSource {
    /// The layers, each file parsed whole against `registry`.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] naming a file that cannot be read or is refused.
    fn layers(&self, registry: &Registry) -> Result<Layers, SettingsError>;
}
