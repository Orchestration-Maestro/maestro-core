//! Loading the bound source-class table.

use crate::failure::Failure;
use maestro_kernel::binding::Bindings;
use maestro_knowledge::search::SourceClassTable;
use std::{fs, path::Path, sync::Arc};

/// The binding that names the source-class table.
pub(crate) const BINDING: &str = "source_classes";

/// The table the bindings of `config_dir` name; none when nothing binds it.
///
/// # Errors
///
/// [`Failure::Refused`] when the bindings file is invalid, or the bound
/// table cannot be read or is not a `maestro-source-classes/1` table.
pub(crate) fn load(config_dir: &Path) -> Result<Option<Arc<SourceClassTable>>, Failure> {
    let bindings = Bindings::load(config_dir).map_err(|error| Failure::refused_by(&error))?;
    let Ok(path) = bindings.path(BINDING) else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|error| {
        Failure::refused(format!(
            "cannot read the source-class table {}: {error}",
            path.display()
        ))
    })?;
    SourceClassTable::parse(&bytes)
        .map(|table| Some(Arc::new(table)))
        .map_err(|error| Failure::refused_by(&error))
}
