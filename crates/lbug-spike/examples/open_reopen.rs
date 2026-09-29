//! G26's measurement of the embedded graph on disk: create a scratch
//! database of 1,000 nodes, then open it read-only, run `RETURN 1`, close
//! it, and do both again. It prints each time, the database's size on disk
//! and, on Linux, the process's peak RSS. Run it on an empty scratch
//! directory:
//! `cargo run -p lbug-spike --features engine --example open_reopen -- <directory>`.

use lbug::{Connection, Database, SystemConfig};
use std::{env, error::Error, fs, path::Path, time::Instant};

/// How many nodes the scratch database holds.
const NODES: usize = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let directory = env::args_os()
        .nth(1)
        .ok_or("usage: open_reopen <empty scratch directory>")?;
    let directory = Path::new(&directory);
    let database = directory.join("graph.lbug");
    let started = Instant::now();
    {
        let writer = Database::new(&database, SystemConfig::default())?;
        let connection = Connection::new(&writer)?;
        connection.query("CREATE NODE TABLE Entity(id STRING, PRIMARY KEY(id));")?;
        for node in 0..NODES {
            connection.query(&format!("CREATE (:Entity {{id: 'e{node}'}});"))?;
        }
    }
    println!("create {NODES} nodes and close: {:?}", started.elapsed());
    for label in ["open", "reopen"] {
        let started = Instant::now();
        let reader = Database::new(&database, SystemConfig::default().read_only(true))?;
        let opened = started.elapsed();
        let connection = Connection::new(&reader)?;
        let answer = connection.query("RETURN 1;")?;
        println!(
            "{label} read-only: {opened:?}; with RETURN 1: {:?}; {answer}",
            started.elapsed()
        );
    }
    let mut bytes = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let size = entry.metadata()?.len();
        println!("file {}: {size} bytes", entry.file_name().display());
        bytes += size;
    }
    println!("disk: {bytes} bytes");
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines().filter(|line| line.starts_with("VmHWM")) {
            println!("peak RSS {line}");
        }
    }
    Ok(())
}
