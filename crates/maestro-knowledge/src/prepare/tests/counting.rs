//! Counting and verifying through a qualified tokenizer: the port's IDs in
//! order, free room on every call, the canaries tokenized again, and the
//! refusals that stop a count.

use super::{
    super::RouterTokenizer,
    port::{Answer, Goldens},
    support::embedder,
};
use maestro_canonicalization::TokenCounter;
use maestro_kernel::gateway::Room;

/// A tokenizer qualified over `port`.
fn qualified(port: &Goldens) -> RouterTokenizer {
    RouterTokenizer::qualify(port.clone(), embedder()).unwrap()
}

#[test]
fn token_ids_come_back_in_order_through_the_fake() {
    let tokenizer = qualified(&Goldens::new());
    // The fake's tokens: the first four bytes of each word's SHA-256, read
    // little-endian.
    assert_eq!(
        tokenizer.token_ids("Chunks count in order.").unwrap(),
        [3_325_001_395, 977_876_332, 1_399_269_720, 2_524_900_158]
    );
}

#[test]
fn every_call_asks_for_free_room() {
    let port = Goldens::new();
    let tokenizer = qualified(&port);
    tokenizer.token_ids("Chunks count in order.").unwrap();
    tokenizer.verify().unwrap();
    // 41 fixtures, one count and 8 canaries.
    assert_eq!(port.rooms(), [Room::Free; 50]);
}

#[test]
fn verify_tokenizes_the_canaries_again_and_refuses_one_that_changed() {
    let port = Goldens::new();
    let tokenizer = qualified(&port);
    tokenizer.verify().unwrap();
    let canaries = [
        "",
        "Hello world",
        "👩🏽\u{200d}💻 👨\u{200d}👩\u{200d}👧\u{200d}👦 🇫🇷 ✅",
        "cafe\u{301} nai\u{308}ve A\u{30a}ngstro\u{308}m",
        "ＡＢＣ ﬁ ① ² Ⅳ",
        "  a   b  ",
        "\talpha\r\n\tbeta\r\n",
        "<s>Hello</s><pad><unk><mask>",
    ];
    assert_eq!(port.texts()[41..], canaries);
    port.answer("<s>Hello</s><pad><unk><mask>", Answer::Ids(vec![0, 1, 2]));
    let refused = tokenizer.verify().unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the router's IDs for the parity fixture specials differ from those of the native \
         counter: native [0, 0, 35378, 2, 1, 3, 4426, 1510, 92, 2740, 2], router [0, 1, 2]"
    );
}

#[test]
fn an_embedder_that_no_longer_fits_in_free_room_refuses_to_count() {
    let port = Goldens::new();
    let tokenizer = qualified(&port);
    port.answer("Chunks count in order.", Answer::Unavailable);
    port.answer("Hello world", Answer::Unavailable);
    let unavailable = "the embedder does not fit in the router's free room, and counting \
                       never unloads another model: no free room for 1280 MiB";
    let counted = tokenizer.token_ids("Chunks count in order.").unwrap_err();
    assert_eq!(counted.to_string(), unavailable);
    assert_eq!(tokenizer.verify().unwrap_err().to_string(), unavailable);
}

#[test]
fn a_port_that_panics_stops_every_later_count() {
    let port = Goldens::new();
    let tokenizer = qualified(&port);
    port.answer("Chunks", Answer::Panic);
    let stopped = "the tokenizer's thread stopped: a call to the model port panicked";
    assert_eq!(
        tokenizer.token_ids("Chunks").unwrap_err().to_string(),
        stopped
    );
    assert_eq!(
        tokenizer.token_ids("Hello world").unwrap_err().to_string(),
        stopped
    );
    assert_eq!(tokenizer.verify().unwrap_err().to_string(), stopped);
}
