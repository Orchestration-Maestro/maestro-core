//! The extractor identity binds every fixed input sent with its request.

use super::super::{extract::response_format, extraction_prompt_digest, extraction_system_prompt};
use crate::artifact::Digest;
use serde_json::json;

#[test]
fn digest_covers_system_schema_and_user_template() {
    let identity = json!({
        "system_prompt": extraction_system_prompt(),
        "response_format": response_format(),
        "user_message_template": "{window_text}",
    });
    assert_eq!(
        extraction_prompt_digest(),
        Digest::of(identity.to_string().as_bytes())
    );
}
