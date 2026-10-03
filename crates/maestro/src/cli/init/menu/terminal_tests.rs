use super::run::{Request, interactive};
use crate::cli::{
    init::{
        command::{self, ApplyChoices},
        flow::{Answer, FlowPort},
        plain::Plain,
        terminal::{Screen, with_port},
    },
    output::Output,
};
use crate::failure::Failure;
use crossterm::event::Event;
use maestro_catalog::settings::FilePreferences;
use ratatui::{backend::TestBackend, buffer::Cell as BufferCell};
use std::{cell::Cell, env, fs, io, path::PathBuf, rc::Rc};

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

/// Observe preparation diagnostics at the actual retry prompt, not stderr bytes.
struct TransitionFrames<F> {
    screen: Screen<TestBackend, F>,
    fail_review: Rc<Cell<bool>>,
    frames: Vec<String>,
}
impl<F: FnMut() -> io::Result<Event>> FlowPort for TransitionFrames<F> {
    fn screen(&mut self, title: &str) -> Result<(), Failure> {
        if title.contains("5/5") {
            self.fail_review.set(true);
        }
        self.screen.screen(title)
    }
    fn notice(&mut self, text: &str) -> Result<(), Failure> {
        self.screen.notice(text)
    }
    fn refresh(&mut self) {
        self.screen.refresh();
    }
    fn show(&mut self, text: &str) -> Result<(), Failure> {
        self.screen.show(text)
    }
    fn ask(&mut self, label: &str) -> Result<Answer, Failure> {
        let answer = self.screen.ask(label)?;
        self.frames.push(
            self.screen
                .terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(BufferCell::symbol)
                .collect(),
        );
        Ok(answer)
    }
}

#[test]
fn catalog_terminal_preparation_failures_reach_retry_frames() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use maestro_catalog::{limits::Limits, settings::WorkspacePreferences};
    use maestro_settings::{Layers, Registry};
    use maestro_test_scratch::scratch_directory;
    use ratatui::{Terminal, backend::TestBackend};
    use std::{cell::Cell, collections::VecDeque, rc::Rc};
    struct Source(Rc<Cell<bool>>);
    impl WorkspacePreferences for Source {
        fn layers(&self, _: &Registry, _: &Limits) -> Result<Layers, String> {
            if self.0.replace(false) {
                Err("injected review preparation failure".into())
            } else {
                Ok(Layers::default())
            }
        }
    }
    for review in [false, true] {
        let root = scratch_directory().unwrap();
        // Keep the prompt short: native temp paths can wrap the diagnostic out of view.
        let catalog = PathBuf::from("missing-catalog");
        let fail_review = Rc::new(Cell::new(false));
        let source = Source(Rc::clone(&fail_review));
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
        let input = if review { "\n\nn\n\n\n\n" } else { "\n\ny\n" };
        let mut events: VecDeque<_> = input
            .chars()
            .map(|ch| {
                Event::Key(KeyEvent::new(
                    if ch == '\n' {
                        KeyCode::Enter
                    } else {
                        KeyCode::Char(ch)
                    },
                    KeyModifiers::NONE,
                ))
            })
            .collect();
        events.push_back(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        let screen = Screen::new(
            Terminal::new(TestBackend::new(80, 24)).unwrap(),
            false,
            || Ok(events.pop_front().expect("unexpected prompt")),
        );
        let mut port = TransitionFrames {
            screen,
            fail_review,
            frames: Vec::new(),
        };
        assert!(
            interactive(
                Output::new(false),
                &request,
                &[],
                &mut port,
                (&source, &root)
            )
            .unwrap()
            .is_none()
        );
        let last = port.frames.last().unwrap();
        assert!(last.contains("Error:"), "{review}: {last}");
        if review {
            assert!(
                last.contains("injected review preparation failure"),
                "{last}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn catalog_terminal_initial_and_changed_language_notices_are_frames() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use maestro_test_scratch::scratch_directory;
    use ratatui::{Terminal, backend::TestBackend};
    use std::{cell::Cell, collections::VecDeque, rc::Rc};
    for initial in [false, true] {
        let root = scratch_directory().unwrap();
        let source = FilePreferences::new(&root, &root);
        let presets = vec!["base".into()];
        let request = Request {
            catalog: Some(&root),
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
        let mut events: VecDeque<_> = if initial { "\n\nn\n\n" } else { "\n\nn\nja\n" }
            .chars()
            .map(|ch| {
                Event::Key(KeyEvent::new(
                    if ch == '\n' {
                        KeyCode::Enter
                    } else {
                        KeyCode::Char(ch)
                    },
                    KeyModifiers::NONE,
                ))
            })
            .collect();
        events.push_back(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        let screen = Screen::new(
            Terminal::new(TestBackend::new(80, 24)).unwrap(),
            false,
            || Ok(events.pop_front().expect("unexpected prompt")),
        );
        let mut port = TransitionFrames {
            screen,
            fail_review: Rc::new(Cell::new(false)),
            frames: Vec::new(),
        };
        let choices = if initial {
            vec!["language=ja".into()]
        } else {
            Vec::new()
        };
        assert!(
            interactive(
                Output::new(false),
                &request,
                &choices,
                &mut port,
                (&source, &root)
            )
            .unwrap()
            .is_none()
        );
        assert!(
            port.frames
                .iter()
                .any(|frame| frame
                    .contains("Interface is English; conversation language remains ja.")),
            "{initial}: {:?}",
            port.frames
        );
        fs::remove_dir_all(root).unwrap();
    }
}
