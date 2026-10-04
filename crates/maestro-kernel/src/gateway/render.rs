//! Exact chat framing through the pinned model's template endpoint.
use super::{
    card::ModelCard,
    port::{ChatRequest, Error},
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Render the same messages and template controls used for generation.
pub(super) fn body(card: &ModelCard, request: &ChatRequest) -> Result<Value, Error> {
    request.validate(card)?;
    Ok(json!({
        "messages": request.messages,
        "add_generation_prompt": true,
        "chat_template_kwargs": request.template_values()?,
    }))
}

/// llama-server's rendered template, including its special tokens.
#[derive(Deserialize)]
pub(super) struct Rendered {
    /// Complete text to tokenize before reserving context for generation.
    prompt: String,
}

impl Rendered {
    /// Refuse an empty rendered prompt instead of charging zero tokens.
    pub(super) fn into_prompt(self) -> Result<String, Error> {
        if self.prompt.is_empty() {
            return Err(Error::InvalidAnswer {
                reason: "empty rendered chat prompt".into(),
            });
        }
        Ok(self.prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::Rendered;
    use serde_json::json;

    #[test]
    fn render_chat_rejects_empty_and_invalid_prompt_envelopes() {
        for value in [json!({"prompt":""}), json!({"prompt":17}), json!({})] {
            let result = serde_json::from_value::<Rendered>(value)
                .map_err(|_| ())
                .and_then(|response| response.into_prompt().map_err(|_| ()));
            assert!(result.is_err());
        }
    }
}
