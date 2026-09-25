//! The binary's filesystem look-up of local asset destinations; the library itself does no I/O.
use maestro_canonicalization::{AssetStatus, CanonicalDocument, Error};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

/// The status of each unchecked asset destination, resolved under the input's directory.
pub(crate) fn asset_inventory(
    doc: &CanonicalDocument,
    root: &Path,
) -> Result<BTreeMap<String, AssetStatus>, Error> {
    let root = fs::canonicalize(root)
        .map_err(|error| Error(format!("cannot resolve asset root: {error}")))?;
    let destinations: BTreeSet<_> = doc
        .blocks
        .iter()
        .flat_map(|block| &block.asset_references)
        .filter(|asset| asset.status == AssetStatus::Unchecked)
        .map(|asset| asset.destination.clone())
        .collect();
    Ok(destinations
        .into_iter()
        .map(|url| {
            let status = local_asset_status(&url, &root);
            (url, status)
        })
        .collect())
}

/// Whether a destination names a regular file under the root: available, missing, outside the root,
/// or unchecked when it cannot be decoded or resolved.
fn local_asset_status(destination: &str, root: &Path) -> AssetStatus {
    let without_suffix = destination.split(['?', '#']).next().unwrap_or("");
    let Some(decoded) = percent_decode(without_suffix) else {
        return AssetStatus::Unchecked;
    };
    if decoded.is_empty() {
        return AssetStatus::Unchecked;
    }
    let mut relative = PathBuf::new();
    for component in Path::new(&decoded).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir if relative.pop() => {}
            _ => return AssetStatus::OutsideRoot,
        }
    }
    match fs::canonicalize(root.join(&relative)) {
        Ok(path) if !path.starts_with(root) => AssetStatus::OutsideRoot,
        Ok(path) if path.is_file() => AssetStatus::Available,
        Ok(_) => AssetStatus::Missing,
        Err(error) => match error.kind() {
            ErrorKind::NotFound | ErrorKind::NotADirectory
                if !crosses_non_directory(root, &relative) =>
            {
                AssetStatus::Missing
            }
            _ => AssetStatus::Unchecked,
        },
    }
}

/// Whether a directory on the way to `relative` exists but is not a directory, so the destination
/// cannot be resolved. Unix reports that as `ENOTDIR`, but Windows as `ERROR_PATH_NOT_FOUND`, the
/// same code as for a directory that does not exist, so the error kind alone cannot tell.
fn crosses_non_directory(root: &Path, relative: &Path) -> bool {
    relative
        .ancestors()
        .skip(1)
        .any(|ancestor| fs::metadata(root.join(ancestor)).is_ok_and(|meta| !meta.is_dir()))
}

/// Decode percent escapes into UTF-8 text; a malformed escape, invalid UTF-8 or a NUL gives
/// nothing.
fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = text.bytes();
    let mut decoded = Vec::with_capacity(text.len());
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            decoded.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded)
        .ok()
        .filter(|text| !text.contains('\0'))
}
