//! Tests of scopes: their paths and names, what a grant covers, the grants
//! of a principal and their journal, `config.toml`, and the readers that
//! filter by a `ScopeSet`, with the same outcomes on Linux, macOS and
//! Windows.

mod config;
mod grants;
mod inventory;
mod paths;
mod readers;
mod records;
mod support;
