//! What the gateway's tests share: a scratch store, a card for each role, and
//! the values those cards record.

use super::super::{CardFields, Limits, ModelCard, Role, RouterEntry, SuiteResult};
use crate::artifact::{Digest, Store};
use std::{
    env, fs,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The llama.cpp build the stub's model servers report.
pub(super) const BUILD: &str = "b6500-3f2c9a1b";
/// The chat template the stub's model servers report.
pub(super) const TEMPLATE: &str =
    "{% for message in messages %}{{ message.content }}\n{% endfor %}";
/// SHA-256 of [`TEMPLATE`], which the answerer's card records.
pub(super) const TEMPLATE_DIGEST: &str =
    "130da0a1ba009156a03a7f22425455175710d4d2df7aa559079389863d2759f2";
/// SHA-256 of `model file`, the file digest every card here records.
pub(super) const FILE_DIGEST: &str =
    "ca56847038f3f329524caec5a86e14865f49918d68094c90dd021a5e67b927f6";
/// SHA-256 of `report`, the report every card here names.
pub(super) const REPORT_DIGEST: &str =
    "845e91831319e89c4d656bdb80c278ac09a7230d61e5dfd2e1b1fbb436ac8917";

/// A directory of its own under the temporary directory, removed with it.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-gateway-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// An artifact store rooted in the directory.
    pub(super) fn store(&self) -> Store {
        Store::new(&self.0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The digest `hex` names.
pub(super) fn digest(hex: &str) -> Digest {
    Digest::parse(hex).unwrap()
}

/// What a card for `role` records: the embedder's vectors have 3 dimensions,
/// and the answerer alone records a chat template.
pub(super) fn fields(role: Role) -> CardFields {
    let (entry, dimensions, template_digest) = match role {
        Role::Embedder => ("embed", NonZeroUsize::new(3), None),
        Role::Reranker => ("rerank", None, None),
        Role::Answerer => ("answer", None, Some(digest(TEMPLATE_DIGEST))),
    };
    CardFields {
        role,
        router_entry: RouterEntry::parse(entry).unwrap(),
        file_digest: digest(FILE_DIGEST),
        template_digest,
        server_build: BUILD.to_owned(),
        dimensions,
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: vec![SuiteResult {
            suite: "synthetic-retrieval".to_owned(),
            report: digest(REPORT_DIGEST),
        }],
    }
}

/// `fields` recorded as a card, in a store that is gone once it returns.
pub(super) fn card_of(fields: &CardFields) -> ModelCard {
    let scratch = Scratch::new();
    ModelCard::record(&scratch.store(), fields).unwrap()
}

/// The card of [`fields`] for `role`.
pub(super) fn card(role: Role) -> ModelCard {
    card_of(&fields(role))
}
