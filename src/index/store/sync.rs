use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, OptionalExtension, Transaction};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime};
use walkdir::WalkDir;

use crate::parser::ParsedSession;

use super::super::types::{
    SyncFailure, SyncPlan, SyncPreflight, SyncReport, SyncRequest, SyncScope, SyncStats,
};
use super::lock::SyncLockGuard;
use super::resume::{build_sync_resume, SyncResumeState};
use super::Store;

#[derive(Debug, Clone)]
struct FileState {
    session_id: Option<String>,
    modified_at: i64,
    size: i64,
}

#[derive(Debug, Clone)]
struct CandidateFile {
    path: std::path::PathBuf,
    path_string: String,
    modified_at: i64,
    size: i64,
    effective_time: String,
}

#[derive(Debug, Clone)]
struct PreparedSyncRun {
    scope: SyncScope,
    run_candidates: Vec<CandidateFile>,
    unchanged_paths: Vec<String>,
    total_files: usize,
    changed_files: usize,
    unchanged_files: usize,
    total_bytes: u64,
    largest_file_bytes: u64,
    remaining_paths: Vec<String>,
    resumed_from_checkpoint: bool,
}

impl Store {
    pub(crate) fn run_sync(
        &mut self,
        sessions_dir: &Path,
        request: &SyncRequest,
        mut lock: Option<&mut SyncLockGuard>,
    ) -> Result<(SyncPlan, SyncReport)> {
        let existing = self.load_existing_files()?;
        let prepared =
            self.prepare_sync_run(sessions_dir, request, &existing, lock.as_deref_mut())?;
        let (recommended_action, reason) = classify_preflight(
            prepared.total_files,
            prepared.changed_files,
            prepared.total_bytes,
            prepared.largest_file_bytes,
            request.is_scoped(),
        );
        let preflight = SyncPreflight {
            total_files: prepared.total_files,
            changed_files: prepared.changed_files,
            unchanged_files: prepared.unchanged_files,
            total_bytes: prepared.total_bytes,
            largest_file_bytes: prepared.largest_file_bytes,
            recommended_action,
            reason,
        };
        let plan = SyncPlan {
            scope: prepared.scope.clone(),
            preflight: preflight.clone(),
        };

        if preflight.recommended_action == "skip" {
            let report = self.skip_sync_report(sessions_dir, &preflight, request, &prepared)?;
            return Ok((plan, report));
        }

        let report = self.sync_sessions(existing, request, prepared, lock)?;
        Ok((plan, report))
    }

    fn prepare_sync_run(
        &self,
        sessions_dir: &Path,
        request: &SyncRequest,
        existing: &HashMap<String, FileState>,
        lock: Option<&mut SyncLockGuard>,
    ) -> Result<PreparedSyncRun> {
        let (mut scope, candidates) = collect_candidate_files(sessions_dir, request, lock)?;
        let (candidates, resumed_from_checkpoint) = self.apply_resume_state(candidates, request)?;
        let mut total_bytes = 0u64;
        let mut largest_file_bytes = 0u64;
        let mut changed_candidates = Vec::new();
        let mut unchanged_paths = Vec::new();

        // Keep unchanged files outside the budget checkpoint so partial runs only persist
        // work that still needs a rebuild on the next invocation.
        for candidate in candidates {
            total_bytes += candidate.size as u64;
            largest_file_bytes = largest_file_bytes.max(candidate.size as u64);
            if is_unchanged(existing, &candidate) {
                unchanged_paths.push(candidate.path_string.clone());
            } else {
                changed_candidates.push(candidate);
            }
        }

        let total_files = changed_candidates.len() + unchanged_paths.len();
        let changed_files = changed_candidates.len();
        let unchanged_files = unchanged_paths.len();
        let (run_candidates, remaining_paths) =
            split_budgeted_candidates(changed_candidates, request.budget_files)?;
        scope.budget_files = request.budget_files;
        scope.candidate_files = total_files;

        Ok(PreparedSyncRun {
            scope,
            run_candidates,
            unchanged_paths,
            total_files,
            changed_files,
            unchanged_files,
            total_bytes,
            largest_file_bytes,
            remaining_paths,
            resumed_from_checkpoint,
        })
    }

