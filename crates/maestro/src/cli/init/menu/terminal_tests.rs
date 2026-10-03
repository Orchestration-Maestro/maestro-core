use super::run::{Request, interactive};
use crate::cli::{
    init::{
        command::{self, ApplyChoices},
        flow::{Answer, FlowPort},
        plain::Plain,
        terminal::with_port,
    },
    output::Output,
};
use crate::failure::Failure;
use maestro_catalog::settings::FilePreferences;
use std::{env, fs, path::PathBuf};

/// Spy on the semantic port, then forward the exact same bytes to the renderer.
struct Recording<'a> {
    port: &'a mut dyn FlowPort,
    plans: &'a mut Vec<String>,
}
impl FlowPort for Recording<'_> {
    fn screen(&mut self, title: &str) -> Result<(), Failure> {
        self.port.screen(title)
    }
    fn refresh(&mut self) {
        self.port.refresh();
    }
    fn show(&mut self, text: &str) -> Result<(), Failure> {
        self.port.show(text)
    }
    fn ask(&mut self, label: &str) -> Result<Answer, Failure> {
        self.port.ask(label)
    }
    fn plan(&mut self, text: &str) -> Result<(), Failure> {
        self.plans.push(text.to_owned());
        self.port.plan(text)
    }
}

/// The native PTY parent runs this in an isolated home/project with real key bytes.
#[test]
#[ignore = "PTY subprocess entry; the parent runs and checks it"]
fn catalog_terminal_plan_parity_child() {
    let root = env::current_dir().unwrap().canonicalize().unwrap();
    let config = PathBuf::from(env::var_os("XDG_CONFIG_HOME").unwrap()).join("maestro");
    let source = FilePreferences::new(&config, &root);
    let catalog = PathBuf::from(env::var_os("MAESTRO_TERMINAL_CATALOG").unwrap());
    let presets = vec!["base".to_owned()];
    let request = Request {
        catalog: Some(&catalog),
        presets: &presets,
        plain: false,
        yes: false,
        effects: ApplyChoices {
            apply: false,
            preferences_only: false,
            non_interactive: false,
            confirm_path: None,
        },
    };
    let mut rendered = Vec::new();
    let reviewed = with_port(false, true, |port| {
        interactive(
            Output::new(false),
            &request,
            &[],
            &mut Recording {
                port,
                plans: &mut rendered,
            },
            (&source, &root),
        )
    })
    .unwrap();
    assert!(reviewed.is_none(), "preview never confirms an apply");
    let mut plain_plans = Vec::new();
    let mut transcript = Vec::new();
    let mut input = "\n\ny\nen\nbrief\nupdates=off\n\npreview\n".as_bytes();
    let mut plain = Plain {
        input: &mut input,
        output: &mut transcript,
    };
    assert!(
        interactive(
            Output::new(false),
            &request,
            &[],
            &mut Recording {
                port: &mut plain,
                plans: &mut plain_plans
            },
            (&source, &root)
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        rendered, plain_plans,
        "renderer and plain exact plan parity"
    );
    let script = command::prepare(
        &catalog,
        &presets,
        &source,
        &[
            "language=en".into(),
            "tone=brief".into(),
            "updates=off".into(),
        ],
    )
    .unwrap();
    assert_eq!(
        rendered.last().unwrap(),
        &script.text(false).unwrap(),
        "script exact plan parity"
    );
    assert!(rendered.last().unwrap().contains(".maestro/config.toml"));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0, "unconfirmed write");
    assert!(!config.join("preferences.toml").exists());
    assert!(
        !PathBuf::from(env::var_os("XDG_DATA_HOME").unwrap())
            .join("maestro/kernel.sqlite3")
            .exists()
    );
    println!("PARITY: renderer/plain/script exact plan bytes; zero unconfirmed writes");
}
