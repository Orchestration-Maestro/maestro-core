//! The registry: each tool once, with what it takes, does and needs.

use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    error, fmt,
};

/// What a tool may do beyond computing its answer. The set is closed: a new
/// effect is a change to this type, which a review sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// Reads what the kernel or the file system holds.
    Read,
    /// Changes what the kernel or the file system holds.
    Write,
    /// Opens a network connection, to a service on the same machine included.
    Network,
    /// Runs a model, through the model gateway.
    Model,
}

/// A tool as it is registered.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Its name, unique in the registry.
    pub name: String,
    /// The JSON Schema its input follows.
    pub schema: Value,
    /// What it may do.
    pub effects: BTreeSet<Effect>,
    /// The scope paths (D4) a caller needs, such as
    /// `workspace/default/collection/ctm`: text until scopes arrive (T011).
    pub scopes: BTreeSet<String>,
}

/// The tools Maestro offers, each registered once.
#[derive(Debug, Default)]
pub struct Registry {
    /// The tools, by name.
    tools: BTreeMap<String, Tool>,
}

impl Registry {
    /// Registers `tool`.
    ///
    /// # Errors
    ///
    /// [`DuplicateTool`] when a tool of the same name is registered already;
    /// that tool stays as it was.
    pub fn register(&mut self, tool: Tool) -> Result<(), DuplicateTool> {
        match self.tools.entry(tool.name.clone()) {
            Entry::Occupied(_) => Err(DuplicateTool(tool.name)),
            Entry::Vacant(slot) => {
                slot.insert(tool);
                Ok(())
            }
        }
    }

    /// The registered tools, in name order.
    pub fn tools(&self) -> impl Iterator<Item = &Tool> {
        self.tools.values()
    }
}

/// A tool registered under a name already taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateTool(String);

impl DuplicateTool {
    /// The name already taken.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DuplicateTool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a tool named {:?} is already registered", self.0)
    }
}

impl error::Error for DuplicateTool {}
