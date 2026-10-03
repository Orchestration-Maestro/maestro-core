//! Native `ConPTY` qualification, never redirected pipes posing as a console.
//! C05k PTY receipts only: production code must never use this module.
#![expect(
    unsafe_code,
    reason = "test-only Win32 `ConPTY` boundary owns every handle and API buffer"
)]
use super::stream;
use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs::File,
    io::{self, Write},
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    process::Command,
    ptr::{null, null_mut},
    sync::mpsc::Receiver,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::{
        Console::{
            COORD, ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode, GetStdHandle, HPCON,
            ResizePseudoConsole, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        },
        Pipes::CreatePipe,
        Threading::{
            CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
            EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION,
            STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};

/// A pseudo console has separate incoming/outgoing pipes, not standard redirects.
pub(super) struct Pty {
    console: HPCON,
    process: HANDLE,
    input: File,
    output: Receiver<Vec<u8>>,
    pub(super) bytes: Vec<u8>,
}

/// Windows strings include a terminator and retain native UTF-16 paths.
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}

/// Create pipes and immediately wrap successful handles in owning Files.
fn pipe() -> (File, File) {
    let mut read = null_mut();
    let mut write = null_mut();
    // SAFETY: API writes two handles into valid locals; Files take ownership once.
    unsafe {
        assert_ne!(CreatePipe(&raw mut read, &raw mut write, null(), 0), 0);
        (File::from_raw_handle(read), File::from_raw_handle(write))
    }
}

/// Mirror Command's explicit environment overrides for the isolated scratch home.
fn environment(command: &Command) -> Vec<u16> {
    let mut variables: BTreeMap<OsString, OsString> = env::vars_os().collect();
    for (name, value) in command.get_envs() {
        if let Some(value) = value {
            variables.insert(name.to_owned(), value.to_owned());
        } else {
            variables.remove(name);
        }
    }
    let mut result = Vec::new();
    for (name, value) in variables {
        let mut assignment = name;
        assignment.push("=");
        assignment.push(value);
        result.extend(wide(&assignment));
    }
    result.push(0);
    result
}

/// Spawn with the documented pseudo-console process attribute, not inherited stdio.
fn spawn(command: &Command, console: HPCON) -> HANDLE {
    let application = wide(command.get_program());
    // Test arguments have no embedded quotes; quote each path/argument for spaces.
    let mut line = format!("\"{}\"", command.get_program().to_string_lossy());
    for argument in command.get_args() {
        assert!(!argument.to_string_lossy().contains('"'));
        write!(line, " \"{}\"", argument.to_string_lossy()).unwrap();
    }
    let mut line = wide(OsStr::new(&line));
    let environment = environment(command);
    let directory = command.get_current_dir().map(|path| wide(path.as_os_str()));
    let mut size = 0;
    // SAFETY: all API buffers live through CreateProcess; attribute storage is aligned.
    unsafe {
        InitializeProcThreadAttributeList(null_mut(), 1, 0, &raw mut size);
        let mut storage = vec![0_usize; size.div_ceil(size_of::<usize>())];
        let list: LPPROC_THREAD_ATTRIBUTE_LIST = storage.as_mut_ptr().cast();
        assert_ne!(
            InitializeProcThreadAttributeList(list, 1, 0, &raw mut size),
            0
        );
        assert_ne!(
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                console as *const _,
                size_of::<HPCON>(),
                null_mut(),
                null()
            ),
            0
        );
        let startup = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap(),
                // Do not inherit WSL/CI redirected handles into the pseudo console.
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: null_mut(),
                hStdOutput: null_mut(),
                hStdError: null_mut(),
                ..Default::default()
            },
            lpAttributeList: list,
        };
        let mut process = PROCESS_INFORMATION::default();
        let result = CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            directory.as_ref().map_or(null(), Vec::as_ptr),
            &raw const startup.StartupInfo,
            &raw mut process,
        );
        DeleteProcThreadAttributeList(list);
        assert_ne!(result, 0, "CreateProcessW: {}", io::Error::last_os_error());
        CloseHandle(process.hThread);
        process.hProcess
    }
}
impl Pty {
    /// Allocate a native 80×24 pseudo console and concurrently drain its output.
    pub(super) fn spawn(command: &mut Command) -> Self {
        Self::spawn_size(command, (80, 24))
    }
    /// Initial geometry tests D6's plain fallback before raw mode starts.
    pub(super) fn spawn_size(command: &mut Command, (width, height): (u16, u16)) -> Self {
        let (input_read, input) = pipe();
        let (output_read, output_write) = pipe();
        let mut console = 0;
        // SAFETY: pipe handles and console output pointer remain valid during the call.
        unsafe {
            assert_eq!(
                CreatePseudoConsole(
                    COORD {
                        X: i16::try_from(width).unwrap(),
                        Y: i16::try_from(height).unwrap()
                    },
                    input_read.as_raw_handle(),
                    output_write.as_raw_handle(),
                    0,
                    &raw mut console
                ),
                0
            );
        }
        let output = stream::output(output_read);
        let process = spawn(command, console);
        Self {
            console,
            process,
            input,
            output,
            bytes: Vec::new(),
        }
    }
    /// Use the existing CLI test deadline, not a product timeout.
    pub(super) fn until(&mut self, marker: &str) {
        stream::until(&self.output, &mut self.bytes, marker);
    }
    /// Inject bytes through `ConPTY`'s native keyboard conversion.
    pub(super) fn send(&mut self, keys: &[u8]) {
        self.input.write_all(keys).unwrap();
    }
    /// Native console resize reaches crossterm's Windows event reader.
    pub(super) fn resize(&self) {
        self.resize_to((100, 30));
    }
    /// Shrink/grow exercises the paused-submission boundary without replacing the draft.
    pub(super) fn resize_to(&self, (width, height): (u16, u16)) {
        // SAFETY: this object owns a live pseudo-console handle.
        unsafe {
            assert_eq!(
                ResizePseudoConsole(
                    self.console,
                    COORD {
                        X: i16::try_from(width).unwrap(),
                        Y: i16::try_from(height).unwrap()
                    }
                ),
                0
            );
        }
    }
    /// Child-side cleanup marker includes the raw-mode query before it exits.
    pub(super) fn finish(&mut self) -> bool {
        // SAFETY: the owned process remains live until Drop, and code is a valid output.
        unsafe {
            assert_eq!(WaitForSingleObject(self.process, 60_000), WAIT_OBJECT_0);
            let mut code = 0;
            assert_ne!(GetExitCodeProcess(self.process, &raw mut code), 0);
            stream::drain(&self.output, &mut self.bytes);
            code == 0
        }
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        // SAFETY: each owned process/console handle is released exactly once.
        unsafe {
            TerminateProcess(self.process, 1);
            CloseHandle(self.process);
            ClosePseudoConsole(self.console);
        }
    }
}

/// Query actual console flags, not crossterm's cached raw-mode bookkeeping.
pub(super) fn modes() -> [u32; 3] {
    [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].map(|kind| {
        let mut mode = 0;
        // SAFETY: standard handles belong to the child console; API only reads them.
        unsafe {
            assert_ne!(GetConsoleMode(GetStdHandle(kind), &raw mut mode), 0);
        }
        mode
    })
}
