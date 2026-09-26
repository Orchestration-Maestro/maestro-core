//! What the router tokenizer's tests share: model cards, recorded in a
//! scratch store that is gone once they are made.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
};
use std::{
    env, fs,
    num::{NonZeroU32, NonZeroUsize},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The llama.cpp build the stub router's embedder reports.
pub(super) const BUILD: &str = "b6500-3f2c9a1b";
/// The file digest the embedder's cards record, SHA-256 of `model file`.
pub(super) const MODEL_FILE: &str =
    "ca56847038f3f329524caec5a86e14865f49918d68094c90dd021a5e67b927f6";
/// Another model file's digest, SHA-256 of `another model file`.
pub(super) const OTHER_FILE: &str =
    "dfb5dcc5de5d764a869a534c112278852326fa24ba93e2c13dac6886876c3fd2";

/// The digest `hex` names.
pub(super) fn digest(hex: &str) -> Digest {
    Digest::parse(hex).unwrap()
}

/// The card of a model filling `role` as the router's entry `embed`, from
/// the file `file_digest` names, served by the llama.cpp build
/// `server_build`.
pub(super) fn card(role: Role, file_digest: &str, server_build: &str) -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let fields = CardFields {
        role,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: digest(file_digest),
        template_digest: None,
        server_build: server_build.to_owned(),
        dimensions: (role == Role::Embedder).then(|| NonZeroUsize::new(1024).unwrap()),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let root = env::temp_dir().join(format!(
        "maestro-knowledge-prepare-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
}

/// The embedder's card the stub router serves: [`MODEL_FILE`] under
/// [`BUILD`].
pub(super) fn embedder() -> ModelCard {
    card(Role::Embedder, MODEL_FILE, BUILD)
}
