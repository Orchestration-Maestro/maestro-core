//! Owned interface data carried through failures without changing English machine records.
use super::messages::{Interface, MessageKey, interpolate};
use std::fmt;

/// A typed template and literal data, never a concatenation of translated fragments.
#[derive(Debug)]
pub(crate) struct Message {
    /// The interface identity selected at the human boundary.
    key: MessageKey,
    /// Literal owned values survive unwinding the command's local variables.
    values: Vec<(&'static str, Argument)>,
}

/// An instruction is a complete typed message, never pretranslated prose in machine data.
#[derive(Debug)]
enum Argument {
    /// Opaque user or downstream data.
    Text(String),
    /// Complete instruction rendered in the same interface as its enclosing diagnostic.
    Message(Box<Message>),
}

impl Message {
    /// Capture data separately from the translated template.
    pub(crate) fn new(key: MessageKey, values: &[(&'static str, &str)]) -> Self {
        Self {
            key,
            values: values
                .iter()
                .map(|(name, value)| (*name, Argument::Text((*value).to_owned())))
                .collect(),
        }
    }

    /// Attach a whole instruction without storing translated text beside its identity.
    pub(crate) fn with_message(mut self, name: &'static str, message: Self) -> Self {
        self.values
            .push((name, Argument::Message(Box::new(message))));
        self
    }

    /// Render once using the selected interface; inserted data is never reinterpreted.
    pub(crate) fn render(&self, interface: Interface) -> Result<String, String> {
        let values: Vec<_> = self
            .values
            .iter()
            .map(|(name, value)| {
                let text = match value {
                    Argument::Text(text) => Ok(text.clone()),
                    Argument::Message(message) => message.render(interface),
                }?;
                Ok((*name, text))
            })
            .collect::<Result<_, String>>()?;
        let borrowed: Vec<_> = values
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        interpolate(interface.template(self.key), &borrowed)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let interface = Interface::select("en").map_err(|_| fmt::Error)?;
        let text = self.render(interface).map_err(|_| fmt::Error)?;
        formatter.write_str(&text)
    }
}
