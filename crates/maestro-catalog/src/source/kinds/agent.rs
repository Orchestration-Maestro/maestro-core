//! `agent`: a Copilot custom agent profile and its `<name>.maestro.toml`
//! sidecar (C01: Copilot ignores and warns on agent `metadata:`). Its hook
//! checks the six fixed body sections, its `server/tool` references and its
//! `mcp-servers`, which name approved `mcp` resources: an agent never embeds
//! a server launch, whose reviewed form the projection renders.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    parse::{is_name, is_tool},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, ResourceId, Value},
};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::BTreeMap;

/// The body's level-two sections, in order (architecture 03 §1.2).
const SECTIONS: [&str; 6] = [
    "Purpose",
    "Responsibilities",
    "Inputs",
    "Working sequence",
    "Outputs",
    "Boundaries",
];

/// The agent kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "agent".to_owned(),
        version: 1,
        directory: "agents".to_owned(),
        layout: Layout::Files {
            suffix: ".agent.md".to_owned(),
            folders: vec!["base".to_owned(), "capabilities/*".to_owned()],
        },
        format: Format::Markdown,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: Some("name".to_owned()),
        fields: vec![
            Field::required("name", FieldType::Text),
            Field::required("description", FieldType::Text),
            Field::optional("tools", FieldType::TextList),
            Field::optional("model", FieldType::Text),
            Field::optional("mcp-servers", FieldType::TextList),
        ],
        body: true,
        requires: vec![
            "agent".to_owned(),
            "skill".to_owned(),
            "instructions".to_owned(),
        ],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("agent-profile".to_owned()),
    }
}

/// The agent's rules beyond its descriptor.
#[derive(Debug)]
pub(super) struct AgentRules;

/// The ID of the MCP server `name`.
fn server(name: &str) -> ResourceId {
    ResourceId {
        kind: "mcp".to_owned(),
        name: name.to_owned(),
    }
}

/// The MCP servers the agent names in `mcp-servers`.
fn servers(resource: &Resource) -> Vec<&str> {
    resource
        .fields
        .get("mcp-servers")
        .and_then(Value::texts)
        .unwrap_or_default()
}

/// The agent's tools: each `server/tool` pair, or a built-in name alone.
fn tools(resource: &Resource) -> Vec<(Option<&str>, &str)> {
    resource
        .fields
        .get("tools")
        .and_then(Value::texts)
        .unwrap_or_default()
        .into_iter()
        .map(|tool| match tool.split_once('/') {
            Some((server, name)) => (Some(server), name),
            None => (None, tool),
        })
        .collect()
}

impl KindRules for AgentRules {
    fn check_resource(
        &self,
        resource: &Resource,
        body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        if let Some(body) = body {
            section_problems(body, problems);
        }
        for name in servers(resource).into_iter().filter(|name| !is_name(name)) {
            problems.push((
                "mcp-servers".to_owned(),
                format!("{name:?} is not a lower-case hyphenated name"),
            ));
        }
        for (server, tool) in tools(resource) {
            if !server.is_none_or(is_name) || !is_tool(tool) {
                let written =
                    server.map_or_else(|| tool.to_owned(), |server| format!("{server}/{tool}"));
                problems.push((
                    "tools".to_owned(),
                    format!("{written:?} is not a tool name"),
                ));
            }
        }
    }

    fn edges(&self, resource: &Resource) -> Vec<ResourceId> {
        tools(resource)
            .into_iter()
            .filter_map(|(name, _)| name)
            .chain(servers(resource))
            .map(server)
            .collect()
    }

    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        for id in servers(resource).into_iter().map(server) {
            if !catalog.contains_key(&id) {
                problems.push((
                    "mcp-servers".to_owned(),
                    format!("names {id}, which does not exist"),
                ));
            }
        }
        for (name, tool) in tools(resource) {
            let Some(name) = name else {
                continue;
            };
            let id = server(name);
            let message = match catalog.get(&id) {
                None => format!("names {name}/{tool}, and {id} does not exist"),
                Some(listed)
                    if !listed
                        .fields
                        .get("tools")
                        .and_then(Value::texts)
                        .unwrap_or_default()
                        .contains(&tool) =>
                {
                    format!("names {name}/{tool}, which {id} does not list")
                }
                Some(_) => continue,
            };
            problems.push(("tools".to_owned(), message));
        }
    }
}

/// The level-two sections of a Markdown body, each with whether it holds text.
#[derive(Debug, Default)]
struct Sections {
    /// Each section's title and whether it holds text.
    found: Vec<(String, bool)>,
    /// The title being read, inside a level-two heading.
    heading: Option<String>,
}

impl Sections {
    /// Reads one Markdown event.
    fn read(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => self.heading = Some(String::new()),
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                if let Some(title) = self.heading.take() {
                    self.found.push((title, false));
                }
            }
            Event::Text(text) | Event::Code(text) => self.text(&text),
            _ => {}
        }
    }

    /// Reads text: a heading's title, or a section's content.
    fn text(&mut self, text: &str) {
        if let Some(title) = &mut self.heading {
            title.push_str(text);
        } else if let Some(last) = self.found.last_mut() {
            last.1 |= !text.trim().is_empty();
        }
    }
}

/// Notes a body whose level-two sections are not exactly [`SECTIONS`] in
/// order, each holding text.
fn section_problems(body: &str, problems: &mut Problems) {
    let mut sections = Sections::default();
    for event in Parser::new(body) {
        sections.read(event);
    }
    let titles: Vec<&str> = sections
        .found
        .iter()
        .map(|(title, _)| title.as_str())
        .collect();
    if titles != SECTIONS {
        problems.push((
            "body".to_owned(),
            format!(
                "expected the sections {} in order; found {}",
                SECTIONS.join(", "),
                titles.join(", ")
            ),
        ));
        return;
    }
    for (title, _) in sections.found.iter().filter(|(_, filled)| !filled) {
        problems.push(("body".to_owned(), format!("section {title:?} is empty")));
    }
}
