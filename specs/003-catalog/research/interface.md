# Init terminal dependency measurements (C05f)

## Decision and evidence boundary

OA9: the owner approved **ratatui plus crossterm on 2026-09-28**, under
ADR-0020. Approval is not dependency adoption or terminal qualification.
Measured 2026-09-30 against `b39ce3cb5d89b965c8fccd48f2086456870bac45`.
Only this note is committed: no production manifest, lock, parser or menu edit.
The plain init flow and preference parser remain dependency-free of these crates.

Supervisor ruling for unavailable hosts: measure static graphs for Linux,
Windows GNU/MSVC and macOS from Linux; run interactive Linux PTY cases.
**Windows and macOS interactive acceptance: BLOCKED, no native host.**
C05k must supply three-OS PTY tests (ConPTY on Windows), native cleanup evidence,
and adoption-time vet/DEP-001 registrations. Cross-Clippy is not host evidence.

The disposable `Interface` below models C05g's planned render/event seam with
semantic inputs, not backend types. C05g's shared flow is not landed in this
base: C05k must bind the adapter to that actual port, not introduce a second
flow. No planner, trust decision or file write lives in this probe.

## Minimum measured features

```toml
[package]
name = "c05f-terminal"
version = "0.0.0"
edition = "2024"
license = "MIT"

[workspace]

[dependencies]
ratatui = { version = "=0.30.2", default-features = false, features = [
  "crossterm",
] }
crossterm = { version = "=0.29.0", default-features = false }
```

The single ratatui `crossterm` feature enables `std` and its backend. No
calendar, macros, layout-cache, serde, termion, termwiz or async event-stream
feature is requested. Removing that feature fails `cargo check` (exit 101):
`unresolved import crossterm::event` and missing `CrosstermBackend`.

Important transitive floor: `ratatui-crossterm 0.1.2` defaults force crossterm
`default = [bracketed-paste, events, windows, derive-more]` and backend
`underline-color`. Directly requesting `events,windows` is redundant: removing
those direct requests preserves the effective feature graph and all PTY passes.
These defaults cannot be subtracted by the application's feature flags. This is
the minimum measured **umbrella-ratatui configuration**, not a claim that every
forced feature is necessary or that every older release/backend was compared.

## Platform results

Host: Rust 1.98.1, Linux 6.18.33.2 WSL2, x86_64, `TERM=xterm-256color`.
PTY injection exercises the real crossterm parser; focus escape injection does
not qualify a particular terminal emulator's ability to emit focus reports.

| Case | Linux PTY | Windows | macOS |
| --- | --- | --- | --- |
| Down-arrow keyboard input | PASS | BLOCKED: no host | BLOCKED: no host |
| Focus lost/gained | PASS | BLOCKED: no host | BLOCKED: no host |
| Real PTY resize 80×24 to 100×30 | PASS | BLOCKED: no host | BLOCKED: no host |
| Ctrl-C restores termios, cursor, focus, alternate screen | PASS | BLOCKED: no host | BLOCKED: no host |
| Injected loop error restores terminal | PASS | BLOCKED: no host | BLOCKED: no host |
| Non-terminal/plain exact output, zero escapes | PASS | BLOCKED: no host | BLOCKED: no host |
| `--no-color` and `NO_COLOR=1`, no palette-setting SGR | PASS | BLOCKED: no host | BLOCKED: no host |

Red boundary: running the harness before the provider binary existed exited 1,
`AssertionError: BLOCKED: terminal probe binary/provider absent`.
Green: four PTY sessions plus exact plain output, exit 0, in debug and release.
Cleanup covers normal cancellation and a returned error, not SIGKILL/power loss.
An initial harness had no controlling terminal; resize correctly failed because
SIGWINCH had no foreground process group. `setsid` plus `TIOCSCTTY` fixes that
harness boundary; no synthetic SIGWINCH replaces the resize ioctl.

