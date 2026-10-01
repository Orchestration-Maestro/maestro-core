//! Bounded response parser, transient metadata and manual redirect spelling.
use super::{
    stream::{Accounting, Failure, TRAILER_MAX_BYTES, read_body},
    wire_quota::{HeaderLimits, Quota},
};
use crate::policy::identity::FetchIdentity;
use hyper::{body::Incoming, client::conn::http1};
use maestro_kernel::acquisition::{RedirectHop, safe_header_names};
use reqwest::header::{CONTENT_LENGTH, HeaderMap, HeaderName, HeaderValue, LOCATION, RETRY_AFTER};
use std::fmt;

/// Authorized transient response, not a capture envelope or publish permission.
pub struct Response {
    /// Complete decoded body; every intermediate stage shares accounting.
    pub body: Vec<u8>,
    /// Encoded DATA only, excluding chunk framing and discarded trailers.
    /// Accounting's wire counter includes every raw post-header read byte.
    pub wire_body: Vec<u8>,
    /// Final HTTP status.
    pub status: u16,
    /// Selected noncredential metadata only.
    pub headers: HeaderMap,
    /// Admitted final fetch identity, protected from diagnostic output.
    pub identity: FetchIdentity,
    /// Redacted redirect response history, never raw Location values.
    pub redirects: Vec<RedirectHop>,
    /// Redirect-only protected target; never part of safe capture metadata.
    pub(super) location: Option<HeaderValue>,
    /// Retry-only protected server floor, never exported as safe metadata.
    pub(super) retry_after: Option<HeaderValue>,
}
impl fmt::Debug for Response {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Response")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
/// Only standard HTTP redirect statuses trigger another admitted request.
pub(super) fn redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Typed status handling and body ownership live outside the driver future.
pub(super) async fn read_response(
    response: hyper::Response<Incoming>,
    identity: FetchIdentity,
    robots: bool,
    accounting: &mut Accounting,
    quota: &Quota,
) -> Result<Response, Failure> {
    accounting.validate()?;
    let status = response.status().as_u16();
    check_status(status)?;
    let location = response.headers().get(LOCATION).cloned();
    let retry_after = response.headers().get(RETRY_AFTER).cloned();
    let mut headers = HeaderMap::new();
    for name in safe_header_names().map_err(|_| Failure::Configuration)? {
        for value in response.headers().get_all(name.as_str()) {
            headers.append(
                HeaderName::from_bytes(name.as_bytes()).map_err(|_| Failure::Configuration)?,
                value.clone(),
            );
        }
    }
    let (body, wire_body) = if robots && (status == 404 || status == 410) {
        (Vec::new(), Vec::new())
    } else {
        let remaining = accounting
            .limits()
            .wire_bytes
            .get()
            .checked_sub(accounting.wire_bytes())
            .ok_or(Failure::EncodedBytes)?;
        let length = response
            .headers()
            .get(CONTENT_LENGTH)
            .map(|length| {
                length
                    .to_str()
                    .map_err(|_| Failure::Content)?
                    .parse::<u64>()
                    .map_err(|_| Failure::Content)
            })
            .transpose()?;
        if length.is_some_and(|length| length > remaining) {
            return Err(Failure::EncodedBytes);
        }
        quota.open(remaining, length)?;
        read_body(response, accounting, quota).await?
    };
    accounting.validate()?;
    Ok(Response {
        body,
        wire_body,
        status,
        headers,
        identity,
        redirects: Vec::new(),
        location,
        retry_after,
    })
}
/// No authentication escalation, challenge solving or partial promotion.
fn check_status(status: u16) -> Result<(), Failure> {
    match status {
        401 => Err(Failure::Authentication),
        403 => Err(Failure::Challenge),
        206 => Err(Failure::Partial),
        _ => Ok(()),
    }
}

/// Resolve relative references without erasing raw segments before N07 checks.
pub(super) fn redirect_url(base: &reqwest::Url, location: &str) -> String {
    if reqwest::Url::parse(location).is_ok() || location.contains("://") {
        return location.to_owned();
    }
    let origin = base.origin().ascii_serialization();
    if location.starts_with("//") {
        return format!("https:{location}");
    }
    if location.starts_with('/') {
        return format!("{origin}{location}");
    }
    if location.starts_with('?') {
        return format!("{origin}{}{location}", base.path());
    }
    if location.starts_with('#') || location.is_empty() {
        return format!("{}{location}", base.as_str());
    }
    let (parent, _) = base.path().rsplit_once('/').unwrap_or(("", ""));
    format!("{origin}{parent}/{location}")
}

/// Fixed parser workspace on the supported 64-bit targets:
/// `32_768 = 8_192` read bytes `+ (100 * 32)` header slots `+ 21_376` copies.
/// Hyper's default 100-slot index is not set again; the regression pins it.
const PARSER_WORKSPACE: u64 = 32_768;
/// Byte and header-count ceilings are derived from the effective memory envelope.
pub(super) fn parser(
    accounting: &mut Accounting,
) -> Result<(http1::Builder, HeaderLimits), Failure> {
    accounting.workspace(PARSER_WORKSPACE)?;
    accounting.reserve(TRAILER_MAX_BYTES)?;
    let memory = accounting
        .limits()
        .memory_bytes
        .min(accounting.limits().decode.memory_bytes)
        .get();
    let header_bytes = memory.min(PARSER_WORKSPACE) / 4;
    let header_count = usize::try_from(header_bytes).map_err(|_| Failure::Memory)?
        / size_of::<(HeaderName, HeaderValue)>();
    let mut builder = http1::Builder::new();
    let read_bytes = usize::try_from(header_bytes).map_err(|_| Failure::Memory)?;
    builder.max_buf_size(read_bytes);
    Ok((
        builder,
        HeaderLimits {
            bytes: read_bytes,
            fields: header_count,
        },
    ))
}
/// Malformed/oversized protocol input is content failure, not a retryable socket.
pub(super) fn protocol_error(error: &hyper::Error) -> Failure {
    if error.is_parse() {
        return Failure::Content;
    }
    if error.is_incomplete_message() {
        return Failure::Partial;
    }
    Failure::Transport
}
