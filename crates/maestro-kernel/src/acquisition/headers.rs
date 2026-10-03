//! Exact data-defined header selection and content-free unsafe value evidence.
use super::{
    envelope::{HeaderReason, SAFE_HEADERS, SafeHeader},
    privacy::ReceiptError,
};
use crate::artifact::Digest;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Serialize;
use std::{collections::BTreeMap, str};

/// Typed header digest preimage; raw remote bytes never enter diagnostics.
#[derive(Serialize)]
struct HeaderIdentity<'a> {
    /// Exact lowercase field name.
    name: &'a str,
    /// All received bytes, including values not valid UTF-8.
    values: Vec<&'a [u8]>,
}
/// Read the exact pinned data policy.
/// # Errors
/// A corrupt policy fails closed, never broadens selection.
pub fn safe_header_names() -> Result<Vec<String>, ReceiptError> {
    serde_json::from_str(SAFE_HEADERS).map_err(Into::into)
}
/// Retain only allowed fields, hashing unsafe or multivalued input.
/// # Errors
/// A corrupt policy or failed typed serialization refuses.
pub fn safe_headers(headers: &HeaderMap) -> Result<BTreeMap<String, SafeHeader>, ReceiptError> {
    let mut result = BTreeMap::new();
    for name in safe_header_names()? {
        let values: Vec<_> = headers
            .get_all(name.as_str())
            .iter()
            .map(HeaderValue::as_bytes)
            .collect();
        if values.is_empty() {
            continue;
        }
        let value = if let [bytes] = values.as_slice() {
            str::from_utf8(bytes)
                .ok()
                .and_then(|value| canonical(&name, value))
        } else {
            None
        };
        let header = match value {
            Some(value) => SafeHeader::Value { value },
            None => SafeHeader::Hashed {
                digest: Digest::of(&serde_json::to_vec(&HeaderIdentity {
                    name: &name,
                    values,
                })?),
                reason: HeaderReason::UntrustedValue,
            },
        };
        result.insert(name, header);
    }
    Ok(result)
}
/// Credential-safe media spelling; parameters and unsupported kinds never persist.
/// # Errors
/// Unsupported media yields none rather than retaining untrusted text.
#[must_use]
pub fn safe_media(value: &str) -> Option<String> {
    let base = value.split(';').next()?.trim().to_ascii_lowercase();
    if [
        "text/html",
        "text/plain",
        "text/markdown",
        "application/json",
        "application/pdf",
        "application/xml",
        "text/xml",
        "application/xhtml+xml",
        "application/octet-stream",
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
    ]
    .contains(&base.as_str())
    {
        Some(base)
    } else {
        None
    }
}
/// Lengths are canonical integers; codecs are fixed names; validators are hashed.
fn canonical(name: &str, value: &str) -> Option<String> {
    match name {
        "content-type" => safe_media(value).filter(|base| base == value),
        "content-length" => value.parse::<u64>().ok().map(|value| value.to_string()),
        "content-encoding" => {
            let names: Vec<_> = value
                .split(',')
                .map(|name| name.trim().to_ascii_lowercase())
                .collect();
            names
                .iter()
                .all(|name| ["identity", "gzip", "deflate"].contains(&name.as_str()))
                .then(|| names.join(", "))
        }
        _ => None,
    }
}
/// Validate caller envelopes against the exact policy at the storage boundary.
pub(super) fn validate(headers: &BTreeMap<String, SafeHeader>) -> Result<(), ReceiptError> {
    let allowed = safe_header_names()?;
    for (name, header) in headers {
        if !allowed.contains(name) {
            return Err(ReceiptError::Invalid);
        }
        if let SafeHeader::Value { value } = header
            && canonical(name, value).as_ref() != Some(value)
        {
            return Err(ReceiptError::Invalid);
        }
    }
    Ok(())
}
