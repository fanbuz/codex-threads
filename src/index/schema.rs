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
            synced_at TEXT NOT NULL
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

    let fts = conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS threads_fts
        USING fts5(session_id UNINDEXED, title, cwd, path, aggregate_text);

        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
        USING fts5(session_id UNINDEXED, role UNINDEXED, text);
        "#,
    );

    if fts.is_ok() {
        Ok(true)
    } else {
        let _ = conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS threads_fts;
            DROP TABLE IF EXISTS messages_fts;
            "#,
        );
        Ok(false)
    }
}
