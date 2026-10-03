//! Entry-level ownership over a strictly decoded shared JSON snapshot.
use crate::source::json;
use serde_json::{Value, json as value};
use std::io;

/// Edit only mcpServers.maestro; byte-identical replay never rewrites user formatting.
pub(super) fn edit(
    bytes: Option<&[u8]>,
    entry: &Value,
    owned: bool,
    remove: bool,
) -> io::Result<Vec<u8>> {
    let mut document = match bytes {
        Some(bytes) => json::parse(bytes).map_err(io::Error::other)?,
        None => value!({}),
    };
    let object = document
        .as_object_mut()
        .ok_or_else(|| io::Error::other(".mcp.json must be an object"))?;
    if !object.contains_key("mcpServers") {
        if owned {
            return Err(io::Error::other(
                "owned .mcp.json/mcpServers/maestro entry changed",
            ));
        }
        if !remove {
            object.insert("mcpServers".to_owned(), value!({}));
        }
    }
    if let Some(servers) = object.get_mut("mcpServers") {
        let servers = servers
            .as_object_mut()
            .ok_or_else(|| io::Error::other(".mcp.json/mcpServers must be an object"))?;
        match (owned, servers.get("maestro")) {
            (true, Some(current)) if current == entry => {
                if remove {
                    servers.remove("maestro");
                } else {
                    return Ok(bytes.unwrap_or_default().to_vec());
                }
            }
            (true, _) => {
                return Err(io::Error::other(
                    "owned .mcp.json/mcpServers/maestro entry changed; \
                     restore the owned entry before retrying",
                ));
            }
            (false, Some(_)) if !remove => {
                return Err(io::Error::other(
                    ".mcp.json/mcpServers/maestro is unowned; \
                     choose another target or move the user entry",
                ));
            }
            (false, _) => {
                if remove {
                    return Ok(bytes.unwrap_or(b"{}").to_vec());
                }
                servers.insert("maestro".to_owned(), entry.clone());
            }
        }
    } else if remove {
        return Ok(bytes.unwrap_or(b"{}").to_vec());
    }
    serde_json::to_vec_pretty(&document).map_err(io::Error::other)
}
