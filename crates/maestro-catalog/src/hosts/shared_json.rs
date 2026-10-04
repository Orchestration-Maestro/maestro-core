//! Strict validation followed by byte-preserving entry-owned JSON splices.
use crate::{
    limits::Limits,
    policy::schema::{bound_json, json_structure},
    source::json,
};
use serde_json::Value;
use std::{io, ops::Range, str};

/// Edit only mcpServers.maestro; unrelated tokens never pass through serialization.
pub(super) fn edit(
    bytes: Option<&[u8]>,
    entry: &Value,
    owned: bool,
    remove: bool,
) -> io::Result<Vec<u8>> {
    let bytes = bytes.unwrap_or(b"{}");
    let text = str::from_utf8(bytes).map_err(io::Error::other)?;
    bound_json(text, &Limits::PRODUCTION).map_err(io::Error::other)?;
    let document = json::parse(bytes).map_err(io::Error::other)?;
    let object = document
        .as_object()
        .ok_or_else(|| io::Error::other(".mcp.json must be an object"))?;
    let servers = object
        .get("mcpServers")
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| io::Error::other(".mcp.json/mcpServers must be an object"))
        })
        .transpose()?;
    let current = servers.and_then(|servers| servers.get("maestro"));
    if owned && current != Some(entry) {
        return Err(io::Error::other(
            "owned .mcp.json/mcpServers/maestro entry changed; \
             restore the owned entry before retrying",
        ));
    }
    if !owned && current.is_some() && !remove {
        return Err(io::Error::other(
            ".mcp.json/mcpServers/maestro is unowned; choose another target or move the user entry",
        ));
    }
    if (owned && !remove) || (!owned && remove) {
        return Ok(bytes.to_vec());
    }
    let root = object_members(text)?;
    let server_span = root
        .iter()
        .find(|(key, _)| key == "mcpServers")
        .map(|(_, range)| range.clone());
    let serialized = serde_json::to_string(entry).map_err(io::Error::other)?;
    if let Some(span) = server_span {
        let colon = json_structure(&text[span.clone()])
            .find(|(_, byte)| *byte == b':')
            .ok_or_else(|| io::Error::other("validated JSON member span missing"))?
            .0;
        let value_start = span.start + colon + 1;
        let value = &text[value_start..span.end];
        let members = object_members(value)?;
        if remove {
            let index = members
                .iter()
                .position(|(key, _)| key == "maestro")
                .ok_or_else(|| io::Error::other("validated JSON member span missing"))?;
            let member = &members
                .get(index)
                .ok_or_else(|| io::Error::other("validated JSON member span missing"))?
                .1;
            let comma_range = if let Some(next) = members.get(index + 1) {
                Some(member.end..next.1.start)
            } else if index > 0 {
                members
                    .get(index - 1)
                    .map(|previous| previous.1.end..member.start)
            } else {
                None
            };
            let comma = comma_range.and_then(|range| {
                json_structure(&value[range.clone()])
                    .find(|(_, byte)| *byte == b',')
                    .map(|(position, _)| value_start + range.start + position)
            });
            return Ok(remove_member(
                bytes,
                value_start + member.start..value_start + member.end,
                comma,
            ));
        }
        let close = value
            .rfind('}')
            .ok_or_else(|| io::Error::other("validated JSON member span missing"))?
            + value_start;
        let prefix = if members.is_empty() { "" } else { "," };
        return Ok(splice(
            bytes,
            close..close,
            format!("{prefix}\"maestro\":{serialized}").as_bytes(),
        ));
    }
    let close = text
        .rfind('}')
        .ok_or_else(|| io::Error::other("validated JSON member span missing"))?;
    let prefix = if root.is_empty() { "" } else { "," };
    Ok(splice(
        bytes,
        close..close,
        format!("{prefix}\"mcpServers\":{{\"maestro\":{serialized}}}").as_bytes(),
    ))
}

/// Locate direct members in an already validated object with the shared bounded scanner.
fn object_members(text: &str) -> io::Result<Vec<(String, Range<usize>)>> {
    if !text.trim_start().starts_with('{') {
        return Err(io::Error::other(
            "JSON member value must start with an object",
        ));
    }
    let mut depth = 0usize;
    let mut start = 0;
    let mut key = None;
    let mut members = Vec::new();
    for (position, byte) in json_structure(text) {
        match byte {
            b'{' | b'[' => {
                if depth == 0 {
                    start = position + 1;
                }
                depth += 1;
            }
            b':' if depth == 1 => {
                key = Some(
                    serde_json::from_str::<String>(text[start..position].trim())
                        .map_err(io::Error::other)?,
                );
            }
            b',' if depth == 1 => {
                if let Some(key) = key.take() {
                    members.push((key, trimmed_range(text, start..position)));
                }
                start = position + 1;
            }
            b'}' | b']' => {
                if depth == 1
                    && let Some(key) = key.take()
                {
                    members.push((key, trimmed_range(text, start..position)));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    Ok(members)
}

/// Preserve surrounding object whitespace, not just decoded neighbour values.
fn trimmed_range(text: &str, range: Range<usize>) -> Range<usize> {
    let member = &text[range.clone()];
    range.start + member.len() - member.trim_start().len()..range.start + member.trim_end().len()
}

/// Replace one byte range without touching anything on either side.
fn splice(bytes: &[u8], range: Range<usize>, inserted: &[u8]) -> Vec<u8> {
    let mut result = bytes.to_vec();
    result.splice(range, inserted.iter().copied());
    result
}

/// Remove the adjacent comma before or after the member, retaining its original offsets.
fn remove_member(bytes: &[u8], member: Range<usize>, comma: Option<usize>) -> Vec<u8> {
    let mut result = bytes.to_vec();
    if let Some(comma) = comma {
        if comma >= member.end {
            result.remove(comma);
            result.drain(member);
        } else {
            result.drain(member);
            result.remove(comma);
        }
    } else {
        result.drain(member);
    }
    result
}
