//! Objects, scalar names and bounded strings, never positional arrays or scripts.
use maestro_kernel::scope::Scope;
use reqwest::Url;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer,
    de::{self},
};
use std::net::IpAddr;
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
        && host.parse::<IpAddr>().is_err()
}
/// Parse an unambiguous HTTPS URL without credentials or encoded separators.
pub(super) fn checked_url(text: &str) -> Option<Url> {
    if text.len() > 8192
        || text
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b'\\' || byte == b' ')
    {
        return None;
    }
    let url = Url::parse(text).ok()?;
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
    if !valid_path(url.path()) {
        return None;
    }
    Some(url)
}
/// One absolute safe URL string.
pub(super) fn url<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, |text| checked_url(text).is_some())
}
/// Relative admitted endpoint or origin path prefix.
pub(super) fn path<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_path)
}
/// One unambiguous path, checked before a later admission task matches boundaries.
pub(super) fn valid_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text.starts_with('/')
        && !text.starts_with("//")
        && valid_text(text)
        && !text.contains(['\\', '?', '#'])
        && !text.bytes().any(|byte| byte.is_ascii_control())
        && !text
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        && !["%2f", "%5c", "%25", "%2e", "%00"]
            .iter()
            .any(|encoded| lower.contains(encoded))
}
/// UTC RFC3339 time; comparisons use fixed UTC spellings without offsets.
pub(super) fn time<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    checked(decoder, valid_time)
}
/// Validate date, seconds and an optional fractional-second part without a new dependency.
fn valid_time(text: &str) -> bool {
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
    list(decoder, valid_path)
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
    optional(decoder, valid_path)
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
