//! Inspect a `Ev_Ap.db` file without touching it.
//!
//! Used to verify that the Rust build can open the database written by the
//! Python app (the "opens without migration" requirement from Phase 7).
//!
//! ```text
//! cargo run --example inspect_db -- path/to/Ev_Ap.db
//! cargo run --example inspect_db -- path/to/Ev_Ap.db --init   # also runs the points migration
//! ```

use std::env;
use std::path::Path;

use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::Backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "Ev_Ap.db".to_string());
    let init = args.any(|arg| arg == "--init");

    let db = SqliteBackend::with_path(Path::new(&path))?;
    println!("Database : {}", db.path().display());
    println!("Exists   : {}", db.db_exists());

    if init {
        db.initialize_db()?;
        println!("Initialized (points column migrated if needed)");
    }

    let tables = db.list_tables()?;
    println!("Events   : {tables:?}");

    for table in &tables {
        let rows = db.fetch_table_data(table)?;
        println!("\n== {table} ({} rows) ==", rows.len());
        for row in rows.iter().take(5) {
            let cells: Vec<String> = row
                .cells
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect();
            println!("   {}", cells.join(" | "));
        }
        if rows.len() > 5 {
            println!("   ... {} more", rows.len() - 5);
        }
    }

    let members = db.fetch_table_data("Members")?;
    println!("\n== Members ({} rows) ==", members.len());
    for row in members.iter().take(5) {
        let cells: Vec<String> = row
            .cells
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        println!("   {}", cells.join(" | "));
    }

    Ok(())
}
