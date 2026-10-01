//! Why a preparation stops: what each stop says, and the cause each keeps.

use super::super::{Error, TokenizerError};
use maestro_kernel::{chunk_set, document, store};
use std::error::Error as _;

#[test]
fn a_stop_before_any_work_or_by_the_caller_has_no_cause() {
    for stop in [
        Error::NotVisible("workspace/default/collection/notes".to_owned()),
        Error::Failed("chunk-set-1".to_owned()),
        Error::Stopped,
    ] {
        assert!(stop.source().is_none(), "{stop}");
    }
}

#[test]
fn a_stop_part_way_says_what_failed_and_keeps_its_cause() {
    let sqlite = || store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let stops = [
        (
            Error::Counter(TokenizerError::Unavailable {
                reason: "no free room".to_owned(),
            }),
            "the counter refused: the embedder does not fit in the router's free room, and \
             counting never unloads another model: no free room",
        ),
        (
            Error::Records(document::Error::Store(sqlite())),
            "the kernel's records failed: the kernel database refused the operation",
        ),
        (
            Error::ChunkSet(chunk_set::Error::UnknownChunkSet("chunk-set-1".to_owned())),
            "the chunk set failed: no chunk set chunk-set-1 is recorded",
        ),
        (
            Error::Artifacts(sqlite()),
            "the artifact store failed: the kernel database refused the operation",
        ),
        (
            Error::Manifest(serde_json::from_str::<u8>("x").unwrap_err()),
            "the chunk set's manifest is not valid: expected value at line 1 column 1",
        ),
    ];
    for (stop, says) in stops {
        assert_eq!(stop.to_string(), says);
        let cause = stop.source().map(ToString::to_string).unwrap();
        assert!(says.ends_with(&cause), "{says} ends with {cause}");
    }
}
