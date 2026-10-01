//! Descriptor placement patterns, shared by registration and discovery.

use super::{
    descriptor::{KindDescriptor, Layout, MetadataPlace, Scope},
    parse::is_name,
};

/// Join two catalog-relative paths, preserving the root's empty spelling.
pub(super) fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else if name.is_empty() {
        directory.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

/// A portable exact relative path, or a folder pattern when `wildcards` is true.
pub(super) fn safe(path: &str, wildcards: bool) -> bool {
    path.is_empty()
        || path.split('/').all(|part| {
            !matches!(part, "" | "." | "..")
                && !part.contains(['\\', ':'])
                && (!part.contains('*') || (wildcards && part == "*"))
        })
}

/// Whether a fixed/wildcard pattern names this exact path.
pub(super) fn fits(pattern: &str, path: &str) -> bool {
    let pattern: Vec<_> = pattern.split('/').collect();
    let path: Vec<_> = path.split('/').collect();
    pattern.len() == path.len()
        && pattern
            .iter()
            .zip(path)
            .all(|(expected, actual)| *expected == actual || (*expected == "*" && is_name(actual)))
}

/// Directory patterns instantiated by the descriptor's scopes.
pub(super) fn directories(descriptor: &KindDescriptor) -> Vec<String> {
    descriptor
        .scopes
        .iter()
        .map(|scope| join(scope.prefix(), &descriptor.directory))
        .collect()
}

/// The descriptor's resource-bearing placements, not their ancestors.
pub(super) fn occupied(descriptor: &KindDescriptor) -> Vec<String> {
    directories(descriptor)
        .into_iter()
        .flat_map(|directory| match &descriptor.layout {
            Layout::Files { folders, .. } => folders
                .iter()
                .map(|folder| join(&directory, folder))
                .collect(),
            Layout::Folder { .. } => vec![join(&directory, "*")],
            Layout::Single { file, .. } | Layout::Area { file } => vec![join(&directory, file)],
        })
        .collect()
}

/// Whether placements can collide, including one owning the other's descendants.
pub(super) fn overlaps(left: &str, right: &str) -> bool {
    left.is_empty()
        || right.is_empty()
        || left
            .split('/')
            .zip(right.split('/'))
            .all(|(left, right)| left == right || left == "*" || right == "*")
}

/// Refuse traversal, wildcard inventories and internally overlapping placements.
pub(super) fn problem(descriptor: &KindDescriptor) -> Option<String> {
    if !safe(&descriptor.directory, false) {
        return Some("unsafe relative placement".to_owned());
    }
    if let Some(problem) = scope_problem(descriptor) {
        return Some(problem);
    }
    if let MetadataPlace::Sidecar { suffix } = &descriptor.metadata
        && !portable_suffix(suffix)
    {
        return Some("unsafe sidecar suffix".to_owned());
    }
    let valid = match &descriptor.layout {
        Layout::Files { suffix, folders } => {
            portable_suffix(suffix)
                && !folders.is_empty()
                && folders.iter().all(|folder| safe(folder, true))
        }
        Layout::Folder { file, data } => {
            !file.is_empty()
                && !file.contains('/')
                && safe(file, false)
                && data
                    .iter()
                    .all(|asset| !asset.is_empty() && safe(asset, false) && asset != file)
        }
        Layout::Area { file } => {
            descriptor.directory.is_empty()
                && !file.is_empty()
                && !file.contains('/')
                && safe(file, false)
        }
        Layout::Single { file, name } => {
            !file.is_empty() && !file.contains('/') && safe(file, false) && is_name(name)
        }
    };
    if !valid {
        return Some("unsafe layout or asset inventory".to_owned());
    }
    let paths = occupied(descriptor);
    for (index, left) in paths.iter().enumerate() {
        if paths
            .iter()
            .skip(index + 1)
            .any(|right| overlaps(left, right))
        {
            return Some("overlapping placement".to_owned());
        }
    }
    if let Layout::Folder { data, .. } = &descriptor.layout {
        for (index, left) in data.iter().enumerate() {
            if data
                .iter()
                .skip(index + 1)
                .any(|right| overlaps(left, right))
            {
                return Some("overlapping asset inventory".to_owned());
            }
        }
    }
    None
}

/// Primary and sidecar suffixes use the same portable, separator-free spelling.
fn portable_suffix(suffix: &str) -> bool {
    !suffix.is_empty() && !suffix.contains(['/', '\\', ':', '*'])
}

/// Scope boundaries apply to every descriptor-relative placement segment.
fn scope_problem(descriptor: &KindDescriptor) -> Option<String> {
    let mut relative = vec![descriptor.directory.clone()];
    match &descriptor.layout {
        Layout::Files { folders, .. } => relative.extend(folders.iter().cloned()),
        Layout::Folder { data, .. } => relative.extend(data.iter().cloned()),
        Layout::Single { .. } | Layout::Area { .. } => {}
    }
    for scope in &descriptor.scopes {
        if *scope == Scope::Root {
            let first = descriptor.directory.split('/').next().unwrap_or_default();
            if !Scope::SUPPORT_ROOTS.contains(&first) {
                return Some("unregistered root support placement".to_owned());
            }
        } else if relative.iter().any(|path| nested(path)) {
            return Some("nested area placement".to_owned());
        }
    }
    None
}

/// An area-root segment cannot be recreated below an area placement.
fn nested(path: &str) -> bool {
    path.split('/').any(|part| {
        Scope::AREA_ROOTS
            .iter()
            .any(|scope| scope.prefix().split('/').next() == Some(part))
    })
}

/// Concrete descriptor-relative placements cannot recreate an area boundary.
/// Scope prefixes themselves and Root support trees retain their semantics.
pub(super) fn concrete(descriptor: &KindDescriptor, directory: &str) -> bool {
    descriptor.scopes.iter().any(|scope| {
        let prefix = scope.prefix();
        let count = if prefix.is_empty() {
            0
        } else {
            prefix.split('/').count()
        };
        let relative = directory
            .split('/')
            .skip(count)
            .collect::<Vec<_>>()
            .join("/");
        let placement = join(prefix, &descriptor.directory);
        let placement_count = if placement.is_empty() {
            0
        } else {
            placement.split('/').count()
        };
        let boundary = directory
            .split('/')
            .take(placement_count)
            .collect::<Vec<_>>()
            .join("/");
        fits(&placement, &boundary) && (*scope == Scope::Root || !nested(&relative))
    })
}
