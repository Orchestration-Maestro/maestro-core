//! Shared draft and renderer port; S1 owns descriptors, validation and edits.
use crate::{
    cli::output::{Output, diagnose},
    failure::Failure,
    presentation::messages::MessageKey,
};
use maestro_catalog::settings::{ResolvedSettings, resolve};
use maestro_settings::{
    Flag, Layer, LayerName, Layers, Registry, SettingClass, Value, parse_flags,
};
use std::path::PathBuf;

/// Renderer-independent navigation, including EOF/interruption cancellation.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::cli) enum Answer {
    /// Literal input; empty text keeps the current choice.
    Text(String),
    /// Return to the previous stage with choices intact.
    Back,
    /// Stop without effects.
    Cancel,
}

/// Plain now, a terminal renderer later; neither adapter writes settings or trust.
pub(in crate::cli) trait FlowPort {
    /// Start a stage; plain rendering retains the existing labelled transcript.
    fn screen(&mut self, title: &str) -> Result<(), Failure> {
        self.show(title)
    }
    /// Full review text; plain keeps the historic single screen line.
    fn review_screen(&mut self, text: &str) -> Result<(), Failure> {
        self.screen(text)
    }
    /// Fallback notices remain stderr diagnostics in the plain transcript.
    fn notice(&mut self, text: &str) -> Result<(), Failure> {
        diagnose(text);
        Ok(())
    }
    /// The existing planner transcript goes to stdout in plain mode, into the frame in TUI.
    fn plan(&mut self, text: &str) -> Result<(), Failure> {
        Output::new(false).text(text)
    }
    /// Refresh descriptor presentation; plain retains the existing transcript.
    fn refresh(&mut self) {}
    /// Render information without changing the draft.
    fn show(&mut self, text: &str) -> Result<(), Failure>;
    /// Read one labelled choice or navigation action.
    fn ask(&mut self, label: &str) -> Result<Answer, Failure>;
}

/// One registry-generated editor draft shared by init and config.
pub(in crate::cli) struct Draft {
    /// Admitted S1 descriptors.
    pub(in crate::cli) registry: Registry,
    /// Captured preference layers.
    layers: Layers,
    /// Run-only flags, never implicitly persisted.
    pub(in crate::cli) flags: Vec<Flag>,
    /// Existing S1 target layer.
    pub(in crate::cli) layer: LayerName,
    /// Validated pending setting assignments.
    pub(in crate::cli) choices: Vec<String>,
    /// Whether canonical init language rules apply.
    init: bool,
    /// Existing localized message selection.
    pub(in crate::cli) output: Output,
}

impl Draft {
    /// Capture immutable layers; previews and cancellation touch no persistence API.
    pub(in crate::cli) fn new(
        registry: Registry,
        layers: Layers,
        layer: LayerName,
        choices: &[String],
        init: bool,
    ) -> Result<Self, Failure> {
        let mut draft = Self {
            registry,
            layers,
            flags: Vec::new(),
            layer,
            choices: Vec::new(),
            init,
            output: Output::new(false),
        };
        for choice in choices {
            draft.edit(choice)?;
        }
        Ok(draft)
    }

    /// Validate through the existing settings operation, retaining the old draft on error.
    pub(in crate::cli) fn edit(&mut self, text: &str) -> Result<(), Failure> {
        let (key, value) = text.split_once('=').ok_or_else(|| {
            Failure::refused("enter KEY=VALUE; `maestro config list` names every setting")
        })?;
        let key = key.trim();
        let value = value.trim();
        let parsed = validate(&self.registry, key, Some(value), self.layer)?;
        if self.init && key == "language" && value == "auto" {
            return Err(Failure::refused(
                "init language must be a canonical language tag, not auto",
            ));
        }
        let value = parsed.ok_or_else(|| Failure::failed("an editor value is missing"))?;
        self.choices
            .retain(|choice| choice.split_once('=').is_none_or(|(other, _)| other != key));
        self.choices.push(format!("{key}={value}"));
        Ok(())
    }

    /// Project edits through S1 layers and C17's resolver, not a second resolver.
    fn resolved(&self) -> Result<ResolvedSettings, Failure> {
        let mut layers = self.layers.clone();
        let slot = match self.layer {
            LayerName::User => &mut layers.user,
            LayerName::Project => &mut layers.project,
        };
        let (path, mut layer) = slot
            .take()
            .unwrap_or_else(|| (PathBuf::new(), Layer::default()));
        for flag in parse_flags(&self.registry, &self.choices).map_err(Failure::refused)? {
            layer = layer.with(&flag.key, Some(&flag.value));
        }
        *slot = Some((path, layer));
        Ok(resolve(
            &self.registry,
            &maestro_settings::resolve(&self.registry, &layers, &self.flags),
        ))
    }

    /// Interface selection follows this exact admitted, edited snapshot.
    pub(in crate::cli) fn language_output(&self, output: Output) -> Result<Output, Failure> {
        output.with_language(self.resolved()?.text("language").unwrap_or("auto"))
    }

    /// Interactive fallback diagnostics belong to the selected renderer.
    pub(in crate::cli) fn language_output_on(
        &self,
        output: Output,
        port: &mut dyn FlowPort,
    ) -> Result<Output, Failure> {
        output.with_language_to(
            self.resolved()?.text("language").unwrap_or("auto"),
            |text| port.notice(text),
        )
    }

