//! The router tokenizer's parity with the native counter, live: an explicit
//! local test, like the native ones, never a pass when ignored. It qualifies
//! the model router's embedder on the 41 parity fixtures, whose goldens `just
//! native` checks against the native counter, so `just parity` runs both.
//!
//! It reads the router's variables that `live_router.rs` names, and builds
//! the embedder's card from what the router and the model file report, as
//! T030 will record the real cards.
//!
//! Every request asks for free room, which only a router with free room
//! (T002, redeployed) honours: an older one ignores it, and may unload the
//! chat model to load the embedder. Run it against no other.
#![cfg(test)]

use super::live_router::embedder_card;
use maestro_canonicalization::TokenCounter;
use maestro_kernel::{artifact::Store, gateway::RouterClient};
use maestro_knowledge::prepare::RouterTokenizer;
use std::{env, fs, process};

#[test]
#[ignore = "needs a model router with free room (T002 redeployed): an older one may unload the \
            chat model; run explicitly"]
fn the_router_gives_every_parity_fixture_the_native_ids() {
    let scratch = env::temp_dir().join(format!("maestro-router-parity-{}", process::id()));
    let (base, card) = embedder_card(&Store::new(&scratch));
    fs::remove_dir_all(&scratch).unwrap();
    let tokenizer = RouterTokenizer::qualify(RouterClient::new(base).unwrap(), card).unwrap();
    tokenizer.verify().unwrap();
    assert_eq!(
        tokenizer.token_ids("Hello world").unwrap(),
        [0, 35378, 8999, 2]
    );
    println!(
        "router parity: {} gives the 41 fixtures the native IDs",
        tokenizer.contract_id()
    );
}
