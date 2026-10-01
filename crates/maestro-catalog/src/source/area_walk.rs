//! Scoped discovery over the bounded snapshot. Placements and inert assets
//! come only from descriptors; there is no resource-kind dispatch.

use super::{
    descriptor::{KindDescriptor, Layout, MetadataPlace},
    discovered::{Found, Unit, refusal},
    naming,
    parse::is_name,
    placements::{concrete, directories, fits, join},
    registry::Registry,
    scan::Snapshot,
    tree::{Entry, EntryKind},
    types::{Diagnostic, Refusal},
};
use crate::limits::Limits;
use std::collections::BTreeSet;

/// One scoped discovery attempt with aggregate resource counts.
struct Walker<'a> {
    /// Bounded listings, including all inert/support trees.
    snapshot: &'a Snapshot,
    /// The shared limits, never reset per area.
    limits: &'a Limits,
    /// The discovered resources and refusals.
    found: Found,
    /// Exact file placements claimed by descriptors.
    consumed: BTreeSet<String>,
}

/// Finds scoped resources, refusing every nonempty unclaimed placement.
pub(super) fn discover(
    snapshot: &Snapshot,
    registry: &Registry,
    limits: &Limits,
) -> Result<Found, Refusal> {
    let mut walker = Walker {
        snapshot,
        limits,
        found: Found {
            diagnostics: snapshot.diagnostics.clone(),
            ..Found::default()
        },
        consumed: BTreeSet::new(),
    };
    for registration in registry.registrations() {
        let descriptor = &registration.descriptor;
        let before = walker.found.units.len();
        for pattern in directories(descriptor) {
            walker.placement(descriptor, &pattern)?;
        }
        if walker.found.units.len() == before
            && let Some(reason) = &descriptor.required
        {
            walker.note(&descriptor.directory, &format!("missing: {reason}"));
        }
    }
    walker.unclaimed();
    Ok(walker.found)
}

