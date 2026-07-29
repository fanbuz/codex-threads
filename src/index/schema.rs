use rusqlite::{Connection, OptionalExtension, Result};

pub const INDEX_FORMAT_VERSION: i64 = 2;

#[derive(Debug, Clone, Copy)]
pub struct SchemaState {
    pub fts_available: bool,
    pub format_version: i64,
}

pub fn init_schema(conn: &Connection) -> Result<SchemaState> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

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

    let stored_version = read_meta_i64(conn, "format_version")?;
    let legacy_fts = has_legacy_fts_schema(conn)?;
    let needs_migration = stored_version.unwrap_or(1) < INDEX_FORMAT_VERSION || legacy_fts;
    let had_indexed_data = count_rows(conn, "threads")? > 0
        || count_rows(conn, "messages")? > 0
        || count_rows(conn, "events")? > 0;

    if needs_migration {
        drop_fts_tables(conn)?;
        if had_indexed_data {
            conn.execute_batch(
                r#"
                DELETE FROM files;
                DELETE FROM messages;
                DELETE FROM events;
                DELETE FROM threads;
                "#,
            )?;
            write_meta(conn, "rebuild_required", "1")?;
        }
    }

    let fts = conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS threads_fts
        USING fts5(
            session_id UNINDEXED,
            title,
            cwd,
            path,
            aggregate_text,
            content='',
            contentless_delete=1
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
        USING fts5(
            session_id UNINDEXED,
            role UNINDEXED,
            text,
            content='',
            contentless_delete=1
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS events_fts
        USING fts5(
            session_id UNINDEXED,
            event_type,
            summary,
            content='',
            contentless_delete=1
        );
        "#,
    );

    let fts_available = if fts.is_ok() {
        true
    } else {
        let _ = drop_fts_tables(conn);
        false
    };

    write_meta(conn, "format_version", &INDEX_FORMAT_VERSION.to_string())?;
    if read_meta(conn, "rebuild_required")?.is_none() {
        write_meta(conn, "rebuild_required", "0")?;
    }

    if needs_migration && had_indexed_data {
        conn.execute_batch("VACUUM;")?;
    }

    Ok(SchemaState {
        fts_available,
        format_version: INDEX_FORMAT_VERSION,
    })
}

pub fn read_rebuild_required(conn: &Connection) -> Result<bool> {
    Ok(read_meta(conn, "rebuild_required")?.as_deref() == Some("1"))
}

pub fn write_rebuild_required(conn: &Connection, required: bool) -> Result<()> {
    write_meta(conn, "rebuild_required", if required { "1" } else { "0" })
}

fn read_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM index_meta WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

fn read_meta_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(read_meta(conn, key)?.and_then(|value| value.parse().ok()))
}

fn write_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO index_meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn count_rows(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
}

fn has_legacy_fts_schema(conn: &Connection) -> Result<bool> {
    let sql = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'threads_fts'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(sql.is_some_and(|sql| !sql.contains("contentless_delete=1")))
}

fn drop_fts_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS threads_fts;
        DROP TABLE IF EXISTS messages_fts;
        DROP TABLE IF EXISTS events_fts;
        "#,
    )
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
