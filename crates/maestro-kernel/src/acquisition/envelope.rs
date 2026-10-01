//! One immutable capture contract shared by admitted transports and the kernel.
use super::privacy::{Handle, ReceiptError};
use crate::artifact::Digest;
use reqwest::Url;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;

/// Exact safe-header policy, versioned as data rather than caller code.
pub const SAFE_HEADERS: &str = include_str!("safe-headers.json");
/// Provenance identity; query credentials are replaced by a typed digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SafeIdentity {
    /// Query-, fragment- and credential-free HTTPS spelling.
    url: String,
    /// Digest of the removed query, preserving identity without its secrets.
    query_digest: Option<Digest>,
}
/// Typed query digest preimage, immune to JSON map feature unification.
#[derive(Serialize)]
struct QueryIdentity<'a> {
    /// Exact raw query spelling.
    query: &'a str,
}
impl SafeIdentity {
    /// Redact a transient HTTPS URL without persisting credentials.
    /// # Errors
    /// Invalid or non-HTTPS identities refuse with no input in the error.
    pub fn new(text: &str) -> Result<Self, ReceiptError> {
        let mut url = Url::parse(text).map_err(|_| ReceiptError::Invalid)?;
        if url.scheme() != "https" || url.host_str().is_none() {
            return Err(ReceiptError::Invalid);
        }
        let query_digest = url
            .query()
            .map(|query| {
                serde_json::to_vec(&QueryIdentity { query }).map(|bytes| Digest::of(&bytes))
            })
            .transpose()?;
        url.set_query(None);
        url.set_fragment(None);
        url.set_username("").map_err(|()| ReceiptError::Invalid)?;
        url.set_password(None).map_err(|()| ReceiptError::Invalid)?;
        Ok(Self {
            url: url.into(),
            query_digest,
        })
    }
    /// Retained heap bytes, including capacity left by removed URL credentials.
    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        self.url.capacity() as u64
            + self
                .query_digest
                .as_ref()
                .map_or(0, |digest| digest.as_str().len() as u64)
    }
}
impl<'de> Deserialize<'de> for SafeIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Validate persisted redacted spelling, never trust a wire struct literal.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            url: String,
            #[serde(deserialize_with = "Option::deserialize")]
            query_digest: Option<Digest>,
        }
        let wire = Wire::deserialize(deserializer)
            .map_err(|_| de::Error::custom("invalid capture identity"))?;
        let safe = Self::new(&wire.url).map_err(de::Error::custom)?;
        if safe.url != wire.url || safe.query_digest.is_some() {
            return Err(de::Error::custom("unredacted capture identity"));
        }
        Ok(Self {
            url: safe.url,
            query_digest: wire.query_digest,
        })
    }
}
/// Exact byte provenance, never interchangeable labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    /// N09's decoded HTTP payload, not its transfer encoding or DOM.
    WireBody,
    /// Browser-rendered DOM after bounded readiness.
    RenderedDom,
    /// An explicit selection from a parent capture.
    SelectedHtml,
    /// A derived API record with a parent capture.
    ApiRecord,
}
/// The closed set of Transport values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "Transport")]
pub enum Transport {
    /// `http`.
    #[serde(rename = "http")]
    Http,
    /// `browser_request`.
    #[serde(rename = "browser_request")]
    BrowserRequest,
    /// `browser_render`.
    #[serde(rename = "browser_render")]
    BrowserRender,
}

/// Why raw header bytes were deliberately omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderReason {
    /// A remotely supplied validator/value is not credential-safe evidence.
    UntrustedValue,
}
/// Safe value or content-free hash, with a recorded omission reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SafeHeader {
    /// Constrained credential-free value.
    Value {
        /// Canonical safe spelling.
        value: String,
    },
    /// Unsafe value is never persisted verbatim.
    Hashed {
        /// Digest of the typed header preimage.
        digest: Digest,
        /// Recorded omission reason.
        reason: HeaderReason,
    },
}
/// A redacted redirect response, in traversal order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedirectHop {
    /// Admitted hop identity with credentials removed.
    pub identity: SafeIdentity,
    /// Exact redirect status.
    pub status: u16,
}
/// The kernel-owned `maestro-capture/1` payload. The scoped linkage handle is
/// its capture ID; body bytes and JSON both remain immutable artifact records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureEnvelope {
    /// Exact wire schema.
    pub schema: String,
    /// Source namespace bound to writer ownership.
    pub source: String,
    /// Durable frontier identity.
    pub item: Handle,
    /// Logical run retained through replay.
    pub run: Handle,
    /// Sanitized requested identity.
    pub requested: SafeIdentity,
    /// Sanitized final identity.
    pub final_identity: SafeIdentity,
    /// Ordered redacted hop records.
    pub redirects: Vec<RedirectHop>,
    /// Final protocol status.
    pub status: u16,
    /// Only the exact data-defined safe allow-list.
    pub headers: BTreeMap<String, SafeHeader>,
    /// Safe declared media; unknown media stays in hashed header evidence.
    #[serde(deserialize_with = "Option::deserialize")]
    pub declared_media: Option<String>,
    /// Separately detected media, never inferred from the label.
    #[serde(deserialize_with = "Option::deserialize")]
    pub detected_media: Option<String>,
    /// Verified immutable payload digest.
    pub artifact: Digest,
    /// Artifact bytes, distinct from received content-length.
    pub length: u64,
    /// Trusted acquisition time in Unix milliseconds.
    pub observed_ms: u64,
    /// Actual mechanism.
    pub transport: Transport,
    /// Immutable representation/acquisition profile digest.
    pub profile: Digest,
    /// Exact authorization context; never unions accounts.
    pub authorization_context: Digest,
    /// Exact payload provenance.
    pub representation: Representation,
    /// Parent capture/member evidence for derived representations.
    #[serde(deserialize_with = "Option::deserialize")]
    pub parent: Option<Handle>,
    /// Protected frozen run inputs.
    pub inputs: Handle,
    /// Current scoped access evidence.
    pub access: Handle,
    /// Admission decision evidence.
    pub decision: Handle,
}
/// Typed capture reuse key. Time and transfer metadata cannot duplicate a payload.
#[derive(Serialize)]
pub(super) struct CaptureKey<'a> {
    /// Schema domain separation.
    pub schema: &'static str,
    /// Source context.
    pub source: &'a str,
    /// Unique request/context frontier item.
    pub item: Handle,
    /// Exact authorization identity.
    pub authorization_context: &'a Digest,
    /// Representation profile identity.
    pub profile: &'a Digest,
    /// Exact representation label.
    pub representation: Representation,
    /// Immutable decoded body identity.
    pub artifact: &'a Digest,
}
impl CaptureEnvelope {
    /// Typed digest identity for immutable digest-identical reuse.
    /// # Errors
    /// Serialization refuses rather than guessing an identity.
    pub fn identity(&self) -> Result<Digest, ReceiptError> {
        Ok(Digest::of(&serde_json::to_vec(&CaptureKey {
            schema: "maestro-capture-key/1",
            source: &self.source,
            item: self.item,
            authorization_context: &self.authorization_context,
            profile: &self.profile,
            representation: self.representation,
            artifact: &self.artifact,
        })?))
    }
}
