use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

use crate::output::excerpt;
use crate::parser::ParsedSession;

use super::schema::init_schema;

#[derive(Debug, Clone, Serialize)]
pub struct SyncStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub skipped_files: usize,
    pub failed_files: usize,
    pub removed_files: usize,
    pub threads: usize,
    pub messages: usize,
    pub events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub stats: SyncStats,
    pub partial: bool,
    pub failures: Vec<SyncFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusSummary {
    pub index_path: String,
    pub fts_available: bool,
    pub files: usize,
    pub threads: usize,
    pub messages: usize,
    pub events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadRecord {
    pub session_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub path: String,
    pub file_name: String,
    pub folder: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub message_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadSearchHit {
    pub session_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub path: String,
    pub message_count: usize,
    pub event_count: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub session_id: String,
    pub timestamp: Option<String>,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageSearchHit {
    pub session_id: String,
    pub title: Option<String>,
    pub timestamp: Option<String>,
    pub role: String,
    pub text: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub session_id: String,
    pub timestamp: Option<String>,
    pub event_type: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadRead {
    pub thread: ThreadRecord,
    pub messages: Vec<MessageRecord>,
}

#[derive(Debug, Clone)]
struct FileState {
    session_id: Option<String>,
    modified_at: i64,
    size: i64,
}

#[derive(Debug)]
pub struct Store {
    conn: Connection,
    index_path: PathBuf,
    fts_available: bool,
}

impl Store {
    pub fn open(index_dir: &Path) -> Result<Self> {
        fs::create_dir_all(index_dir)
            .with_context(|| format!("failed to create {}", index_dir.display()))?;
        let index_path = index_dir.join("threads.sqlite3");
        let conn = Connection::open(&index_path)
            .with_context(|| format!("failed to open {}", index_path.display()))?;
        let fts_available = init_schema(&conn)?;
        Ok(Self {
            conn,
            index_path,
            fts_available,
        })
    }

    pub fn sync_sessions(&mut self, sessions_dir: &Path) -> Result<SyncReport> {
        if !sessions_dir.exists() {
            bail!("会话目录不存在: {}", sessions_dir.display());
        }

        let existing = self.load_existing_files()?;
        let mut seen_paths = HashSet::new();
        let mut retained_session_ids = HashSet::new();
        let fts_available = self.fts_available;
        let mut stats = SyncStats {
            scanned_files: 0,
            indexed_files: 0,
            skipped_files: 0,
            failed_files: 0,
            removed_files: 0,
            threads: 0,
            messages: 0,
            events: 0,
        };
        let mut failures = Vec::new();

        let tx = self.conn.transaction()?;

        for entry in WalkDir::new(sessions_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }

            stats.scanned_files += 1;
            let path = entry.path().to_path_buf();
            let path_string = path.to_string_lossy().into_owned();
            seen_paths.insert(path_string.clone());

            let metadata = match fs::metadata(&path)
                .with_context(|| format!("failed to stat {}", path.display()))
            {
                Ok(metadata) => metadata,
                Err(error) => {
                    stats.failed_files += 1;
                    retain_previous_session_id(&existing, &path_string, &mut retained_session_ids);
                    failures.push(SyncFailure {
                        path: path_string,
                        error: error.to_string(),
                    });
                    continue;
                }
            };
            let modified_at = match metadata
                .modified()
                .and_then(|value| system_time_to_nanos(value).map_err(std::io::Error::other))
            {
                Ok(modified_at) => modified_at,
                Err(error) => {
                    stats.failed_files += 1;
                    retain_previous_session_id(&existing, &path_string, &mut retained_session_ids);
                    failures.push(SyncFailure {
                        path: path_string,
                        error: error.to_string(),
                    });
                    continue;
                }
            };
            let size = metadata.len() as i64;

            let is_unchanged = existing
                .get(&path_string)
                .map(|state| state.modified_at == modified_at && state.size == size)
                .unwrap_or(false);

            if is_unchanged {
                stats.skipped_files += 1;
                if let Some(session_id) = existing
                    .get(&path_string)
                    .and_then(|state| state.session_id.clone())
                {
                    retained_session_ids.insert(session_id);
                }
                continue;
            }

            let parsed = match crate::parser::parse_session_file(&path) {
                Ok(parsed) => parsed,
                Err(error) => {
                    stats.failed_files += 1;
                    retain_previous_session_id(&existing, &path_string, &mut retained_session_ids);
                    failures.push(SyncFailure {
                        path: path_string,
                        error: error.to_string(),
                    });
                    continue;
                }
            };
            retained_session_ids.insert(parsed.session_id.clone());
            let old_session_id = existing
                .get(&path_string)
                .and_then(|state| state.session_id.clone());
            replace_session(
                &tx,
                fts_available,
                &path,
                modified_at,
                size,
                old_session_id.as_deref(),
                &parsed,
            )?;
            stats.indexed_files += 1;
        }

        for (path, state) in existing {
            if seen_paths.contains(&path) {
                continue;
            }
            if let Some(session_id) = state.session_id {
                if !retained_session_ids.contains(&session_id) {
                    delete_session(&tx, fts_available, &session_id)?;
                }
            }
            tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
            stats.removed_files += 1;
        }

        tx.commit()?;

        let counts = self.count_totals()?;
        stats.threads = counts.0;
        stats.messages = counts.1;
        stats.events = counts.2;
        Ok(SyncReport {
            partial: !failures.is_empty(),
            stats,
            failures,
        })
    }

    pub fn search_threads(&self, query: &str, limit: usize) -> Result<Vec<ThreadSearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        if self.fts_available {
            if let Ok(results) = self.search_threads_fts(query, limit) {
                return Ok(results);
            }
        }

        self.search_threads_like(query, limit)
    }

    pub fn search_messages(&self, query: &str, limit: usize) -> Result<Vec<MessageSearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        if self.fts_available {
            if let Ok(results) = self.search_messages_fts(query, limit) {
                return Ok(results);
            }
        }

        self.search_messages_like(query, limit)
    }

    pub fn read_thread(&self, identifier: &str, limit: Option<usize>) -> Result<ThreadRead> {
        let session_id = self.resolve_session_id(identifier)?;
        let thread = self
            .load_thread(&session_id)?
            .ok_or_else(|| anyhow!("未找到线程: {}", identifier))?;
        let messages = self.read_messages_by_session(&session_id, limit)?;
        Ok(ThreadRead { thread, messages })
    }

    pub fn read_messages(
        &self,
        identifier: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MessageRecord>> {
        let session_id = self.resolve_session_id(identifier)?;
        self.read_messages_by_session(&session_id, limit)
    }

    pub fn read_events(&self, identifier: &str, limit: Option<usize>) -> Result<Vec<EventRecord>> {
        let session_id = self.resolve_session_id(identifier)?;
        self.read_events_by_session(&session_id, limit)
    }

    pub fn status(&self) -> Result<StatusSummary> {
        let counts = self.count_totals()?;
        let files = self.count_rows("files")?;
        Ok(StatusSummary {
            index_path: self.index_path.to_string_lossy().into_owned(),
            fts_available: self.fts_available,
            files,
            threads: counts.0,
            messages: counts.1,
            events: counts.2,
        })
    }

    fn load_existing_files(&self) -> Result<HashMap<String, FileState>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, session_id, modified_at, size FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                FileState {
                    session_id: row.get(1)?,
                    modified_at: row.get(2)?,
                    size: row.get(3)?,
                },
            ))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (path, state) = row?;
            map.insert(path, state);
        }
        Ok(map)
    }

    fn count_totals(&self) -> Result<(usize, usize, usize)> {
        Ok((
            self.count_rows("threads")?,
            self.count_rows("messages")?,
            self.count_rows("events")?,
        ))
    }

    fn count_rows(&self, table: &str) -> Result<usize> {
        let sql = format!("SELECT COUNT(*) FROM {}", table);
        let count = self.conn.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
    }

    fn search_threads_fts(&self, query: &str, limit: usize) -> Result<Vec<ThreadSearchHit>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                t.session_id,
                t.title,
                t.cwd,
                t.path,
                t.message_count,
                t.event_count,
                t.aggregate_text,
                snippet(threads_fts, 4, '[', ']', '…', 12)
            FROM threads_fts
            JOIN threads t ON t.id = threads_fts.rowid
            WHERE threads_fts MATCH ?1
            ORDER BY bm25(threads_fts)
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let aggregate_text: String = row.get(6)?;
            let snippet: Option<String> = row.get(7)?;
            Ok(ThreadSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                path: row.get(3)?,
                message_count: row.get::<_, i64>(4)? as usize,
                event_count: row.get::<_, i64>(5)? as usize,
                snippet: snippet.unwrap_or_else(|| excerpt(&aggregate_text, query, 120)),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_threads_like(&self, query: &str, limit: usize) -> Result<Vec<ThreadSearchHit>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, title, cwd, path, message_count, event_count, aggregate_text
            FROM threads
            WHERE lower(title) LIKE lower(?1)
                OR lower(ifnull(cwd, '')) LIKE lower(?1)
                OR lower(path) LIKE lower(?1)
                OR lower(aggregate_text) LIKE lower(?1)
            ORDER BY started_at DESC
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            let aggregate_text: String = row.get(6)?;
            Ok(ThreadSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                path: row.get(3)?,
                message_count: row.get::<_, i64>(4)? as usize,
                event_count: row.get::<_, i64>(5)? as usize,
                snippet: excerpt(&aggregate_text, query, 120),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_messages_fts(&self, query: &str, limit: usize) -> Result<Vec<MessageSearchHit>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                m.session_id,
                t.title,
                m.timestamp,
                m.role,
                m.text,
                snippet(messages_fts, 2, '[', ']', '…', 12)
            FROM messages_fts
            JOIN messages m ON m.id = messages_fts.rowid
            LEFT JOIN threads t ON t.session_id = m.session_id
            WHERE messages_fts MATCH ?1
            ORDER BY bm25(messages_fts)
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let text: String = row.get(4)?;
            let snippet: Option<String> = row.get(5)?;
            Ok(MessageSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                role: row.get(3)?,
                text: text.clone(),
                snippet: snippet.unwrap_or_else(|| excerpt(&text, query, 120)),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_messages_like(&self, query: &str, limit: usize) -> Result<Vec<MessageSearchHit>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT m.session_id, t.title, m.timestamp, m.role, m.text
            FROM messages m
            LEFT JOIN threads t ON t.session_id = m.session_id
            WHERE lower(m.text) LIKE lower(?1)
            ORDER BY m.timestamp DESC
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            let text: String = row.get(4)?;
            Ok(MessageSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                role: row.get(3)?,
                text: text.clone(),
                snippet: excerpt(&text, query, 120),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn resolve_session_id(&self, identifier: &str) -> Result<String> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id
            FROM threads
            WHERE session_id = ?1
                OR file_name = ?1
                OR replace(file_name, '.jsonl', '') = ?1
                OR file_name LIKE '%' || ?1 || '%'
            ORDER BY session_id
            LIMIT 5
            "#,
        )?;

        let mut rows = stmt.query(params![identifier])?;
        let mut matches = Vec::new();
        while let Some(row) = rows.next()? {
            matches.push(row.get::<_, String>(0)?);
        }

        matches.sort();
        matches.dedup();

        match matches.len() {
            0 => bail!("未找到线程: {}", identifier),
            1 => Ok(matches.remove(0)),
            _ => bail!("线程标识不唯一，请使用更精确的 session_id: {}", identifier),
        }
    }

    fn load_thread(&self, session_id: &str) -> Result<Option<ThreadRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT session_id, title, cwd, path, file_name, folder, started_at, ended_at,
                       message_count, event_count
                FROM threads
                WHERE session_id = ?1
                "#,
                params![session_id],
                |row| {
                    Ok(ThreadRecord {
                        session_id: row.get(0)?,
                        title: row.get(1)?,
                        cwd: row.get(2)?,
                        path: row.get(3)?,
                        file_name: row.get(4)?,
                        folder: row.get(5)?,
                        started_at: row.get(6)?,
                        ended_at: row.get(7)?,
                        message_count: row.get::<_, i64>(8)? as usize,
                        event_count: row.get::<_, i64>(9)? as usize,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn read_messages_by_session(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MessageRecord>> {
        if let Some(limit) = limit {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, role, text
                FROM (
                    SELECT session_id, timestamp, role, text, idx
                    FROM messages
                    WHERE session_id = ?1
                    ORDER BY idx DESC
                    LIMIT ?2
                )
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |row| {
                Ok(MessageRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    role: row.get(2)?,
                    text: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, role, text
                FROM messages
                WHERE session_id = ?1
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(MessageRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    role: row.get(2)?,
                    text: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        }
    }

    fn read_events_by_session(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<EventRecord>> {
        if let Some(limit) = limit {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, event_type, summary
                FROM (
                    SELECT session_id, timestamp, event_type, summary, idx
                    FROM events
                    WHERE session_id = ?1
                    ORDER BY idx DESC
                    LIMIT ?2
                )
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |row| {
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    summary: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, event_type, summary
                FROM events
                WHERE session_id = ?1
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    summary: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        }
    }
}

fn replace_session(
    tx: &Transaction<'_>,
    fts_available: bool,
    path: &Path,
    modified_at: i64,
    size: i64,
    old_session_id: Option<&str>,
    parsed: &ParsedSession,
) -> Result<()> {
    if let Some(old_session_id) = old_session_id {
        if old_session_id != parsed.session_id {
            delete_session(tx, fts_available, old_session_id)?;
        }
    }

    delete_session(tx, fts_available, &parsed.session_id)?;

    let path_string = path.to_string_lossy().into_owned();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let folder = path
        .parent()
        .map(|value| value.to_string_lossy().into_owned());

    tx.execute(
        r#"
        INSERT INTO threads (
            session_id, path, file_name, folder, cwd, title, started_at, ended_at,
            message_count, event_count, aggregate_text
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        params![
            parsed.session_id,
            path_string,
            file_name,
            folder,
            parsed.cwd,
            parsed.title,
            parsed.started_at,
            parsed.ended_at,
            parsed.messages.len() as i64,
            parsed.events.len() as i64,
            parsed.aggregate_text,
        ],
    )?;

    let thread_row_id = tx.last_insert_rowid();
    if fts_available {
        tx.execute(
            "INSERT INTO threads_fts(rowid, session_id, title, cwd, path, aggregate_text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                thread_row_id,
                parsed.session_id,
                parsed.title,
                parsed.cwd,
                path_string,
                parsed.aggregate_text,
            ],
        )?;
    }

    for (idx, message) in parsed.messages.iter().enumerate() {
        tx.execute(
            "INSERT INTO messages(session_id, idx, timestamp, role, text, raw_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                parsed.session_id,
                idx as i64,
                message.timestamp,
                message.role,
                message.text,
                message.raw_json,
            ],
        )?;
        let message_row_id = tx.last_insert_rowid();
        if fts_available {
            tx.execute(
                "INSERT INTO messages_fts(rowid, session_id, role, text) VALUES (?1, ?2, ?3, ?4)",
                params![
                    message_row_id,
                    parsed.session_id,
                    message.role,
                    message.text
                ],
            )?;
        }
    }

    for (idx, event) in parsed.events.iter().enumerate() {
        tx.execute(
            "INSERT INTO events(session_id, idx, timestamp, event_type, summary, raw_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                parsed.session_id,
                idx as i64,
                event.timestamp,
                event.event_type,
                event.summary,
                event.raw_json,
            ],
        )?;
    }

    tx.execute(
        "INSERT OR REPLACE INTO files(path, session_id, modified_at, size, synced_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![path_string, parsed.session_id, modified_at, size],
    )?;

    Ok(())
}

fn retain_previous_session_id(
    existing: &HashMap<String, FileState>,
    path: &str,
    retained_session_ids: &mut HashSet<String>,
) {
    if let Some(session_id) = existing
        .get(path)
        .and_then(|state| state.session_id.clone())
    {
        retained_session_ids.insert(session_id);
    }
}

fn delete_session(tx: &Transaction<'_>, fts_available: bool, session_id: &str) -> Result<()> {
    if fts_available {
        if let Some(thread_row_id) = tx
            .query_row(
                "SELECT id FROM threads WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            tx.execute(
                "DELETE FROM threads_fts WHERE rowid = ?1",
                params![thread_row_id],
            )?;
        }

        let mut stmt = tx.prepare("SELECT id FROM messages WHERE session_id = ?1")?;
        let ids = stmt
            .query_map(params![session_id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        for id in ids {
            tx.execute("DELETE FROM messages_fts WHERE rowid = ?1", params![id])?;
        }
    }

    tx.execute(
        "DELETE FROM messages WHERE session_id = ?1",
        params![session_id],
    )?;
    tx.execute(
        "DELETE FROM events WHERE session_id = ?1",
        params![session_id],
    )?;
    tx.execute(
        "DELETE FROM threads WHERE session_id = ?1",
        params![session_id],
    )?;
    tx.execute(
        "DELETE FROM files WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

fn system_time_to_nanos(time: SystemTime) -> Result<i64> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("invalid system time: {}", error))?;
    Ok(duration.as_nanos() as i64)
}
