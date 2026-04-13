use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, OptionalExtension, Transaction};
use walkdir::WalkDir;

use crate::parser::ParsedSession;

use super::super::types::{SyncFailure, SyncPreflight, SyncReport, SyncStats};
use super::Store;

#[derive(Debug, Clone)]
struct FileState {
    session_id: Option<String>,
    modified_at: i64,
    size: i64,
}

impl Store {
    pub fn preflight_sync(&self, sessions_dir: &Path) -> Result<SyncPreflight> {
        if !sessions_dir.exists() {
            bail!("会话目录不存在: {}", sessions_dir.display());
        }

        let existing = self.load_existing_files()?;
        let mut total_files = 0usize;
        let mut changed_files = 0usize;
        let mut total_bytes = 0u64;
        let mut largest_file_bytes = 0u64;

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

            total_files += 1;
            let path = entry.path().to_path_buf();
            let path_string = path.to_string_lossy().into_owned();
            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to stat {}", path.display()))?;
            let modified_at = metadata
                .modified()
                .and_then(|value| system_time_to_nanos(value).map_err(std::io::Error::other))
                .with_context(|| format!("failed to read mtime {}", path.display()))?;
            let size = metadata.len();
            total_bytes += size;
            largest_file_bytes = largest_file_bytes.max(size);

            let is_unchanged = existing
                .get(&path_string)
                .map(|state| state.modified_at == modified_at && state.size == size as i64)
                .unwrap_or(false);
            if !is_unchanged {
                changed_files += 1;
            }
        }

        let unchanged_files = total_files.saturating_sub(changed_files);
        let (recommended_action, reason) =
            classify_preflight(total_files, changed_files, total_bytes, largest_file_bytes);

        Ok(SyncPreflight {
            total_files,
            changed_files,
            unchanged_files,
            total_bytes,
            largest_file_bytes,
            recommended_action,
            reason,
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

    pub fn skip_sync_report(
        &self,
        sessions_dir: &Path,
        preflight: &SyncPreflight,
    ) -> Result<SyncReport> {
        if !sessions_dir.exists() {
            bail!("会话目录不存在: {}", sessions_dir.display());
        }

        let counts = self.count_totals()?;
        Ok(SyncReport {
            partial: false,
            stats: SyncStats {
                scanned_files: preflight.total_files,
                indexed_files: 0,
                skipped_files: preflight.total_files,
                failed_files: 0,
                removed_files: 0,
                threads: counts.0,
                messages: counts.1,
                events: counts.2,
            },
            failures: Vec::new(),
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
        let event_row_id = tx.last_insert_rowid();
        if fts_available {
            tx.execute(
                "INSERT INTO events_fts(rowid, session_id, event_type, summary) VALUES (?1, ?2, ?3, ?4)",
                params![
                    event_row_id,
                    parsed.session_id,
                    event.event_type,
                    event.summary
                ],
            )?;
        }
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

        let mut stmt = tx.prepare("SELECT id FROM events WHERE session_id = ?1")?;
        let ids = stmt
            .query_map(params![session_id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        for id in ids {
            tx.execute("DELETE FROM events_fts WHERE rowid = ?1", params![id])?;
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

fn classify_preflight(
    total_files: usize,
    changed_files: usize,
    total_bytes: u64,
    largest_file_bytes: u64,
) -> (String, String) {
    if total_files == 0 {
        return ("skip".to_string(), "未发现可同步的会话文件".to_string());
    }

    if changed_files == 0 {
        return ("skip".to_string(), "未检测到发生变化的会话文件".to_string());
    }

    if total_bytes >= 1_000_000_000 || largest_file_bytes >= 100_000_000 || changed_files >= 100 {
        return (
            "heavy-sync".to_string(),
            "检测到大体量或高变更同步，建议关注本次同步成本".to_string(),
        );
    }

    (
        "sync".to_string(),
        "检测到有变更文件，建议执行同步".to_string(),
    )
}
