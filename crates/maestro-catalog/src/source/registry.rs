//! The kinds a checker knows: each a validated [`KindDescriptor`] and, when
//! it names one, a hook from the registry's fixed table of named hooks.

use super::{
    descriptor::{FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    parse::is_name,
    rules::KindRules,
    types::Maturity,
};

/// Top-level entries that are not resources: never read, never run.
pub(super) const NOT_RESOURCES: [&str; 5] =
    ["bootstrap", "docs", "README.md", "CODEOWNERS", "LICENSE"];

/// The hooks a descriptor may select, each by its name. Content never
/// supplies code: a hook is reviewed code in the built-in table.
pub(super) type Hooks = &'static [(&'static str, &'static dyn KindRules)];

/// One registered kind.
#[derive(Debug, Clone)]
pub struct Registration {
    /// What the kind is, as data.
    pub descriptor: KindDescriptor,
    /// The hook its descriptor selects, if any.
    pub(super) rules: Option<&'static dyn KindRules>,
}

/// The kinds a checker knows, in registration order, and the hooks their
/// descriptors may select; the default registry has neither.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// The hooks, by name.
    hooks: Hooks,
    /// The registered kinds.
    kinds: Vec<Registration>,
}

impl Registry {
    /// An empty registry whose descriptors may select `hooks`.
    pub(super) fn with_hooks(hooks: Hooks) -> Self {
        Self {
            hooks,
            kinds: Vec::new(),
        }
    }

    /// Registers the kind `descriptor` describes, with the hook it selects.
    ///
    /// # Errors
    ///
    /// Why it cannot be registered: its kind or directory is already
    /// registered, its descriptor contradicts itself, or it selects an
    /// unknown hook.
    pub fn register(&mut self, descriptor: KindDescriptor) -> Result<(), String> {
        if self.kind(&descriptor.kind).is_some() {
            return Err(format!("kind {:?} is already registered", descriptor.kind));
        }
        if self.directory(&descriptor.directory).is_some() {
            return Err(format!(
                "directory {:?} already has a kind",
                descriptor.directory
            ));
        }
        if let Some(problem) = shape_problem(&descriptor) {
            return Err(problem);
        }
        let rules = hook(&descriptor, self.hooks)?;
        self.kinds.push(Registration { descriptor, rules });
        Ok(())
    }

    /// The registered kinds, in registration order.
    pub fn registrations(&self) -> impl Iterator<Item = &Registration> {
        self.kinds.iter()
    }

    /// The kind named `kind`.
    #[must_use]
    pub fn kind(&self, kind: &str) -> Option<&Registration> {
        self.kinds
            .iter()
            .find(|registration| registration.descriptor.kind == kind)
    }

    /// The kind whose directory is `directory`.
    #[must_use]
    pub fn directory(&self, directory: &str) -> Option<&Registration> {
        self.kinds
            .iter()
            .find(|registration| registration.descriptor.directory == directory)
    }
}

/// The hook `descriptor` selects from `hooks`, if any.
fn hook(
    descriptor: &KindDescriptor,
    hooks: Hooks,
) -> Result<Option<&'static dyn KindRules>, String> {
    let Some(name) = &descriptor.hook else {
        return Ok(None);
    };
    hooks
        .iter()
        .find(|(known, _)| known == name)
        .map(|(_, rules)| Some(*rules))
        .ok_or_else(|| format!("kind {}: unknown hook {name:?}", descriptor.kind))
}

/// Why `descriptor` contradicts itself, if it does.
fn shape_problem(descriptor: &KindDescriptor) -> Option<String> {
    let problem = if !is_name(&descriptor.kind) {
        "its name is not a lower-case hyphenated name".to_owned()
    } else if descriptor.version == 0 {
        "its version must be 1 or more".to_owned()
    } else if !is_name(&descriptor.directory)
        || NOT_RESOURCES.contains(&descriptor.directory.as_str())
    {
        format!(
            "directory {:?} is not one the catalog reads",
            descriptor.directory
        )
    } else if matches!(descriptor.metadata, MetadataPlace::Sidecar { .. })
        && !matches!(descriptor.layout, Layout::Files { .. })
    {
        "a sidecar pairs only with the files layout".to_owned()
    } else if descriptor.body && descriptor.format != Format::Markdown {
        "only a Markdown kind has a body".to_owned()
    } else if descriptor.lifecycle.contains(&Maturity::Qualified) {
        "qualified needs S4 evidence; no S3 kind admits it".to_owned()
    } else {
        return name_field_problem(descriptor);
    };
    Some(format!("kind {}: {problem}", descriptor.kind))
}

/// Why `descriptor`'s name field is not one of its text fields, if it is not.
fn name_field_problem(descriptor: &KindDescriptor) -> Option<String> {
    let name = descriptor.name_field.as_ref()?;
    let text = descriptor
        .fields
        .iter()
        .any(|field| field.key == *name && field.kind == FieldType::Text);
    (!text).then(|| {
        format!(
            "kind {}: name field {name:?} is not one of its text fields",
            descriptor.kind
        )
    })
}