Scratch `cargo clippy --all-targets --locked -- -D warnings` passed (exit 0)
for Linux, `x86_64-pc-windows-gnu` and `aarch64-apple-darwin`.
Windows GNU build exited 101: missing `x86_64-w64-mingw32-dlltool`.
Windows binary/time therefore not measured; no installation was attempted.
macOS binary/time not measured: no SDK/linker or native host. MSVC graph only.

## Dependency, licence and admission cost

Counts use unique package/version pairs from `cargo tree -e normal,build,dev`,
including workspace members and build/dev dependencies, not the metadata
resolver's inactive optional nodes. All base versions are preserved.

| Target | Base packages | Overlay packages | Added | Removed |
| --- | ---: | ---: | ---: | ---: |
| x86_64-unknown-linux-gnu | 189 | 223 | 34 | 0 |
| x86_64-pc-windows-gnu | 190 | 223 | 33 | 0 |
| x86_64-pc-windows-msvc | 190 | 222 | 32 | 0 |
| aarch64-apple-darwin | 192 | 225 | 33 | 0 |

The lock grows from 230 to 279 entries (+49), including inactive optional and
other-target packages. `cargo vet --locked` on the scratch workspace exits 255:
**48 dependencies lack safe-to-deploy**. The new `hashbrown 0.16.1` is already
covered by existing audits; no exemption/audit was added. Active union is 37
packages below; L = Linux, G = Windows GNU, W = Windows MSVC, M = macOS.
Licence expressions containing `/` are normalized to `OR` in this table.

| Added package | Version | Targets | Licence |
| --- | --- | --- | --- |
| castaway | 0.2.4 | L,G,W,M | MIT |
| compact_str | 0.9.1 | L,G,W,M | MIT |
| convert_case | 0.10.0 | L,G,W,M | MIT |
| crossterm | 0.29.0 | L,G,W,M | MIT |
| crossterm_winapi | 0.9.1 | G,W | MIT |
| darling | 0.24.1 | L,G,W,M | MIT |
| darling_core | 0.24.1 | L,G,W,M | MIT |
| darling_macro | 0.24.1 | L,G,W,M | MIT |
| derive_more | 2.1.1 | L,G,W,M | MIT |
| derive_more-impl | 2.1.1 | L,G,W,M | MIT |
| document-features | 0.2.12 | L,G,W,M | MIT OR Apache-2.0 |
| errno | 0.3.14 | L | MIT OR Apache-2.0 |
| foldhash | 0.2.0 | L,G,W,M | Zlib |
| hashbrown | 0.16.1 | L,G,W,M | MIT OR Apache-2.0 |
| indoc | 2.0.7 | L,G,W,M | MIT OR Apache-2.0 |
| instability | 0.3.14 | L,G,W,M | MIT |
| kasuari | 0.4.12 | L,G,W,M | MIT OR Apache-2.0 |
| line-clipping | 0.3.8 | L,G,W,M | MIT OR Apache-2.0 |
| litrs | 1.0.0 | L,G,W,M | MIT OR Apache-2.0 |
| lru | 0.18.5 | L,G,W,M | MIT |
| ratatui | 0.30.2 | L,G,W,M | MIT |
| ratatui-core | 0.1.2 | L,G,W,M | MIT |
| ratatui-crossterm | 0.1.2 | L,G,W,M | MIT |
| ratatui-widgets | 0.3.2 | L,G,W,M | MIT |
| rustc_version | 0.4.1 | L,G,W,M | MIT OR Apache-2.0 |
| rustversion | 1.0.23 | L,G,W,M | MIT OR Apache-2.0 |
| signal-hook | 0.3.18 | L,M | MIT OR Apache-2.0 |
| signal-hook-mio | 0.2.5 | L,M | MIT OR Apache-2.0 |
| signal-hook-registry | 1.4.8 | L,M | MIT OR Apache-2.0 |
| static_assertions | 1.1.0 | L,G,W,M | MIT OR Apache-2.0 |
| strum | 0.28.0 | L,G,W,M | MIT |
| strum_macros | 0.28.0 | L,G,W,M | MIT |
| unicode-segmentation | 1.13.3 | L,G,W,M | MIT OR Apache-2.0 |
| unicode-truncate | 2.0.1 | L,G,W,M | MIT OR Apache-2.0 |
| unicode-width | 0.2.2 | L,G,W,M | MIT OR Apache-2.0 |
| winapi | 0.3.9 | G,W | MIT OR Apache-2.0 |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | G | MIT OR Apache-2.0 |

