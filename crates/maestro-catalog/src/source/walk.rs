//! Discovery: the catalog's top level, then each registered kind's
//! directory as its layout describes it. Nothing is read or parsed here, so
//! the resource limit refuses a catalog before any parser allocates.

use super::{
    descriptor::{KindDescriptor, Layout, MetadataPlace},
    parse::is_name,
    registry::{NOT_RESOURCES, Registry},
    tree::{Entry, EntryKind, SourceTree},
    types::{Diagnostic, Refusal},
};
use crate::limits::Limits;
use std::collections::BTreeSet;

/// One resource found: its kind, name and files.
#[derive(Debug)]
pub(super) struct Unit {
    /// Its kind's name.
    pub(super) kind: String,
    /// Its name.
    pub(super) name: String,
    /// Its primary file.
    pub(super) path: String,
    /// Its sidecar, when its kind keeps metadata in one.
    pub(super) sidecar: Option<String>,
    /// The data folders it holds, never read.
    pub(super) data: Vec<String>,
}

impl Unit {
    /// The resource `name` of `descriptor` whose primary file is `path`.
    fn new(descriptor: &KindDescriptor, name: &str, path: String) -> Self {
        Self {
            kind: descriptor.kind.clone(),
            name: name.to_owned(),
            path,
            sidecar: None,
            data: Vec::new(),
        }
    }
}

/// What discovery found.
#[derive(Debug)]
pub(super) struct Found {
    /// The resources.
    pub(super) units: Vec<Unit>,
    /// The problems of the layout.
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// The relative path of `name` in `directory`.
fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

/// A refusal of the whole catalog.
fn refusal(message: String) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new("", "", message)],
    }
}

/// The folder patterns that continue into the folder `name`: those whose
/// next segment is `name`, or `*` when `name` is a valid name.
fn next_patterns<'a>(patterns: &[Vec<&'a str>], name: &str) -> Vec<Vec<&'a str>> {
    patterns
        .iter()
        .filter_map(|pattern| match pattern.split_first() {
            Some((first, rest)) if *first == name || (*first == "*" && is_name(name)) => {
                Some(rest.to_vec())
            }
            _ => None,
        })
        .collect()
}

/// Discovery's state.
struct Walker<'a> {
    /// The catalog.
    tree: &'a dyn SourceTree,
    /// The resource limit.
    limit: usize,
    /// What it found.
    found: Found,
}

/// Finds every resource `tree` holds for the kinds of `registry`.
///
/// # Errors
///
/// A [`Refusal`] when the catalog cannot be listed or holds more resources
/// than `limits` allow.
pub(super) fn walk(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
) -> Result<Found, Refusal> {
    let entries = tree.list("").map_err(|error| Refusal {
        diagnostics: vec![Diagnostic::unreadable(
            "",
            format!("cannot list the catalog directory: {error}"),
        )],
    })?;
    let mut walker = Walker {
        tree,
        limit: limits.catalog_resources,
        found: Found {
            units: Vec::new(),
            diagnostics: Vec::new(),
        },
    };
    for entry in entries {
        if entry.name.starts_with('.') || NOT_RESOURCES.contains(&entry.name.as_str()) {
            continue;
        }
        match (entry.kind, registry.directory(&entry.name)) {
            (EntryKind::Unsupported, _) => walker.link(&entry.name),
            (EntryKind::Directory, Some(registration)) => walker.kind(&registration.descriptor)?,
            (EntryKind::Directory, None) => {
                walker.note(&entry.name, "no kind registered for this directory");
            }
            (EntryKind::File, _) => {
                walker.note(
                    &entry.name,
                    "not a catalog resource or a known non-resource entry",
                );
            }
        }
    }
    for registration in registry.registrations() {
        walker.require(&registration.descriptor);
    }
    Ok(walker.found)
}