impl Walker<'_> {
    /// Discover one descriptor placement according to its data-defined layout.
    fn placement(&mut self, descriptor: &KindDescriptor, pattern: &str) -> Result<(), Refusal> {
        match &descriptor.layout {
            Layout::Files { suffix, folders } => {
                for folder in folders {
                    self.files(descriptor, &join(pattern, folder), suffix)?;
                }
            }
            Layout::Folder { file, data } => {
                for directory in self
                    .snapshot
                    .directories
                    .keys()
                    .filter(|directory| fits(&join(pattern, "*"), directory))
                {
                    self.folder(descriptor, directory, (file, data))?;
                }
            }
            Layout::Area { file } => {
                for directory in self
                    .snapshot
                    .directories
                    .keys()
                    .filter(|directory| fits(pattern, directory))
                {
                    let name = directory
                        .rsplit('/')
                        .next()
                        .filter(|name| !name.is_empty())
                        .unwrap_or("common");
                    self.single(descriptor, &join(directory, file), name)?;
                }
            }
            Layout::Single { file, name } => {
                for directory in self
                    .snapshot
                    .directories
                    .keys()
                    .filter(|directory| fits(pattern, directory))
                {
                    self.single(descriptor, &join(directory, file), name)?;
                }
            }
        }
        Ok(())
    }

    /// Discover all files in one resource-bearing directory pattern.
    fn files(
        &mut self,
        descriptor: &KindDescriptor,
        pattern: &str,
        suffix: &str,
    ) -> Result<(), Refusal> {
        for (directory, entries) in &self.snapshot.directories {
            if !fits(pattern, directory) || !self.concrete(descriptor, directory) {
                continue;
            }
            for entry in entries {
                self.file(descriptor, directory, (entry, suffix))?;
            }
        }
        Ok(())
    }

    /// Discover one exact primary file, when present and regular.
    fn single(
        &mut self,
        descriptor: &KindDescriptor,
        path: &str,
        name: &str,
    ) -> Result<(), Refusal> {
        self.consumed.insert(path.to_owned());
        if exists(self.snapshot, path) {
            self.push(Unit::new(descriptor, name, path.to_owned()))?;
        }
        Ok(())
    }

    /// Pair one file and its exact sidecar, never a directory/link substitute.
    fn file(
        &mut self,
        descriptor: &KindDescriptor,
        directory: &str,
        (entry, suffix): (&Entry, &str),
    ) -> Result<(), Refusal> {
        if entry.kind == EntryKind::Directory {
            return Ok(());
        }
        if let MetadataPlace::Sidecar { suffix } = &descriptor.metadata
            && entry.name.ends_with(suffix)
        {
            return Ok(());
        }
        let Some(stem) = entry.name.strip_suffix(suffix) else {
            return Ok(());
        };
        let path = join(directory, &entry.name);
        self.consumed.insert(path.clone());
        let mut unit = Unit::new(descriptor, stem, path);
        if let MetadataPlace::Sidecar { suffix } = &descriptor.metadata {
            let sidecar = join(directory, &format!("{stem}{suffix}"));
            self.consumed.insert(sidecar.clone());
            if !exists(self.snapshot, &sidecar) {
                self.note(&unit.path, &format!("no {stem}{suffix} sidecar beside it"));
                return Ok(());
            }
            unit.sidecar = Some(sidecar);
        }
        if entry.kind == EntryKind::File {
            self.push(unit)?;
        }
        Ok(())
    }

    /// One folder resource with exact owner-local inert assets.
    fn folder(
        &mut self,
        descriptor: &KindDescriptor,
        directory: &str,
        (file, data): (&str, &[String]),
    ) -> Result<(), Refusal> {
        if !self.concrete(descriptor, directory) {
            return Ok(());
        }
        let name = directory.rsplit('/').next().unwrap_or(directory);
        let path = join(directory, file);
        self.consumed.insert(path.clone());
        let mut unit = Unit::new(descriptor, name, path);
        for asset in data {
            let path = join(directory, asset);
            self.consumed.insert(path.clone());
            if exists(self.snapshot, &path) {
                unit.data.push(path);
            } else {
                self.note(&path, "missing inventoried asset");
            }
        }
        if exists(self.snapshot, &unit.path) {
            self.push(unit)
        } else {
            self.note(directory, &format!("no {file}"));
            Ok(())
        }
    }

    /// Validate one matched directory against the descriptor's scope boundaries.
    fn concrete(&mut self, descriptor: &KindDescriptor, directory: &str) -> bool {
        if concrete(descriptor, directory) {
            true
        } else {
            self.note(directory, "nested area placement");
            false
        }
    }

    /// Refuse names and unclaimed nonempty content, including shared support roots.
    fn unclaimed(&mut self) {
        for (directory, entries) in &self.snapshot.directories {
            for entry in entries {
                let path = join(directory, &entry.name);
                self.check_path(&path, entry.kind);
            }
        }
    }

    /// Refuse product/native names and content without an exact inventory placement.
    fn check_path(&mut self, path: &str, kind: EntryKind) {
        if !naming::functional(path, kind) {
            self.note(
                path,
                "must use a functional name, not a registered product or misplaced native filename",
            );
        }
        if kind != EntryKind::Directory && !self.consumed.contains(path) {
            self.note(
                path,
                "not a registered v4 placement; nested/unknown areas and unregistered trees refuse",
            );
        }
    }

    /// Add a located refusal without parsing any content.
    fn note(&mut self, path: &str, message: &str) {
        self.found
            .diagnostics
            .push(Diagnostic::new(path, "", message));
    }

    /// Admit a functional local name under the aggregate resource bound.
    fn push(&mut self, unit: Unit) -> Result<(), Refusal> {
        if !is_name(&unit.name) || naming::product(&unit.name) {
            self.note(
                &unit.path,
                "must use a lower-case hyphenated functional name",
            );
            return Ok(());
        }
        if self.found.units.len() >= self.limits.catalog_resources {
            return Err(refusal(format!(
                "more than {} resources",
                self.limits.catalog_resources
            )));
        }
        self.found.units.push(unit);
        Ok(())
    }
}

/// A regular file at the exact path, never a directory/link substitute.
fn exists(snapshot: &Snapshot, path: &str) -> bool {
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    snapshot.directories.get(parent).is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.name == name && entry.kind == EntryKind::File)
    })
}
