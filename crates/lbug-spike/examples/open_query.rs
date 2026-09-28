//! The smallest program that links `LadybugDB`: open an in-memory database,
//! run one query, print the result. Its release size is the G25 size probe.

use lbug::{Connection, Database, SystemConfig};

fn main() -> Result<(), lbug::Error> {
    let db = Database::in_memory(SystemConfig::default())?;
    let conn = Connection::new(&db)?;
    let result = conn.query("RETURN 1 + 1;")?;
    println!("{result}");
    Ok(())
}
