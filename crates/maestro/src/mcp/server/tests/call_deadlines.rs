//! The MCP call caps against the search and chat caps they wrap.

use super::super::types::{ASK_CALL_DEADLINE, CALL_DEADLINE};
use maestro_kernel::evidence::RequestBudget;
use maestro_knowledge::answer::CHAT_DEADLINE;
use std::time::Duration;

/// A common MCP client tool timeout; every cap stays under it.
const CLIENT_TIMEOUT: Duration = Duration::from_mins(1);

/// The longest search a caller may ask for.
fn longest_search() -> Duration {
    Duration::from_millis(u64::from(RequestBudget::MAX_DEADLINE_MS))
}

#[test]
fn a_search_call_outlasts_the_longest_search_and_stays_under_client_timeouts() {
    assert_eq!(longest_search(), Duration::from_secs(30));
    assert!(CALL_DEADLINE > longest_search());
    assert!(CALL_DEADLINE < CLIENT_TIMEOUT);
}

#[test]
fn an_ask_call_outlasts_the_longest_search_and_one_chat_and_stays_under_client_timeouts() {
    assert!(ASK_CALL_DEADLINE >= longest_search() + CHAT_DEADLINE);
    assert!(ASK_CALL_DEADLINE < CLIENT_TIMEOUT);
}