impl Walker<'_> {
    /// Notes `message` about `path`.
    fn note(&mut self, path: &str, message: &str) {
        self.found
            .diagnostics
            .push(Diagnostic::new(path, "", message));
    }

    /// Notes a link or special file at `path`.
    fn link(&mut self, path: &str) {
        self.note(path, "links and special files are not read");
    }

    /// Notes an entry at `path` that its directory does not hold.
    fn stray(&mut self, path: &str) {
        self.note(path, "not a resource file of this directory");
    }

    /// The entries of `directory`, or none after noting why.
    fn list(&mut self, directory: &str) -> Vec<Entry> {
        self.tree.list(directory).unwrap_or_else(|error| {
            self.found.diagnostics.push(Diagnostic::unreadable(
                directory,
                format!("cannot list: {error}"),
            ));
            Vec::new()
        })
    }

    /// Records the resource `unit`.
    fn push(&mut self, unit: Unit) -> Result<(), Refusal> {
        if self.found.units.len() >= self.limit {
            return Err(refusal(format!("more than {} resources", self.limit)));
        }
        self.found.units.push(unit);
        Ok(())
    }

    /// Notes a kind the catalog must hold but does not.
    fn require(&mut self, descriptor: &KindDescriptor) {
        let Some(reason) = &descriptor.required else {
            return;
        };
        if self
            .found
            .units
            .iter()
            .any(|unit| unit.kind == descriptor.kind)
        {
            return;
        }
        let path = match &descriptor.layout {
            Layout::Single { file, .. } => join(&descriptor.directory, file),
            _ => descriptor.directory.clone(),
        };
        self.note(&path, &format!("missing: {reason}"));
    }

    /// Finds the resources of `descriptor` in its directory.
    fn kind(&mut self, descriptor: &KindDescriptor) -> Result<(), Refusal> {
        match &descriptor.layout {
            Layout::Files { folders, .. } => {
                let patterns: Vec<Vec<&str>> = folders
                    .iter()
                    .map(|folder| folder.split('/').filter(|part| !part.is_empty()).collect())
                    .collect();
                self.files(descriptor, &descriptor.directory, &patterns)
            }
            Layout::Folder { file, data } => self.folders(descriptor, file, data),
            Layout::Single { file, name } => self.single(descriptor, file, name),
        }
    }

    /// Finds `Files` resources in `directory`, whose remaining folder
    /// patterns are `patterns`.
    fn files(
        &mut self,
        descriptor: &KindDescriptor,
        directory: &str,
        patterns: &[Vec<&str>],
    ) -> Result<(), Refusal> {
        let here = patterns.iter().any(Vec::is_empty);
        let mut files = Vec::new();
        for entry in self.list(directory) {
            let path = join(directory, &entry.name);
            match entry.kind {
                EntryKind::Unsupported => self.link(&path),
                EntryKind::File if here => files.push(entry.name),
                EntryKind::File => self.stray(&path),
                EntryKind::Directory => self.descend(descriptor, &path, patterns, &entry.name)?,
            }
        }
        self.pair(descriptor, directory, &files)
    }

    /// Continues `Files` discovery into the folder `name` at `path`, when a
    /// pattern continues there.
    fn descend(
        &mut self,
        descriptor: &KindDescriptor,
        path: &str,
        patterns: &[Vec<&str>],
        name: &str,
    ) -> Result<(), Refusal> {
        let next = next_patterns(patterns, name);
        if next.is_empty() {
            self.stray(path);
            Ok(())
        } else {
            self.files(descriptor, path, &next)
        }
    }

    /// Pairs the `files` of `directory` into resources, each primary file
    /// with its sidecar when the kind keeps one.
    fn pair(
        &mut self,
        descriptor: &KindDescriptor,
        directory: &str,
        files: &[String],
    ) -> Result<(), Refusal> {
        let Layout::Files { suffix, .. } = &descriptor.layout else {
            return Ok(());
        };
        let sidecar = match &descriptor.metadata {
            MetadataPlace::Sidecar { suffix } => Some(suffix.as_str()),
            _ => None,
        };
        let mut primaries = BTreeSet::new();
        let mut sidecars = BTreeSet::new();
        for file in files {
            if let Some(stem) = sidecar.and_then(|sidecar| file.strip_suffix(sidecar)) {
                sidecars.insert(stem);
            } else if let Some(stem) = file.strip_suffix(suffix.as_str()) {
                primaries.insert(stem);
            } else {
                self.stray(&join(directory, file));
            }
        }
        for stem in &primaries {
            let path = join(directory, &format!("{stem}{suffix}"));
            if !is_name(stem) {
                self.note(
                    &path,
                    &format!("{stem:?} is not a lower-case hyphenated name"),
                );
                continue;
            }
            match sidecar {
                Some(sidecar) if !sidecars.contains(stem) => {
                    self.note(&path, &format!("no {stem}{sidecar} sidecar beside it"));
                }
                Some(sidecar) => {
                    let beside = join(directory, &format!("{stem}{sidecar}"));
                    self.push(Unit {
                        sidecar: Some(beside),
                        ..Unit::new(descriptor, stem, path)
                    })?;
                }
                None => self.push(Unit::new(descriptor, stem, path))?,
            }
        }
        for stem in sidecars.difference(&primaries) {
            let path = join(directory, &format!("{stem}{}", sidecar.unwrap_or_default()));
            self.note(&path, &format!("no {stem}{suffix} beside it"));
        }
        Ok(())
    }

    /// Finds `Folder` resources: one folder per resource holding `file`,
    /// with `data` subfolders kept as data.
    fn folders(
        &mut self,
        descriptor: &KindDescriptor,
        file: &str,
        data: &[String],
    ) -> Result<(), Refusal> {
        for entry in self.list(&descriptor.directory) {
            let folder = join(&descriptor.directory, &entry.name);
            match entry.kind {
                EntryKind::Unsupported => self.link(&folder),
                EntryKind::File => self.stray(&folder),
                EntryKind::Directory if !is_name(&entry.name) => {
                    self.note(
                        &folder,
                        &format!("{:?} is not a lower-case hyphenated name", entry.name),
                    );
                }
                EntryKind::Directory => {
                    self.folder(descriptor, &folder, &entry.name, (file, data))?;
                }
            }
        }
        Ok(())
    }

    /// Finds the `Folder` resource `name` in `folder`, which must hold
    /// `file` and may hold the `data` subfolders.
    fn folder(
        &mut self,
        descriptor: &KindDescriptor,
        folder: &str,
        name: &str,
        (file, data): (&str, &[String]),
    ) -> Result<(), Refusal> {
        let mut primary = false;
        let mut kept = Vec::new();
        for inner in self.list(folder) {
            let path = join(folder, &inner.name);
            match inner.kind {
                EntryKind::File if inner.name == file => primary = true,
                EntryKind::Directory if data.contains(&inner.name) => kept.push(path),
                EntryKind::Unsupported => self.link(&path),
                _ => self.stray(&path),
            }
        }
        if primary {
            self.push(Unit {
                data: kept,
                ..Unit::new(descriptor, name, join(folder, file))
            })
        } else {
            self.note(folder, &format!("no {file}"));
            Ok(())
        }
    }

    /// Finds the `Single` resource `name`, the file `file`.
    fn single(
        &mut self,
        descriptor: &KindDescriptor,
        file: &str,
        name: &str,
    ) -> Result<(), Refusal> {
        for entry in self.list(&descriptor.directory) {
            let path = join(&descriptor.directory, &entry.name);
            match entry.kind {
                EntryKind::File if entry.name == file => {
                    self.push(Unit::new(descriptor, name, path))?;
                }
                EntryKind::Unsupported => self.link(&path),
                _ => self.stray(&path),
            }
        }
        Ok(())
    }
}
