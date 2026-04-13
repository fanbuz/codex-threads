use rusqlite::{Connection, Result};

pub fn init_schema(conn: &Connection) -> Result<bool> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            session_id TEXT,
            modified_at INTEGER NOT NULL,
            size INTEGER NOT NULL,
            synced_at TEXT NOT NULL,
            tail_record TEXT
        );

        CREATE TABLE IF NOT EXISTS threads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            folder TEXT,
            cwd TEXT,
            title TEXT NOT NULL,
            started_at TEXT,
            ended_at TEXT,
            message_count INTEGER NOT NULL,
            event_count INTEGER NOT NULL,
            aggregate_text TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_threads_session_id ON threads(session_id);
        CREATE INDEX IF NOT EXISTS idx_threads_title ON threads(title);

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            idx INTEGER NOT NULL,
            timestamp TEXT,
            role TEXT NOT NULL,
            text TEXT NOT NULL,
            raw_json TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id, idx);

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            idx INTEGER NOT NULL,
            timestamp TEXT,
            event_type TEXT NOT NULL,
            summary TEXT NOT NULL,
            raw_json TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_events_session_id ON events(session_id, idx);
        "#,
    )?;
    ensure_column(conn, "files", "tail_record", "TEXT")?;

    let fts = conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS threads_fts
        USING fts5(session_id UNINDEXED, title, cwd, path, aggregate_text);

        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
        USING fts5(session_id UNINDEXED, role UNINDEXED, text);

        CREATE VIRTUAL TABLE IF NOT EXISTS events_fts
        USING fts5(session_id UNINDEXED, event_type, summary);
        "#,
    );

    if fts.is_ok() {
        Ok(true)
    } else {
        let _ = conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS threads_fts;
            DROP TABLE IF EXISTS messages_fts;
            DROP TABLE IF EXISTS events_fts;
            "#,
        );
        Ok(false)
    }
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let existing = row.get::<_, String>(1)?;
        if existing == column {
            return Ok(());
        }
    }

    let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    conn.execute(&alter, [])?;
    Ok(())
}