Inactive/other-target new lock entries: approx 0.5.1, by_address 1.2.1,
deranged 0.5.8, libm 0.2.16, num-conv 0.2.2, num_threads 0.1.7,
palette/palette_derive/palette_math 0.7.7, portable-atomic 1.15.0,
powerfmt 0.2.0, time 0.3.55, time-core 0.1.9,
winapi-i686-pc-windows-gnu 0.4.0. errno/rustversion were already locked but
become active on the indicated targets; they are not among the 49 new entries.

`cargo tree -d` shows five new/expanded duplicate families on every target:

| Package | Existing versions | Added version | Source |
| --- | --- | --- | --- |
| darling | 0.20.11 | 0.24.1 | instability 0.3.14 vs qdrant-client's derive_builder |
| darling_core | 0.20.11 | 0.24.1 | darling |
| darling_macro | 0.20.11 | 0.24.1 | darling |
| foldhash | 0.1.5 | 0.2.0 | hashbrown 0.16.1/lru 0.18.5 |
| hashbrown | 0.15.5, 0.17.1 | 0.16.1 | lru 0.18.5 vs existing storage/HTTP graph |

Existing base64 0.22.1/0.23.1, getrandom 0.2.17/0.4.3 and syn 2.0.119/3.0.6
remain unchanged. No second crossterm version is forced. Adoption needs scoped
DEP-001 exceptions for the five families, with real convergence/removal
conditions; none are added here. Do not invent a future release number.

Organization-policy scratch licence/ban check exits 2:
`advisories ok, bans FAILED, licenses ok, sources ok`; its five errors are those
duplicate families. No new licence allowance is needed (MIT, Apache-2.0, Zlib).
All active added packages have `links = null`. Linux `readelf -d` finds the same
four dynamic dependencies before/after: libgcc_s.so.1, libm.so.6, libc.so.6,
ld-linux-x86-64.so.2. Windows uses OS console/kernel APIs through winapi;
this is not evidence of a successful Windows link. No new C build is required.

## Build time and binary size

Linux release, default release optimization, no LTO or stripping,
`CARGO_BUILD_JOBS=3`. Single observations on the shared host, not latency targets.
`time` runs inside `capped`, excluding build-slot waiting. Registry downloads
were complete before timing. Fresh target directories and
`capped env RUSTC_WRAPPER= ...` disable sccache for the small isolated comparison.
An empty wrapper set *outside* capped is insufficient: capped installs sccache.

| Isolated binary | Uncached wall time | Bytes |
| --- | ---: | ---: |
| Plain std-only baseline | 0.15 s | 457,968 |
| Terminal probe | 14.40 s | 877,720 |
| Added cost | **14.25 s** | **419,752** |

Actual maestro overlay retains reachable probe code via a scratch-only
`--c05f-probe` main dispatch. Base release is **30,553,616 bytes**; overlay is
**30,838,840 bytes**, delta **285,224 bytes (0.93%)**. This is probe code, not a
finished five-step menu's size. Shared target/sccache builds took base 158.09 s,
overlay first attempt 99.46 s (exit 101, rejected two `let _ = Result` cleanup
bindings), then corrected retry 49.89 s (exit 0). These asymmetric warm-cache
observations are not a valid clean maestro build-time delta; the isolated
uncached comparison above is the added compile-cost measurement.

## Reproduce in disposable scratch

Save the manifest above, the Rust source below as `terminal/src/main.rs`, and
the harness below as `probe.py` inside a scratch directory. Keep a std-only
`base` package with the same edition and `println!` plain text for cost comparison.
Use the recorded lockfiles in the lane evidence archive, or pin regenerated
entries to the versions in this note before comparing graphs.