    /// Every descriptor, never a screen-specific list, with provenance and restrictions.
    pub(in crate::cli) fn show(&self, port: &mut dyn FlowPort) -> Result<(), Failure> {
        port.refresh();
        let resolved = self.resolved()?;
        for descriptor in self.registry.descriptors() {
            let setting = resolved
                .get(&descriptor.key)
                .ok_or_else(|| Failure::failed("registry descriptor has no resolution"))?;
            let (value, source) = match setting {
                Ok(setting) => (setting.value().to_string(), setting.source()),
                Err(error) => (format!("unavailable: {}", error.message), "refused"),
            };
            port.show(&format!(
                "{} = {value}\n  accepts: {}; source: {source}; {}\n  {}",
                descriptor.key,
                descriptor.kind.expectation(),
                descriptor.description,
                restriction(descriptor, self.layer)
            ))?;
        }
        for diagnostic in resolved.diagnostics() {
            port.show(&format!("{}: {}", diagnostic.key, diagnostic.message))?;
        }
        port.show(
            "Trust is authority, not a preference: use `maestro trust add`, `list` or `remove`.",
        )
    }
}

/// Restrictions come from descriptors and the existing layer contract.
fn restriction(descriptor: &maestro_settings::SettingDescriptor, layer: LayerName) -> String {
    if descriptor.class == SettingClass::Locked {
        "Locked: no file or flag may edit; inspect with `maestro config explain`.".to_owned()
    } else if descriptor.standard_only {
        "Standard-only: edit the central standard; `maestro catalog check` validates it.".to_owned()
    } else if descriptor.key == "updates" && layer == LayerName::Project {
        "Workspace may choose off or propose; Auto is user-only (`maestro config set updates \
            auto --user`)."
            .to_owned()
    } else {
        format!("Editable in {} preferences.", layer.name())
    }
}

/// Edit one prominent descriptor; errors keep the value and focus on the same field.
pub(in crate::cli) fn preference(
    port: &mut dyn FlowPort,
    draft: &mut Draft,
    key: &str,
    help: &str,
) -> Result<Answer, Failure> {
    let descriptor = draft
        .registry
        .get(key)
        .ok_or_else(|| Failure::failed("missing preference descriptor"))?;
    let resolved = draft.resolved()?;
    let current = resolved
        .get(key)
        .and_then(|value| value.as_ref().ok())
        .ok_or_else(|| Failure::refused("preference cannot be resolved"))?;
    let label = format!(
        "{key} = {} (source: {}; accepts: {})\n{help}\n{key} [Enter keeps current]: ",
        current.value(),
        current.source(),
        descriptor.kind.expectation()
    );
    loop {
        match port.ask(&label)? {
            Answer::Text(text) if !text.is_empty() => match draft.edit(&format!("{key}={text}")) {
                Ok(()) => return Ok(Answer::Text(text)),
                Err(error) => port.show(&format!("Error: {error}"))?,
            },
            answer => return Ok(answer),
        }
    }
}

/// The same every-setting editor is used by both entry points.
pub(in crate::cli) fn editor(
    port: &mut dyn FlowPort,
    draft: &mut Draft,
) -> Result<Answer, Failure> {
    draft.show(port)?;
    loop {
        match port.ask(&draft.output.wording(MessageKey::FlowEditorPrompt, &[])?)? {
            Answer::Text(text) if !text.is_empty() => match draft.edit(&text) {
                Ok(()) => {
                    if text
                        .split_once('=')
                        .is_some_and(|(key, _)| key.trim() == "language")
                    {
                        draft.output = draft.language_output_on(draft.output, port)?;
                    }
                    draft.show(port)?;
                }
                Err(error) => port.show(&format!("Error: {error}"))?,
            },
            answer => return Ok(answer),
        }
    }
}

/// Default-no confirmation, never authorizing effects without --apply.
pub(in crate::cli) fn review(
    port: &mut dyn FlowPort,
    apply: bool,
    output: Output,
) -> Result<Answer, Failure> {
    loop {
        match port.ask(&output.wording(
            if apply {
                MessageKey::FlowApplyPrompt
            } else {
                MessageKey::FlowPreviewPrompt
            },
            &[],
        )?)? {
            Answer::Text(text) if apply && text.eq_ignore_ascii_case("yes") => {
                return Ok(Answer::Text("yes".to_owned()));
            }
            Answer::Text(text)
                if matches!(
                    text.to_ascii_lowercase().as_str(),
                    "" | "n" | "no" | "preview"
                ) =>
            {
                return Ok(Answer::Text("preview".to_owned()));
            }
            Answer::Text(_) => port.show(
                "Error: choose back, cancel, preview or explicit yes when --apply is present.",
            )?,
            answer => return Ok(answer),
        }
    }
}

/// Common descriptor/layer validation, before any file or journal is opened.
pub(in crate::cli) fn validate(
    registry: &Registry,
    key: &str,
    value: Option<&str>,
    layer: LayerName,
) -> Result<Option<Value>, Failure> {
    let descriptor = registry.get(key).ok_or_else(|| {
        Failure::refused(format!(
            "unknown key {key:?}: `maestro config list` names every setting"
        ))
    })?;
    if descriptor.class == SettingClass::Locked {
        return Err(Failure::refused(format!(
            "{key}: the setting is locked: no file or flag may change it"
        )));
    }
    if descriptor.standard_only {
        return Err(Failure::refused(format!(
            "{key}: standard-only; edit the central standard and validate with \
            `maestro catalog check --catalog-dir DIR`"
        )));
    }
    if layer == LayerName::Project && key == "updates" && value == Some("auto") {
        return Err(Failure::refused(
            "updates: auto is user-only; use `maestro config set updates auto --user`",
        ));
    }
    value
        .map(|text| descriptor.kind.parse_text(text))
        .transpose()
        .map_err(|error| Failure::refused(format!("{key}: {error}")))
}
