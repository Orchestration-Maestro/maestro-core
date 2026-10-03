//! Real backend parser and cleanup receipts, including unwinding inside the renderer.
use super::Pty;
use crate::{
    cli::init::{flow::Answer, terminal::with_port},
    cli::output::Output,
    failure::Failure,
};
use crossterm::terminal;
use maestro_test_scratch::scratch_directory;
use std::{
    env, fs,
    panic::{AssertUnwindSafe, catch_unwind},
    process::Command,
};

/// Invoked only by the parent under a real controlling PTY/ConPTY.
#[test]
#[ignore = "PTY subprocess entry; the parent runs and checks it"]
fn catalog_terminal_pty_child() {
    #[cfg(unix)]
    let before = super::unix::modes();
    #[cfg(windows)]
    let before = super::windows::modes();
    let case = env::var("MAESTRO_TERMINAL_CASE").unwrap();
    let color = Output::new(false).without_color(case == "no-color").color();
    if case == "no-color-env" {
        assert!(!color, "NO_COLOR must select a monochrome frame");
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_port(false, color, |port| {
            port.screen("1/5 Workspace")?;
            port.show("Root: synthetic; no implicit trust")?;
            let answer = port.ask("READY: type or cancel")?;
            match case.as_str() {
                "error" => return Err(Failure::failed("injected loop failure")),
                "panic" => panic!("injected renderer unwind"),
                "normal" => assert_eq!(answer, Answer::Text(String::new())),
                "back" => assert_eq!(answer, Answer::Back),
                _ => assert_eq!(answer, Answer::Cancel),
            }
            if case == "resize" {
                assert_eq!(terminal::size().unwrap(), (100, 30));
            }
            Ok(())
        })
    }));
    #[cfg(unix)]
    super::unix::assert_modes(&before);
    #[cfg(windows)]
    assert_eq!(
        super::windows::modes(),
        before,
        "native console modes leaked"
    );
    assert!(
        !terminal::is_raw_mode_enabled().unwrap(),
        "raw mode remained enabled"
    );
    if case == "panic" {
        assert!(result.is_err());
    } else if case == "error" {
        assert!(result.unwrap().is_err());
    } else {
        result.unwrap().unwrap();
    }
    println!("NATIVE: terminal mode fields restored");
    println!("CLEAN: raw mode off");
}

#[test]
fn catalog_terminal_pty_native_cleanup_keys_resize_no_color() {
    for (case, keys) in [
        ("normal", &b"\r"[..]),
        ("back", &b"\x1b"[..]),
        ("ctrl-c", &b"\x03"[..]),
        ("eof", &b"\x04"[..]),
        ("resize", &b"\x03"[..]),
        ("shrink", &b"\x03"[..]),
        ("error", &b"\r"[..]),
        ("panic", &b"\r"[..]),
        ("no-color", &b"\x03"[..]),
        ("no-color-env", &b"\x03"[..]),
    ] {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args(["catalog_terminal_pty_child", "--ignored", "--nocapture"])
            .env("MAESTRO_TERMINAL_CASE", case)
            .env("TERM", "xterm-256color")
            .env_remove("NO_COLOR");
        if case == "no-color-env" {
            command.env("NO_COLOR", "1");
        }
        let mut pty = Pty::spawn(&mut command);
        pty.until("READY");
        if case == "resize" {
            pty.resize();
            pty.until(&"─".repeat(98));
        }
        if case == "shrink" {
            pty.resize_to((40, 12));
            pty.send(b"\r");
            pty.until("Resize to at least");
        }
        pty.send(keys);
        pty.until("NATIVE: terminal mode fields restored");
        pty.until("CLEAN: raw mode off");
        assert!(
            pty.finish(),
            "{case}: {}",
            String::from_utf8_lossy(&pty.bytes)
        );
        let text = String::from_utf8_lossy(&pty.bytes);
        for sequence in ["\x1b[?1049h", "\x1b[?1049l", "\x1b[?25h"] {
            assert!(
                text.contains(sequence),
                "{case}: missing {sequence:?}: {text}"
            );
        }
        if case == "no-color" {
            assert!(!text.contains("38;2;") && !text.contains("48;2;"), "{text}");
        }
        println!("PASS native PTY {case}: parser + raw mode + alternate screen + cursor cleanup");
    }
}