```sh
export CARGO_BUILD_JOBS=3
CAPPED="$HOME/.local/bin/capped"
# Red before the binary exists: expected exit 1.
python3 probe.py terminal/target/debug/c05f-terminal
"$CAPPED" cargo build --manifest-path terminal/Cargo.toml
python3 probe.py terminal/target/debug/c05f-terminal
# Target names are the four rows in the dependency table.
for target in x86_64-unknown-linux-gnu x86_64-pc-windows-gnu \
  x86_64-pc-windows-msvc aarch64-apple-darwin; do
  "$CAPPED" cargo tree --manifest-path terminal/Cargo.toml \
    --locked --target "$target" -e features
  "$CAPPED" cargo tree --manifest-path terminal/Cargo.toml \
    --locked --target "$target" -d
 done
# Fresh target for each stage; time excludes queueing.
for stage in base terminal; do
  CARGO_TARGET_DIR="$PWD/uncached-$stage" "$CAPPED" env RUSTC_WRAPPER= \
    /usr/bin/time -f 'wall_s=%e user_s=%U sys_s=%S max_rss_kib=%M' \
    cargo build --manifest-path "$stage/Cargo.toml" --release --locked
 done
```

For workspace cost, `git archive` the recorded base into scratch. Add the two
measured declarations to `[workspace.dependencies]` and `name.workspace = true`
to maestro's dependencies in a second scratch snapshot. Run the same trees for
both snapshots, plus `-e normal,build,dev --prefix none --format '{p}'`; deduplicate
package/version lines to reproduce the counts. Metadata supplies licences and
`links`, **not active counts**. Run `cargo vet --locked` from the overlay root
with the unchanged supply-chain files and the common.md licence/ban recipe.

For actual binary delta, build the base with `-p maestro --release --locked`,
copy its executable, then use the overlay manifest/lock in the same scratch
workspace. Copy the probe to `crates/maestro/src/c05f_probe.rs`, make its `main`
`pub(crate)`, declare `mod c05f_probe;`, and add this branch before `cli::main()`:

```rust
if std::env::args().any(|arg| arg == "--c05f-probe") {
    return if c05f_probe::main().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
}
```

Rebuild and compare `stat -c '%s'` and `readelf -d`; do not merge this dispatch.
The ledger report retains exact commands, raw logs, locks and source hashes.

### Disposable Rust adapter

```rust
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend, style::{Color, Style},
    widgets::Paragraph, Terminal,
};
use std::{env, io::{self, IsTerminal, Write}, time::Duration};

trait Interface {
    fn draw(&mut self) -> io::Result<()>;
    fn next(&mut self) -> io::Result<Option<Input>>;
}
enum Input { Key, Focus(bool), Resize(u16, u16), Cancel }
struct Screen {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    color: bool,
}
struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        drop(disable_raw_mode());
        drop(execute!(io::stdout(), DisableFocusChange, Show, LeaveAlternateScreen));
    }
}
impl Interface for Screen {
    fn draw(&mut self) -> io::Result<()> {
        let style = if self.color {
            Style::default().fg(Color::Cyan)
        } else { Style::default() };
        self.terminal.draw(|frame| {
            frame.render_widget(Paragraph::new("Probe").style(style), frame.area());
        })?;
        Ok(())
    }
    fn next(&mut self) -> io::Result<Option<Input>> {
        if !event::poll(Duration::from_secs(5))? { return Ok(None); }
        Ok(match event::read()? {
            Event::Key(key) if key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL) =>
                Some(Input::Cancel),
            Event::Key(_) => Some(Input::Key),
            Event::FocusGained => Some(Input::Focus(true)),
            Event::FocusLost => Some(Input::Focus(false)),
            Event::Resize(width, height) => Some(Input::Resize(width, height)),
            _ => None,
        })
    }
}
fn marker(text: &str) -> io::Result<()> {
    write!(io::stdout(), "{text}\r\n")?;
    io::stdout().flush()
}
fn run(interface: &mut impl Interface, fail: bool) -> io::Result<()> {
    interface.draw()?;
    marker("READY")?;
    if fail { return Err(io::Error::other("injected draw-loop error")); }
    loop {
        match interface.next()? {
            Some(Input::Cancel) => return Ok(()),
            Some(Input::Key) => marker("KEY")?,
            Some(Input::Focus(true)) => marker("FOCUS")?,
            Some(Input::Focus(false)) => marker("BLUR")?,
            Some(Input::Resize(width, height)) => {
                interface.draw()?;
                marker(&format!("RESIZE {width} {height}"))?;
            }
            None => return Err(io::Error::new(io::ErrorKind::TimedOut, "no event")),
        }
    }
}
fn main() -> io::Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.iter().any(|arg| arg == "--plain")
        || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        println!("Probe: keyboard focus resize cleanup");
        return Ok(());
    }
    let cleanup = Cleanup;
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableFocusChange, Hide)?;
    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let color = !args.iter().any(|arg| arg == "--no-color") && env::var_os("NO_COLOR").is_none();
    let result = run(&mut Screen { terminal, color },
                     args.iter().any(|arg| arg == "--fail"));
    drop(cleanup);
    marker("CLEAN")?;
    result
}
```