    fn apply_resume_state(
        &self,
        candidates: Vec<CandidateFile>,
        request: &SyncRequest,
    ) -> Result<(Vec<CandidateFile>, bool)> {
        let Some(state) = self.load_sync_resume_state()? else {
            return Ok((candidates, false));
        };
        if state.request != *request {
            return Ok((candidates, false));
        }

        Ok((resume_candidates(candidates, state), true))
    }

    fn sync_sessions(
        &mut self,
        existing: HashMap<String, FileState>,
        request: &SyncRequest,
        prepared: PreparedSyncRun,
        mut lock: Option<&mut SyncLockGuard>,
    ) -> Result<SyncReport> {
        let mut seen_paths = HashSet::new();
        let mut retained_session_ids = HashSet::new();
        // Budgeted runs can skip parsing unchanged files, but cleanup still needs to treat
        // them as part of the current scope so an unscoped sync never prunes them by mistake.
        for path in &prepared.unchanged_paths {
            seen_paths.insert(path.clone());
            retain_previous_session_id(&existing, path, &mut retained_session_ids);
        }
        let fts_available = self.fts_available;
        let mut stats = SyncStats {
            scanned_files: prepared.unchanged_paths.len(),
            indexed_files: 0,
            skipped_files: prepared.unchanged_paths.len(),
            failed_files: 0,
            removed_files: 0,
            threads: 0,
            messages: 0,
            events: 0,
        };
        let mut failures = Vec::new();

        let tx = self.conn.transaction()?;

        for (index, candidate) in prepared.run_candidates.iter().enumerate() {
            heartbeat_if_needed(&mut lock, index)?;
            stats.scanned_files += 1;
            seen_paths.insert(candidate.path_string.clone());

            let is_unchanged = existing
                .get(&candidate.path_string)
                .map(|state| {
                    state.modified_at == candidate.modified_at && state.size == candidate.size
                })
                .unwrap_or(false);

            if is_unchanged {
                stats.skipped_files += 1;
                if let Some(session_id) = existing
                    .get(&candidate.path_string)
                    .and_then(|state| state.session_id.clone())
                {
                    retained_session_ids.insert(session_id);
                }
                continue;
            }

            let parsed = match crate::parser::parse_session_file(&candidate.path) {
                Ok(parsed) => parsed,
                Err(error) => {
                    stats.failed_files += 1;
                    retain_previous_session_id(
                        &existing,
                        &candidate.path_string,
                        &mut retained_session_ids,
                    );
                    failures.push(SyncFailure {
                        path: candidate.path_string.clone(),
                        error: error.to_string(),
                    });
                    continue;
                }
            };
            retained_session_ids.insert(parsed.session_id.clone());
            let old_session_id = existing
                .get(&candidate.path_string)
                .and_then(|state| state.session_id.clone());
            replace_session(
                &tx,
                fts_available,
                &candidate.path,
                candidate.modified_at,
                candidate.size,
                old_session_id.as_deref(),
                &parsed,
            )?;
            stats.indexed_files += 1;
        }

        // Scoped sync only refreshes the selected slice and avoids pruning unrelated history.
        // Full cleanup still belongs to an unscoped sync run.
        if !request.is_scoped() {
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
        }

        tx.commit()?;

        let counts = self.count_totals()?;
        stats.threads = counts.0;
        stats.messages = counts.1;
        stats.events = counts.2;
        let processed_files = prepared.run_candidates.len();
        let resume = self.finalize_sync_resume_state(request, &prepared, processed_files)?;
        Ok(SyncReport {
            partial: !failures.is_empty() || resume.state == "saved",
            stats,
            failures,
            resume,
        })
    }