#[test]
fn catalog_terminal_pty_init_and_config_cancel_without_writes() {
    let home = scratch_directory().unwrap();
    let root = home.join("project");
    fs::create_dir(&root).unwrap();
    let binary = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "maestro.exe"
        } else {
            "maestro"
        });
    assert!(
        binary.is_file(),
        "cargo test must build the CLI binary: {}",
        binary.display()
    );
    for (args, marker) in [
        (&["init"][..], "Reviewed catalog"),
        (&["config"][..], "KEY=VALUE"),
    ] {
        let mut command = Command::new(&binary);
        command
            .args(args)
            .current_dir(&root)
            .env("TERM", "xterm-256color")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"));
        let mut pty = Pty::spawn(&mut command);
        pty.until(marker);
        pty.send(b"\x03");
        assert!(pty.finish(), "{}", String::from_utf8_lossy(&pty.bytes));
        let text = String::from_utf8_lossy(&pty.bytes);
        assert!(
            text.contains("\x1b[?1049l") && text.contains("\x1b[?25h"),
            "{text}"
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert!(!home.join("config/maestro/preferences.toml").exists());
        assert!(!home.join("data/maestro/kernel.sqlite3").exists());
    }
}

#[test]
fn catalog_terminal_pty_renderer_plain_script_exact_plan_parity() {
    let home = scratch_directory().unwrap();
    let root = home.join("project");
    fs::create_dir(&root).unwrap();
    let working = env::current_dir().unwrap();
    let executable = env::current_exe().unwrap();
    let catalog = working
        .ancestors()
        .chain(executable.ancestors())
        .map(|base| base.join("tests/fixtures/catalog/bootstrap/owner-local"))
        .find(|path| path.is_dir())
        .expect("public catalog fixture in the workspace");
    let mut command = Command::new(&executable);
    command
        .env("MAESTRO_TERMINAL_CATALOG", catalog)
        .args([
            "catalog_terminal_plan_parity_child",
            "--ignored",
            "--nocapture",
        ])
        .current_dir(&root)
        .env("TERM", "xterm-256color")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"));
    let mut pty = Pty::spawn(&mut command);
    pty.until("Reviewed catalog");
    pty.send(b"\r\ry\ren\rbrief\rupdates=off\r\rpreview\r");
    pty.until("PARITY: renderer/plain/script exact plan bytes");
    assert!(pty.finish(), "{}", String::from_utf8_lossy(&pty.bytes));
    let text = String::from_utf8_lossy(&pty.bytes);
    assert!(
        text.contains("\x1b[?1049l") && text.contains("\x1b[?25h"),
        "{text}"
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn catalog_terminal_pty_plain_flags_dumb_small_and_no_color() {
    let home = scratch_directory().unwrap();
    let root = home.join("project");
    fs::create_dir(&root).unwrap();
    let binary = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "maestro.exe"
        } else {
            "maestro"
        });
    for (args, term, geometry, renderer, no_color, marker) in [
        (
            &["init", "--plain"][..],
            "xterm",
            (80, 24),
            false,
            false,
            "Reviewed catalog",
        ),
        (
            &["config", "--plain"][..],
            "xterm",
            (80, 24),
            false,
            false,
            "KEY=VALUE",
        ),
        (&["config"][..], "dumb", (80, 24), false, false, "KEY=VALUE"),
        (
            &["config"][..],
            "xterm",
            (79, 23),
            false,
            false,
            "KEY=VALUE",
        ),
        (
            &["--no-color", "config"][..],
            "xterm",
            (80, 24),
            true,
            false,
            "KEY=VALUE",
        ),
        (&["config"][..], "xterm", (80, 24), true, true, "KEY=VALUE"),
        (
            &["config"][..],
            "xterm-mono",
            (80, 24),
            true,
            false,
            "KEY=VALUE",
        ),
        (&["config"][..], "vt100", (80, 24), true, false, "KEY=VALUE"),
        (
            &["config"][..],
            "unknown",
            (80, 24),
            true,
            false,
            "KEY=VALUE",
        ),
    ] {
        let mut command = Command::new(&binary);
        command
            .args(args)
            .current_dir(&root)
            .env("TERM", term)
            .env_remove("NO_COLOR")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"));
        if no_color {
            command.env("NO_COLOR", "1");
        }
        let mut pty = Pty::spawn_size(&mut command, geometry);
        pty.until(marker);
        pty.send(if renderer {
            &b"\x03"[..]
        } else {
            &b"cancel\r"[..]
        });
        assert!(pty.finish());
        let text = String::from_utf8_lossy(&pty.bytes);
        assert_eq!(text.contains("\x1b[?1049h"), renderer, "{args:?}: {text}");
        assert!(!text.contains("38;2;") && !text.contains("48;2;"), "{text}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}
