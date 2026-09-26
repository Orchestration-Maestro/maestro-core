//! Events as they leave the kernel: the `CloudEvents` 1.0 envelope of
//! docs/architecture/07 §3.1 around an event the journal recorded, which
//! serializes as `CloudEvents`' JSON format.

use super::{event::Event, knowledge::PUBLIC_EVENTS};
use serde::Serialize;
use serde_json::Value;
use std::{error, fmt};

/// The `CloudEvents` version every envelope follows.
const SPECVERSION: &str = "1.0";

/// The media type of every event's data.
const DATACONTENTTYPE: &str = "application/json";

/// What the `dataschema` of a public event starts with: the name and major
/// version of its schema follow.
const SCHEMAS: &str = "maestro://schemas/events/";

/// The machine a kernel runs on, which the `source` of its events names:
/// `maestro://<machine>/kernel`. Its caller names it: the kernel looks up no
/// host name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine(String);

impl Machine {
    /// The machine `name` names: letters, digits, `-`, `.`, `_` and `~`,
    /// the characters a URI never escapes (RFC 3986 §2.3), so that the name
    /// is the host of the `source` URI as it stands.
    ///
    /// # Errors
    ///
    /// [`InvalidMachine`] when `name` is empty or holds any other character.
    pub fn parse(name: &str) -> Result<Self, InvalidMachine> {
        if !name.is_empty() && name.chars().all(unreserved) {
            Ok(Self(name.to_owned()))
        } else {
            Err(InvalidMachine(name.to_owned()))
        }
    }
}

/// Whether a URI never escapes `character` (RFC 3986 §2.3).
fn unreserved(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~')
}

/// A name that cannot name a machine, given whole: it is empty, or holds a
/// character the host of a URI would have to escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidMachine(pub String);

impl fmt::Display for InvalidMachine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} cannot name the machine of an event's source: use letters, digits, '-', \
             '.', '_' and '~' only",
            self.0
        )
    }
}

impl error::Error for InvalidMachine {}

/// An event as it leaves the kernel: the `CloudEvents` 1.0 envelope of an
/// event the journal recorded, its attributes named and written as
/// `CloudEvents`' JSON format has them. It gains attributes without a new
/// version, so it is non-exhaustive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Envelope {
    /// The version of `CloudEvents` it follows: `1.0`.
    pub specversion: &'static str,
    /// The event's ID, its ULID, which a consumer deduplicates by.
    pub id: String,
    /// Where it comes from: `maestro://<machine>/kernel`.
    pub source: String,
    /// What happened, such as `maestro.knowledge.generation.published.v1`.
    pub r#type: String,
    /// What it happened to, such as `collection/<id>`.
    pub subject: String,
    /// When the journal recorded it: RFC 3339 in UTC, to the millisecond.
    pub time: String,
    /// The media type of its data: `application/json`.
    pub datacontenttype: &'static str,
    /// The schema its data follows, when it is a public event of the
    /// catalogue: `maestro://schemas/events/<name>/<major>`, committed as
    /// `schemas/events/<name>/<major>.json`. Any other event has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataschema: Option<String>,
    /// An extension: the path of the scope it belongs to.
    pub maestroscope: String,
    /// An extension: the stream it belongs to, which `maestrosequence`
    /// counts in and a consumer acknowledges it on, such as `collection/<id>`
    /// for the knowledge events.
    pub maestrostream: String,
    /// An extension: its place in its stream, 1 for the first event.
    /// `CloudEvents`' integers stop at 2,147,483,647, which a stream passes
    /// only after as many events.
    pub maestrosequence: u64,
    /// An extension: the W3C trace context of the work that recorded it.
    /// None yet: the kernel carries no trace context in S1, so the envelope
    /// leaves it out, and an event gains it later without a new version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
    /// What it carries.
    pub data: Value,
}

impl Envelope {
    /// The envelope of `event`, recorded by the kernel of `machine`.
    #[must_use]
    pub fn new(event: Event, machine: &Machine) -> Self {
        let dataschema = PUBLIC_EVENTS
            .iter()
            .find(|public| public.r#type == event.r#type)
            .map(|public| format!("{SCHEMAS}{}", public.schema));
        Self {
            specversion: SPECVERSION,
            id: event.id.to_string(),
            source: format!("maestro://{}/kernel", machine.0),
            r#type: event.r#type,
            subject: event.subject,
            time: event.time,
            datacontenttype: DATACONTENTTYPE,
            dataschema,
            maestroscope: event.scope,
            maestrostream: event.stream,
            maestrosequence: event.sequence,
            traceparent: None,
            data: event.data,
        }
    }
}
