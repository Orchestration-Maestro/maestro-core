//! Independently authored hostile public fixture. Never uses private material.
use std::{
    env, fs,
    io::{self, BufRead as _, Write as _},
    net::{TcpListener, ToSocketAddrs as _, UdpSocket},
    os::unix::fs::PermissionsExt as _,
    process::Command,
};

/// An actual parser must obtain preflight acknowledgement before emitting output.
fn emit(data: &str, entities: u64) -> io::Result<()> {
    println!(
        concat!(
            r#"{{"kind":"decode","value":{{"stage":"attachment","input_bytes":100,"#,
            r#""expanded_bytes":{},"levels":1,"members":1,"entities":{},"pixels":0,"#,
            r#""memory_bytes":{}}}}}"#,
        ),
        data.len(),
        entities,
        data.len()
    );
    io::stdout().flush()?;
    let mut ack = String::new();
    io::stdin().lock().read_line(&mut ack)?;
    if ack != "OK\n" {
        return Err(io::Error::other("decode refused"));
    }
    println!(r#"{{"kind":"data","value":"{data}"}}"#);
    Ok(())
}
/// Create a downloaded/produced executable: noexec and Landlock must deny it.
fn executable_denied() -> io::Result<bool> {
    fs::copy("/parser", "/work/planted")?;
    fs::set_permissions("/work/planted", fs::Permissions::from_mode(0o755))?;
    Ok(Command::new("/work/planted").arg("deny").status().is_err()
        && Command::new("/input/document")
            .arg("deny")
            .status()
            .is_err())
}
/// Deliberately attempt prohibited effects, not just report namespace identities.
fn main() -> io::Result<()> {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "deny" => {
            let host = env::args().nth(2).unwrap_or_else(|| "/host-canary".into());
            let denied = TcpListener::bind("127.0.0.1:0").is_err()
                && UdpSocket::bind("127.0.0.1:0").is_err()
                && "example.invalid:80".to_socket_addrs().is_err()
                && fs::read(host).is_err()
                && fs::read("/proc/1/root/host-canary").is_err()
                && fs::read("/sys/fs/cgroup/cgroup.procs").is_err()
                && fs::read("/run/user/1000/bus").is_err()
                && fs::write("/input/document", "modified").is_err()
                && executable_denied()?;
            emit(if denied { "DENIED" } else { "ESCAPED" }, 0)?;
            println!(r#"{{"kind":"done"}}"#);
        }
        "closure" => {
            let reexec = Command::new("/parser").arg("leaf").status()?.success();
            let denied = fs::write("/parser", "replace").is_err()
                && fs::rename("/parser", "/work/replaced").is_err()
                && fs::copy("/input/document", "/parser").is_err()
                && executable_denied()?
                && Command::new("/bin/sh")
                    .arg("-c")
                    .arg("exit 0")
                    .status()
                    .is_err();
            emit(
                if reexec && denied {
                    "CLOSURE_DENIED"
                } else {
                    "CLOSURE_ESCAPED"
                },
                0,
            )?;
            println!(r#"{{"kind":"done"}}"#);
        }
        "crash" => std::process::abort(),
        "cpu" => loop {
            std::hint::black_box(123_u64.wrapping_mul(456));
        },
        "memory" => {
            let mut allocations = Vec::new();
            loop {
                allocations.push(vec![42_u8; 1024 * 1024]);
                std::hint::black_box(&allocations);
            }
        }
        "fork" => {
            let mut children = Vec::new();
            for _ in 0..64 {
                if let Ok(child) = Command::new("/parser").arg("sleep").spawn() {
                    children.push(child);
                }
            }
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        "sleep" => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
        "flood" => loop {
            print!("{}", "X".repeat(1024));
            io::stdout().flush()?;
        },
        "decode" | "entities" => {
            emit(&"a".repeat(60), u64::from(mode == "entities"))?;
            emit(&"a".repeat(60), 0)?;
            println!(r#"{{"kind":"done"}}"#);
        }
        "loader" => {
            fs::copy("/input/library", "/work/planted.so")?;
            let status = Command::new("/parser")
                .arg("leaf")
                .env("LD_PRELOAD", "/work/planted.so")
                .env("LD_LIBRARY_PATH", "/work")
                .status()?;
            fs::copy("/parser", "/work/planted")?;
            let direct = Command::new("/lib64/ld-linux-x86-64.so.2")
                .arg("/work/planted")
                .arg("leaf")
                .status()?;
            emit(
                if status.success() && !fs::exists("/work/library-executed")? && !direct.success() {
                    "LOADER_DENIED"
                } else {
                    "LOADER_ESCAPED"
                },
                0,
            )?;
            println!(r#"{{"kind":"done"}}"#);
        }
        "unadmitted" => println!(r#"{{"kind":"data","value":"unadmitted"}}"#),
        "shape" => println!(r#"{{"kind":"done","unexpected":1}}"#),
        "extra" => {
            emit("admitted", 0)?;
            println!(r#"{{"kind":"done"}}"#);
            println!(r#"{{"kind":"data","value":"extra"}}"#);
        }
        "truncated" => print!(r#"{{"kind":"done""#),
        "leaf" => {}
        _ => return Err(io::Error::other("unknown hostile mode")),
    }
    Ok(())
}
