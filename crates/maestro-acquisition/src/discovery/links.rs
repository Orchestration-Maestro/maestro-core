//! Offline link extraction over verified, bounded captured bytes; never a client.
use dom_query::{Document, Matcher};
use maestro_kernel::{acquisition::ReceiptError, artifact::Digest};
use serde::Deserialize;
use std::{collections::BTreeSet, future::Future, pin::Pin, str};
use url::Url;

/// Exact contract output, with limits distinct from successful complete extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkExtraction {
    /// Canonically sorted, case-sensitive absolute URL candidates.
    pub links: Vec<String>,
    /// Exact extractor version/function/selectors, retained in partition evidence.
    pub contract: String,
    /// The extractor stopped early; its inventory cannot prove completeness.
    pub limit_hit: bool,
}
/// A replaceable offline extractor; substitutes cannot dispatch network effects.
pub trait LinkExtractor: Send + Sync {
    /// Declared contract, also available when a byte cap prevents parser entry.
    fn contract(&self) -> &str;
    /// Extract links from admitted capture bytes and their credential-free base URL.
    /// # Errors
    /// Unsupported or unavailable extraction holds discovery, never falls back.
    fn extract<'a>(
        &'a self,
        url: &'a str,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>>;
}
/// Exact data-defined link and base selection contract.
pub const LINK_SELECTORS: &str = include_str!("link-selectors.json");
/// A data-selected attribute, validated without panic-prone selector APIs.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    /// Exact CSS selection spelling.
    selector: String,
    /// Exact URL-bearing attribute.
    attribute: String,
}
/// One versioned list; hidden anchors are not excluded.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    /// First HTML base element, resolved against the captured final URL.
    base: Selector,
    /// Explicit supported link/resource attributes, with no code tag dispatch.
    links: Vec<Selector>,
}
/// Offline HTML5 link reader. Default Markdown/optional hashbrown features are off.
/// Empty/fragment-only references are self-links unless HTML supplies a different
/// base document. RFC 3986 query inheritance therefore needs no redacted query.
#[derive(Debug)]
pub struct DomLinks {
    /// Exact immutable selector-list digest and library identity.
    contract: String,
    /// Data-defined link selectors.
    rules: Rules,
}
impl DomLinks {
    /// Load and validate the pinned selector contract without choosing a fallback.
    /// # Errors
    /// Malformed selector data refuses before any page is parsed.
    pub fn new() -> Result<Self, ReceiptError> {
        let rules: Rules = serde_json::from_str(LINK_SELECTORS)?;
        for rule in rules.links.iter().chain([&rules.base]) {
            Matcher::new(&rule.selector).map_err(|_| ReceiptError::Invalid)?;
        }
        Ok(Self {
            contract: format!(
                "dom_query 0.28.0;utf8-html5;selectors:{}",
                Digest::of(LINK_SELECTORS.as_bytes()).as_str()
            ),
            rules,
        })
    }
    /// HTML5 base and URL resolution over already bounded UTF-8 bytes.
    fn read(&self, url: &str, bytes: &[u8]) -> Result<LinkExtraction, ReceiptError> {
        let text = str::from_utf8(bytes).map_err(|_| ReceiptError::Invalid)?;
        let document = Document::from(text);
        let mut base = Url::parse(url).map_err(|_| ReceiptError::Invalid)?;
        let mut limit_hit = false;
        let mut base_is_document = true;
        let base_matcher =
            Matcher::new(&self.rules.base.selector).map_err(|_| ReceiptError::Invalid)?;
        if let Some(href) = document
            .select_single_matcher(&base_matcher)
            .attr(&self.rules.base.attribute)
        {
            match base.join(&href) {
                Ok(resolved) => {
                    base_is_document = self_reference(&href);
                    base = resolved;
                }
                Err(_) => {
                    limit_hit = true;
                }
            }
        }
        let mut links = BTreeSet::new();
        for rule in &self.rules.links {
            if collect_rule(&document, &base, rule, &mut links, base_is_document)? {
                limit_hit = true;
                break;
            }
        }
        Ok(LinkExtraction {
            links: links.into_iter().collect(),
            contract: self.contract.clone(),
            limit_hit,
        })
    }
}
impl LinkExtractor for DomLinks {
    fn contract(&self) -> &str {
        &self.contract
    }
    fn extract<'a>(
        &'a self,
        url: &'a str,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        Box::pin(async move { self.read(url, bytes) })
    }
}

/// Stop on malformed URLs or the kernel's finite inventory ceiling, never claim coverage.
fn collect_rule(
    document: &Document,
    base: &Url,
    rule: &Selector,
    links: &mut BTreeSet<String>,
    base_is_document: bool,
) -> Result<bool, ReceiptError> {
    let matcher = Matcher::new(&rule.selector).map_err(|_| ReceiptError::Invalid)?;
    for node in document.select_matcher(&matcher).nodes() {
        let Some(value) = node.attr(&rule.attribute) else {
            continue;
        };
        if base_is_document && self_reference(&value) {
            continue;
        }
        let Ok(mut resolved) = base.join(&value) else {
            return Ok(true);
        };
        resolved.set_fragment(None);
        links.insert(resolved.to_string());
        if links.len() > 1000 {
            links.pop_last();
            return Ok(true);
        }
    }
    Ok(false)
}

/// These two RFC 3986 reference forms denote the base itself, including its query.
fn self_reference(value: &str) -> bool {
    let value = value.trim_matches(|character| character <= ' ');
    value.is_empty() || value.starts_with('#')
}