    fn skip_sync_report(
        &self,
        sessions_dir: &Path,
        preflight: &SyncPreflight,
        request: &SyncRequest,
        prepared: &PreparedSyncRun,
    ) -> Result<SyncReport> {
        if !sessions_dir.exists() {
            bail!("会话目录不存在: {}", sessions_dir.display());
        }

        let counts = self.count_totals()?;
        let resume = self.finalize_sync_resume_state(request, prepared, 0)?;
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
            resume,
        })
    }

    fn finalize_sync_resume_state(
        &self,
        request: &SyncRequest,
        prepared: &PreparedSyncRun,
        processed_files: usize,
    ) -> Result<super::super::types::SyncResume> {
        let state_path = self.sync_resume_state_path();
        if !prepared.remaining_paths.is_empty() {
            let state = SyncResumeState::new(request.clone(), prepared.remaining_paths.clone())?;
            self.save_sync_resume_state(&state)?;
            return Ok(build_sync_resume(
                state_path,
                "saved",
                request.budget_files,
                prepared.resumed_from_checkpoint,
                processed_files,
                prepared.remaining_paths.len(),
                Some("Remaining changed files were saved for the next sync run.".to_string()),
            ));
        }

        if prepared.resumed_from_checkpoint {
            self.clear_sync_resume_state()?;
            return Ok(build_sync_resume(
                state_path,
                "completed",
                request.budget_files,
                true,
                processed_files,
                0,
                Some("Saved sync state was consumed and cleared.".to_string()),
            ));
        }

        Ok(build_sync_resume(
            state_path,
            "idle",
            request.budget_files,
            false,
            processed_files,
            0,
            None,
        ))
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

fn is_unchanged(existing: &HashMap<String, FileState>, candidate: &CandidateFile) -> bool {
    existing
        .get(&candidate.path_string)
        .map(|state| state.modified_at == candidate.modified_at && state.size == candidate.size)
        .unwrap_or(false)
}

fn split_budgeted_candidates(
    mut candidates: Vec<CandidateFile>,
    budget_files: Option<usize>,
) -> Result<(Vec<CandidateFile>, Vec<String>)> {
    let Some(limit) = budget_files else {
        return Ok((candidates, Vec::new()));
    };
    if limit == 0 {
        bail!("--budget-files 需要大于 0");
    }
    if candidates.len() <= limit {
        return Ok((candidates, Vec::new()));
    }

    let remaining = candidates
        .split_off(limit)
        .into_iter()
        .map(|candidate| candidate.path_string)
        .collect();
    Ok((candidates, remaining))
}

fn resume_candidates(candidates: Vec<CandidateFile>, state: SyncResumeState) -> Vec<CandidateFile> {
    let mut by_path = candidates
        .into_iter()
        .map(|candidate| (candidate.path_string.clone(), candidate))
        .collect::<HashMap<_, _>>();
    let mut resumed = Vec::new();

    // Reuse the saved path order so repeated runs keep a stable checkpoint sequence.
    for path in state.pending_paths {
        if let Some(candidate) = by_path.remove(&path) {
            resumed.push(candidate);
        }
    }

    resumed
}

fn collect_candidate_files(
    sessions_dir: &Path,
    request: &SyncRequest,
    mut lock: Option<&mut SyncLockGuard>,
) -> Result<(SyncScope, Vec<CandidateFile>)> {
    if !sessions_dir.exists() {
        bail!("会话目录不存在: {}", sessions_dir.display());
    }

    let since = normalize_scope_time(request.since.as_deref(), "--since")?;
    let until = normalize_scope_time(request.until.as_deref(), "--until")?;
    if matches!(request.recent, Some(0)) {
        bail!("--recent 需要大于 0");
    }
    if matches!(request.budget_files, Some(0)) {
        bail!("--budget-files 需要大于 0");
    }
    if let (Some(since), Some(until)) = (&since, &until) {
        if since > until {
            bail!("无效的同步时间范围: --since 不能晚于 --until");
        }
    }

    let path_filter = request.path.as_ref().map(|value| value.to_lowercase());
    let mut candidates = Vec::new();

    for (index, entry) in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
        .enumerate()
    {
        heartbeat_if_needed(&mut lock, index)?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }

        let path = entry.path().to_path_buf();
        let path_string = path.to_string_lossy().into_owned();
        if !matches_path_scope(&path_string, path_filter.as_deref()) {
            continue;
        }

        let metadata =
            fs::metadata(&path).with_context(|| format!("failed to stat {}", path.display()))?;
        let modified_at = metadata
            .modified()
            .and_then(|value| system_time_to_nanos(value).map_err(std::io::Error::other))
            .with_context(|| format!("failed to read mtime {}", path.display()))?;
        let effective_time = resolve_candidate_time(&path, modified_at)?;
        if !matches_time_scope(&effective_time, since.as_deref(), until.as_deref()) {
            continue;
        }

        candidates.push(CandidateFile {
            path,
            path_string,
            modified_at,
            size: metadata.len() as i64,
            effective_time,
        });
    }

    // Keep the newest candidates first so `--recent` trims deterministically.
    candidates.sort_by(|left, right| {
        right
            .effective_time
            .cmp(&left.effective_time)
            .then_with(|| left.path_string.cmp(&right.path_string))
    });
    if let Some(limit) = request.recent {
        candidates.truncate(limit);
    }

    Ok((
        SyncScope {
            since,
            until,
            path: request.path.clone(),
            recent: request.recent,
            budget_files: request.budget_files,
            candidate_files: candidates.len(),
        },
        candidates,
    ))
}

