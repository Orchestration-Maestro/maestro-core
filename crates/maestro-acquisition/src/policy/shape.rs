//! Objects, scalar names and bounded strings, never positional arrays or scripts.
use maestro_kernel::scope::Scope;
use reqwest::Url;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer,
    de::{self},
};
use std::{borrow::Cow, marker::PhantomData};

pub(super) use maestro_knowledge::strict_json::{
    name, names, nullable, nullable_object, object, objects,
};

/// Logical names are not filesystem paths.
pub(super) fn valid_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}
/// Decode one bounded ID.
pub(super) fn id<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_id)
}
/// Decode bounded non-executable text.
pub(super) fn text<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_text)
}
/// Text never contains NUL.
fn valid_text(text: &str) -> bool {
    text.len() <= 4096 && !text.contains('\0')
}
/// One canonical DNS host, never an IP, userinfo or ambiguous host.
pub(super) fn host<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_host)
}
/// DNS labels are lowercase and never empty, numeric-only or escaped.
fn valid_host(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        && Url::parse(&format!("https://{host}/")).is_ok_and(|url| url.domain() == Some(host))
}
/// Parse HTTPS without credentials, encoded separators or encoded unreserved bytes.
/// Path escapes use uppercase hex; query and fragment bytes remain unchanged.
pub(super) fn checked_url(text: &str) -> Option<Url> {
    if text.len() > 8192
        || !safe_encoding(text)
        || text
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b'\\' || byte == b' ')
    {
        return None;
    }
    let mut url = Url::parse(text).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !valid_host(url.host_str()?)
        || !url.has_host()
    {
        return None;
    }
    let authority = text
        .strip_prefix("https://")?
        .split('/')
        .next()?
        .split(['?', '#'])
        .next()?;
    let canonical = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str()?),
        None => url.host_str()?.to_owned(),
    };
    // Explicit :443 is allowed; every other host spelling must be canonical.
    if authority != canonical && authority != format!("{}:443", url.host_str()?) {
        return None;
    }
    let remainder = text.strip_prefix("https://")?.strip_prefix(authority)?;
    let supplied_path = remainder.split(['?', '#']).next()?;
    let supplied_path = if supplied_path.is_empty() {
        "/"
    } else {
        supplied_path
    };
    if !valid_path(supplied_path) || supplied_path != url.path() {
        return None;
    }
    let supplied_query = remainder
        .split('#')
        .next()?
        .split_once('?')
        .map(|(_, query)| query);
    if supplied_query != url.query() {
        return None;
    }
    url.set_path(&canonical_path(supplied_path));
    Some(url)
}
/// Percent escapes must be complete UTF-8 without raw or escaped controls.
fn safe_encoding(text: &str) -> bool {
    let mut decoded = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let Some(high) = bytes.next().and_then(hex_digit) else {
                return false;
            };
            let Some(low) = bytes.next().and_then(hex_digit) else {
                return false;
            };
            decoded.push(high * 16 + low);
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded).is_ok_and(|text| !text.chars().any(char::is_control))
}
/// One hexadecimal nibble, whose value always fits arithmetic in a byte.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte.to_ascii_lowercase() {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte.to_ascii_lowercase() - b'a' + 10),
        _ => None,
    }
}
/// One absolute safe URL string.
pub(super) fn url<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, |text| checked_url(text).is_some())
}
/// Relative admitted endpoint or origin path prefix.
pub(super) fn path<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_path).map(|path| canonical_path(&path))
}
/// One unambiguous path, checked before a later admission task matches boundaries.
pub(super) fn valid_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text.starts_with('/')
        && !text.contains("//")
        && safe_encoding(text)
        && valid_text(text)
        && !text.contains(['\\', '?', '#'])
        && !text.bytes().any(|byte| byte.is_ascii_control())
        && !text
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        && !text.as_bytes().windows(3).any(encoded_unreserved)
        && !["%2f", "%5c", "%25", "%00"]
            .iter()
            .any(|encoded| lower.contains(encoded))
}
/// RFC 3986 unreserved bytes must appear literally, never percent-encoded.
fn encoded_unreserved(bytes: &[u8]) -> bool {
    let [b'%', high, low] = bytes else {
        return false;
    };
    let Some((high, low)) = hex_digit(*high).zip(hex_digit(*low)) else {
        return false;
    };
    let byte = high * 16 + low;
    byte.is_ascii_alphanumeric() || b"-._~".contains(&byte)
}
/// Validated paths store uppercase escape hex, preserving all other characters.
fn canonical_path(text: &str) -> String {
    let mut path = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        path.push(character);
        if character == '%' {
            for hex in chars.by_ref().take(2) {
                path.push(hex.to_ascii_uppercase());
            }
        }
    }
    path
}
/// UTC RFC3339 time; comparisons use fixed UTC spellings without offsets.
pub(super) fn time<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_time)
}
/// Validate date, seconds and an optional fractional-second part without a new dependency.
pub(super) fn valid_time(text: &str) -> bool {
    let Some((date, clock)) = text.split_once('T') else {
        return false;
    };
    let Some(clock) = clock.strip_suffix('Z') else {
        return false;
    };
    let date: Vec<_> = date.split('-').collect();
    let clock: Vec<_> = clock.split(':').collect();
    let ([year, month, day], [hour, minute, second]) = (date.as_slice(), clock.as_slice()) else {
        return false;
    };
    let Some(year) = digits(year, 4) else {
        return false;
    };
    let Some(month) = digits(month, 2) else {
        return false;
    };
    let Some(day) = digits(day, 2) else {
        return false;
    };
    let seconds = second.split_once('.');
    let second = if let Some((second, fraction)) = seconds {
        if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        second
    } else {
        second
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    day > 0
        && day <= days
        && digits(hour, 2).is_some_and(|number| number < 24)
        && digits(minute, 2).is_some_and(|number| number < 60)
        && digits(second, 2).is_some_and(|number| number < 60)
}
/// Fixed-width ASCII decimal field.
fn digits(text: &str, width: usize) -> Option<u32> {
    if text.len() == width && text.bytes().all(|byte| byte.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}
/// Decode one primitive and apply its format bound without source diagnostics.
fn checked<'de, D: Deserializer<'de>>(
    decoder: D,
    valid: impl FnOnce(&str) -> bool,
) -> Result<String, D::Error> {
    decode_checked(decoder, |text: &String| valid(text))
}
/// Validate each list element by its primitive constraint.
fn list<'de, D: Deserializer<'de>>(
    decoder: D,
    valid: impl Fn(&str) -> bool,
) -> Result<Vec<String>, D::Error> {
    decode_checked(decoder, |values: &Vec<String>| {
        values.iter().all(|value| valid(value))
    })
}
/// One decode/constraint path for required, list and nullable primitives.
fn decode_checked<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
    valid: impl FnOnce(&T) -> bool,
) -> Result<T, D::Error> {
    let value = T::deserialize(decoder)?;
    if valid(&value) {
        Ok(value)
    } else {
        Err(de::Error::custom("invalid bounded field"))
    }
}
/// A list of logical names.
pub(super) fn ids<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    list(decoder, valid_id)
}
/// A list of bounded texts.
pub(super) fn texts<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    list(decoder, valid_text)
}
/// A list of paths.
pub(super) fn paths<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    list(decoder, valid_path).map(|paths| paths.iter().map(|path| canonical_path(path)).collect())
}
/// A list of URLs.
pub(super) fn urls<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    list(decoder, |text| checked_url(text).is_some())
}
/// Scope tags reuse the kernel path grammar.
pub(super) fn scopes<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    list(decoder, |text| text.parse::<Scope>().is_ok())
}
/// A required nullable ID.
pub(super) fn nullable_id<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<Option<String>, D::Error> {
    optional(decoder, valid_id)
}
/// A required nullable path.
pub(super) fn nullable_path<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<Option<String>, D::Error> {
    optional(decoder, valid_path).map(|path| path.map(|path| canonical_path(&path)))
}
/// A required nullable time.
pub(super) fn nullable_time<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<Option<String>, D::Error> {
    optional(decoder, valid_time)
}
/// Apply the primitive constraint only to a present non-null scalar.
fn optional<'de, D: Deserializer<'de>>(
    decoder: D,
    valid: impl FnOnce(&str) -> bool,
) -> Result<Option<String>, D::Error> {
    decode_checked(decoder, |value: &Option<String>| {
        value.as_deref().is_none_or(valid)
    })
}

/// Schema-only wrapper: null is allowed, but omission is not.
#[derive(Debug)]
pub(super) struct RequiredNullable<T>(PhantomData<T>);
impl<T: JsonSchema> JsonSchema for RequiredNullable<T> {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> Cow<'static, str> {
        format!("RequiredNullable_{}", T::schema_name()).into()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Option::<T>::json_schema(generator)
    }
}