### Linux PTY assertion harness

```python
import errno
import fcntl
import os
import pathlib
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time

binary = pathlib.Path(sys.argv[1]).resolve()
assert binary.is_file(), "BLOCKED: terminal probe binary/provider absent"
for args in ([], ["--plain"]):
    plain = subprocess.run([binary, *args],
                           check=True, capture_output=True).stdout
    assert plain == b"Probe: keyboard focus resize cleanup\n"
    assert b"\x1b" not in plain


def run_case(extra, env, failure=False):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    before = termios.tcgetattr(slave)

    def controlling_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    process = subprocess.Popen([binary, *extra], stdin=slave, stdout=slave,
                               stderr=slave, preexec_fn=controlling_terminal,
                               env={**os.environ, "TERM": "xterm-256color", **env})
    data = bytearray()

    def until(marker):
        deadline = time.monotonic() + 10
        while marker not in data and time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    data.extend(os.read(master, 65536))
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    break
        assert marker in data, (marker, bytes(data))

    try:
        until(b"READY")
        if not failure:
            os.write(master, b"\x1b[B")
            until(b"KEY")
            os.write(master, b"\x1b[O")
            until(b"BLUR")
            os.write(master, b"\x1b[I")
            until(b"FOCUS")
            fcntl.ioctl(slave, termios.TIOCSWINSZ,
                        struct.pack("HHHH", 30, 100, 0, 0))
            until(b"RESIZE 100 30")
            os.write(master, b"\x03")
        process.wait(timeout=10)
        until(b"CLEAN")
        assert process.returncode == (1 if failure else 0), bytes(data)
        assert termios.tcgetattr(slave) == before, "raw mode leaked"
        assert b"\x1b[?1049h" in data and b"\x1b[?1049l" in data
        assert b"\x1b[?1004h" in data and b"\x1b[?1004l" in data
        assert b"\x1b[?25h" in data, "cursor not restored"
        assert b"Probe" in data, "no rendered frame"
        colors = re.findall(rb"\x1b\[([0-9;]*)m", data)
        if extra == ["--no-color"] or "NO_COLOR" in env:
            assert all(value in (b"", b"0", b"39", b"49", b"39;49", b"59")
                       for value in colors), colors
        print("PASS", extra, env, "error" if failure else "Ctrl-C", len(data), "bytes")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


run_case([], {})
run_case(["--no-color"], {})
run_case([], {"NO_COLOR": "1"})
run_case(["--fail"], {}, failure=True)
print("PASS plain/non-terminal: exact bytes, no escapes; 4 PTY sessions")
```
