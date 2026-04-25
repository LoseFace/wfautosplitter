use rusqlite::Connection;
use std::path::PathBuf;

pub fn init_db() -> Connection {
    let mut path = dirs::document_dir().unwrap_or(PathBuf::from("."));
    path.push("wfspeedrun.db");

    let conn = Connection::open(path).expect("failed to open database");

    let schema = include_str!("schema.sql");

    conn.execute_batch(&schema).expect("failed to apply schema");

    conn
}
