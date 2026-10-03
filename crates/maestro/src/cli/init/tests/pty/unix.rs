//! Safe rustix PTY operations; only registering `pre_exec` is unsafe.
//! C05k PTY receipts only: production code must never use this module.
use super::stream;
use rustix::{
    fs::{Mode, OFlags, open},
    process::{ioctl_tiocsctty, setsid},
    pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
    stdio,
    termios::{Termios, Winsize, tcgetattr, tcsetwinsize},
};
use std::{
    fs::File,
    io::{self, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    sync::mpsc::Receiver,
    thread,
    time::{Duration, Instant},
};

/// Parent retains the slave to compare kernel terminal modes after every exit.
pub(super) struct Pty {
    child: Child,
    master: File,
    slave: File,
    #[cfg(not(target_os = "macos"))]
    before: String,
    output: Receiver<Vec<u8>>,
    pub(super) bytes: Vec<u8>,
}
impl Pty {
    /// Allocate a controlling terminal at D6's minimum supported geometry.
    pub(super) fn spawn(command: &mut Command) -> Self {
        Self::spawn_size(command, (80, 24))
    }
    /// Initial geometry tests D6's plain fallback before raw mode starts.
    #[expect(
        unsafe_code,
        reason = "pre_exec registers only async-signal-safe setsid and TIOCSCTTY"
    )]
    pub(super) fn spawn_size(command: &mut Command, (width, height): (u16, u16)) -> Self {
        let master = File::from(openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap());
        grantpt(&master).unwrap();
        unlockpt(&master).unwrap();
        let name = ptsname(&master, Vec::new()).unwrap();
        let slave = File::from(
            open(
                name.as_c_str(),
                OFlags::RDWR | OFlags::NOCTTY,
                Mode::empty(),
            )
            .unwrap(),
        );
        tcsetwinsize(
            &slave,
            Winsize {
                ws_row: height,
                ws_col: width,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .unwrap();
        #[cfg(not(target_os = "macos"))]
        let before = format!("{:?}", tcgetattr(&slave).unwrap());
        let control = slave.try_clone().unwrap();
        command
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        // SAFETY: the closure performs only the two async-signal-safe syscalls.
        unsafe {
            command.pre_exec(move || {
                setsid().map_err(io::Error::from)?;
                ioctl_tiocsctty(&control).map_err(io::Error::from)
            });
        }
        let child = command.spawn().unwrap();
        let output = stream::output(master.try_clone().unwrap());
        Self {
            child,
            master,
            slave,
            #[cfg(not(target_os = "macos"))]
            before,
            output,
            bytes: Vec::new(),
        }
    }
    /// Bound test waiting, not a product timeout (matches existing CLI test deadline).
    pub(super) fn until(&mut self, marker: &str) {
        stream::until(&self.output, &mut self.bytes, marker);
    }
    /// Inject real bytes into the backend parser.
    pub(super) fn send(&mut self, keys: &[u8]) {
        self.master.write_all(keys).unwrap();
    }
    /// A real resize ioctl triggers the controlling foreground process group's SIGWINCH.
    pub(super) fn resize(&self) {
        self.resize_to((100, 30));
    }
    /// Shrink/grow exercises the paused-submission boundary without replacing the draft.
    pub(super) fn resize_to(&self, (width, height): (u16, u16)) {
        tcsetwinsize(
            &self.slave,
            Winsize {
                ws_row: height,
                ws_col: width,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .unwrap();
    }
    /// Wait for completion and prove raw mode did not leak into the parent terminal.
    pub(super) fn finish(&mut self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "PTY child did not exit");
            thread::sleep(Duration::from_millis(10));
        };
        stream::drain(&self.output, &mut self.bytes);
        // macOS revokes the slave when its session leader exits (run 37146227096).
        // Every Unix child compares full native modes before that revoke instead.
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            format!("{:?}", tcgetattr(&self.slave).unwrap()),
            self.before,
            "terminal mode leaked"
        );
        status.success()
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
    }
}

/// Snapshot the child's real kernel modes, never the renderer's cached raw-mode flag.
pub(super) fn modes() -> Termios {
    tcgetattr(stdio::stdin()).unwrap()
}

/// Name each native field if restoration failed; all control characters are included.
pub(super) fn assert_modes(before: &Termios) {
    let after = modes();
    assert_eq!(
        after.input_modes, before.input_modes,
        "input terminal modes leaked"
    );
    assert_eq!(
        after.output_modes, before.output_modes,
        "output terminal modes leaked"
    );
    assert_eq!(
        after.control_modes, before.control_modes,
        "control terminal modes leaked"
    );
    assert_eq!(
        after.local_modes, before.local_modes,
        "local terminal modes leaked"
    );
    assert_eq!(
        format!("{:?}", after.special_codes),
        format!("{:?}", before.special_codes),
        "terminal control characters leaked"
    );
    assert_eq!(
        after.input_speed(),
        before.input_speed(),
        "input speed changed"
    );
    assert_eq!(
        after.output_speed(),
        before.output_speed(),
        "output speed changed"
    );
}