fn heartbeat_if_needed(lock: &mut Option<&mut SyncLockGuard>, index: usize) -> Result<()> {
    if index % 32 != 0 {
        return Ok(());
    }
    if let Some(lock) = lock.as_deref_mut() {
        lock.heartbeat()?;
    }
    Ok(())
}

fn normalize_scope_time(value: Option<&str>, flag: &str) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }

    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .with_context(|| format!("{flag} 需要使用 RFC3339 时间，例如 2026-04-12T10:30:00Z"))?;
    parsed
        .format(&Rfc3339)
        .map(Some)
        .map_err(|error| anyhow!("failed to format time: {}", error))
}

fn matches_path_scope(path: &str, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    path.to_lowercase().contains(filter)
}

fn matches_time_scope(path_time: &str, since: Option<&str>, until: Option<&str>) -> bool {
    if let Some(since) = since {
        if path_time < since {
            return false;
        }
    }
    if let Some(until) = until {
        if path_time > until {
            return false;
        }
    }
    true
}

fn resolve_candidate_time(path: &Path, modified_at: i64) -> Result<String> {
    // Prefer the rollout timestamp encoded in the filename so range filtering stays cheap.
    // Fall back to file mtime when the filename does not carry a parseable session timestamp.
    if let Some(timestamp) = extract_session_timestamp(path) {
        return Ok(timestamp);
    }
    format_modified_time(modified_at)
}

fn extract_session_timestamp(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".jsonl")?;
    let rest = stem.strip_prefix("rollout-")?;
    let timestamp = match rest.rsplit_once("-session-") {
        Some((timestamp, _)) => timestamp,
        None => rest,
    };
    let parsed = PrimitiveDateTime::parse(
        timestamp,
        &format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]"),
    )
    .ok()?;
    parsed.assume_utc().format(&Rfc3339).ok()
}

fn format_modified_time(modified_at: i64) -> Result<String> {
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(modified_at as i128)
        .map_err(|error| anyhow!("invalid modified time: {}", error))?;
    datetime
        .format(&Rfc3339)
        .map_err(|error| anyhow!("failed to format modified time: {}", error))
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
    scoped: bool,
) -> (String, String) {
    if total_files == 0 {
        let reason = if scoped {
            "当前范围内未发现可同步的会话文件"
        } else {
            "未发现可同步的会话文件"
        };
        return ("skip".to_string(), reason.to_string());
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
