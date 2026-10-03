//! Functional naming from shared adapter metadata, with only exact host and
//! tool-required native-file exceptions. Packages provide no exception input.

use super::{
    descriptor::Scope,
    placements::{fits, join},
    tree::EntryKind,
};
use crate::adapters::REGISTERED;

/// Whether a name contains a registered product or alias as a whole token sequence.
pub(super) fn product(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let tokens: Vec<_> = name
        .split(['-', '_', '.'])
        .filter(|token| !token.is_empty())
        .collect();
    REGISTERED
        .iter()
        .flat_map(|adapter| adapter.product_names)
        .any(|alias| {
            let alias = alias.to_ascii_lowercase();
            let alias: Vec<_> = alias.split(['-', '_', '.']).collect();
            tokens.windows(alias.len()).any(|window| window == alias)
        })
}

/// Whether the tool-required filename is at one of its exact area-local placements.
fn native(path: &str, filename: &str) -> Option<bool> {
    let files = REGISTERED.iter().flat_map(|adapter| adapter.native_files);
    for file in files {
        if file.filename.eq_ignore_ascii_case(filename) {
            let allowed = [
                Scope::Common,
                Scope::Core,
                Scope::Team,
                Scope::Language,
                Scope::Standard,
            ]
            .iter()
            .any(|scope| {
                file.placements
                    .iter()
                    .any(|placement| fits(&join(scope.prefix(), placement), path))
            });
            return Some(allowed);
        }
    }
    None
}

/// Whether every path segment uses functional names, with exact approved exceptions.
pub(super) fn functional(path: &str, kind: EntryKind) -> bool {
    let mut prefix = String::new();
    for segment in path.split('/') {
        prefix = join(&prefix, segment);
        if prefix == path
            && let Some(allowed) = native(path, segment)
        {
            if !allowed {
                return false;
            }
            continue;
        }
        if product(segment)
            && !REGISTERED.iter().any(|adapter| {
                adapter.host_directory == Some(prefix.as_str())
                    && (prefix != path || kind == EntryKind::Directory)
            })
        {
            return false;
        }
    }
    true
}
