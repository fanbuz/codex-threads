use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::parser::{parse_session_file, ParsedSession};

use super::{RestoreAppThreadReport, RestoredAppThread};

const DEFAULT_SOURCE: &str = "vscode";
const DEFAULT_MODEL_PROVIDER: &str = "custom";
const DEFAULT_SANDBOX_POLICY: &str = "workspace-write";
const DEFAULT_APPROVAL_MODE: &str = "on-request";
const REQUIRED_THREAD_COLUMNS: [&str; 12] = [
    "id",
    "rollout_path",
    "created_at",
    "updated_at",
    "source",
    "model_provider",
    "cwd",
    "title",
    "sandbox_policy",
    "approval_mode",
    "archived",
    "archived_at",
];

struct SessionMatch {
    path: PathBuf,
    parsed: ParsedSession,
}

#[derive(Debug, Clone)]
struct InsertDefaults {
    source: String,
    model_provider: String,
    sandbox_policy: String,
    approval_mode: String,
}

pub fn restore_app_thread(
    thread_id: &str,
    sessions_dir: &Path,
    codex_home: &Path,
    pin: bool,
    dry_run: bool,
) -> Result<RestoreAppThreadReport> {
    let session_match = find_session(sessions_dir, thread_id)?;
    let state_db_path = codex_home.join("state_5.sqlite");
    if !state_db_path.exists() {
        bail!("Codex App 状态库不存在: {}", state_db_path.display());
    }

    let global_state_path = codex_home.join(".codex-global-state.json");
    let thread = build_thread_record(thread_id, &session_match)?;

    let (thread_exists, defaults) = {
        let conn = Connection::open(&state_db_path)
            .with_context(|| format!("failed to open {}", state_db_path.display()))?;
        ensure_threads_schema(&conn)?;
        ensure_rollout_path_is_safe(&conn, &thread.id, &thread.rollout_path)?;
        (
            thread_exists(&conn, &thread.id)?,
            load_insert_defaults(&conn)?,
        )
    };

    let thread_action = match (dry_run, thread_exists) {
        (true, true) => "would_update_existing",
        (true, false) => "would_insert",
        (false, true) => "updated_existing",
        (false, false) => "inserted",
    }
    .to_string();

    let (mut global_state, pin_action) =
        load_global_state(&global_state_path, &thread.id, pin, dry_run)?;
    let warnings = collect_warnings(&state_db_path);

    let backup_dir = if dry_run {
        None
    } else {
        // Always take a point-in-time backup before mutating private app state.
        let backup_dir = create_backup_dir(codex_home, &thread.id)?;
        backup_file_if_exists(&state_db_path, &backup_dir)?;
        backup_file_if_exists(&sqlite_sidecar_path(&state_db_path, "-wal"), &backup_dir)?;
        backup_file_if_exists(&sqlite_sidecar_path(&state_db_path, "-shm"), &backup_dir)?;
        if pin {
            backup_file_if_exists(&global_state_path, &backup_dir)?;
        }
        Some(backup_dir)
    };

    if !dry_run {
        let mut conn = Connection::open(&state_db_path)
            .with_context(|| format!("failed to open {}", state_db_path.display()))?;
        upsert_thread(&mut conn, &thread, &defaults)?;
        if pin {
            ensure_thread_is_pinned(&mut global_state, &thread.id)?;
            write_global_state(&global_state_path, &global_state)?;
        }
    }

    Ok(RestoreAppThreadReport {
        feature: "restore-app-thread".to_string(),
        dry_run,
        thread_id: thread.id.clone(),
        thread_action,
        pin_action,
        state_db_path: state_db_path.to_string_lossy().into_owned(),
        global_state_path: global_state_path.to_string_lossy().into_owned(),
        backup_dir: backup_dir.map(|path| path.to_string_lossy().into_owned()),
        warnings,
        thread,
    })
}

fn find_session(sessions_dir: &Path, thread_id: &str) -> Result<SessionMatch> {
    if !sessions_dir.exists() {
        bail!("会话目录不存在: {}", sessions_dir.display());
    }

    for entry in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }

        let Ok(parsed) = parse_session_file(path) else {
            continue;
        };
        if parsed.session_id == thread_id {
            return Ok(SessionMatch {
                path: path.to_path_buf(),
                parsed,
            });
        }
    }

    bail!(
        "未在 {} 中找到 thread id {} 对应的原始会话文件",
        sessions_dir.display(),
        thread_id
    )
}

fn build_thread_record(thread_id: &str, session_match: &SessionMatch) -> Result<RestoredAppThread> {
    let file_timestamp = file_modified_ms(&session_match.path)?;
    let created_at = session_match
        .parsed
        .started_at
        .as_deref()
        .and_then(parse_rfc3339_ms)
        .unwrap_or(file_timestamp);
    let updated_at = session_match
        .parsed
        .ended_at
        .as_deref()
        .and_then(parse_rfc3339_ms)
        .unwrap_or(file_timestamp);

    Ok(RestoredAppThread {
        id: thread_id.to_string(),
        rollout_path: session_match.path.to_string_lossy().into_owned(),
        title: session_match.parsed.title.clone(),
        cwd: session_match.parsed.cwd.clone().unwrap_or_default(),
        created_at,
        updated_at,
        archived: false,
    })
}

fn file_modified_ms(path: &Path) -> Result<i64> {
    let modified = fs::metadata(path)
        .with_context(|| format!("failed to read metadata {}", path.display()))?
        .modified()
        .with_context(|| format!("failed to read modified time {}", path.display()))?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .with_context(|| format!("failed to normalize modified time {}", path.display()))?;
    Ok(duration.as_millis() as i64)
}

fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|datetime| (datetime.unix_timestamp_nanos() / 1_000_000) as i64)
}

fn ensure_threads_schema(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
    let mut rows = stmt.query([])?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        columns.push(row.get::<_, String>(1)?);
    }

    if columns.is_empty() {
        bail!("Codex App 状态库中不存在 threads 表，暂时无法恢复线程视图");
    }

    // Refuse to write when the app schema drifts beyond the fields this experiment understands.
    let missing = REQUIRED_THREAD_COLUMNS
        .iter()
        .filter(|column| !columns.iter().any(|existing| existing == *column))
        .map(|column| (*column).to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "当前 Codex App threads 表缺少必要字段: {}",
            missing.join(", ")
        );
    }

    Ok(())
}

fn ensure_rollout_path_is_safe(
    conn: &Connection,
    thread_id: &str,
    rollout_path: &str,
) -> Result<()> {
    let existing = conn
        .query_row(
            "SELECT id FROM threads WHERE rollout_path = ?1 LIMIT 1",
            [rollout_path],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if existing != thread_id {
            bail!(
                "state_5.sqlite 中已有其他线程占用了同一个 rollout_path: {}",
                existing
            );
        }
    }
    Ok(())
}

fn thread_exists(conn: &Connection, thread_id: &str) -> Result<bool> {
    let existing = conn
        .query_row(
            "SELECT 1 FROM threads WHERE id = ?1 LIMIT 1",
            [thread_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    Ok(existing.is_some())
}

fn load_insert_defaults(conn: &Connection) -> Result<InsertDefaults> {
    let defaults = conn
        .query_row(
            r#"
            SELECT source, model_provider, sandbox_policy, approval_mode
            FROM threads
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
            [],
            |row| {
                Ok(InsertDefaults {
                    source: row.get(0)?,
                    model_provider: row.get(1)?,
                    sandbox_policy: row.get(2)?,
                    approval_mode: row.get(3)?,
                })
            },
        )
        .optional()?;

    Ok(defaults.unwrap_or_else(|| InsertDefaults {
        source: DEFAULT_SOURCE.to_string(),
        model_provider: DEFAULT_MODEL_PROVIDER.to_string(),
        sandbox_policy: DEFAULT_SANDBOX_POLICY.to_string(),
        approval_mode: DEFAULT_APPROVAL_MODE.to_string(),
    }))
}

fn load_global_state(
    path: &Path,
    thread_id: &str,
    pin: bool,
    dry_run: bool,
) -> Result<(Value, String)> {
    if !pin {
        return Ok((Value::Object(Map::new()), "skipped".to_string()));
    }

    let mut value = if path.exists() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        Value::Object(Map::new())
    };

    let already_pinned = contains_pinned_thread(&value, thread_id)?;
    let action = if already_pinned {
        "already_pinned"
    } else if dry_run {
        "would_pin"
    } else {
        ensure_thread_is_pinned(&mut value, thread_id)?;
        "pinned"
    };

    Ok((value, action.to_string()))
}

fn contains_pinned_thread(value: &Value, thread_id: &str) -> Result<bool> {
    let Some(object) = value.as_object() else {
        bail!("Codex 全局状态文件格式无效，顶层不是对象");
    };

    let Some(pinned) = object.get("pinned-thread-ids") else {
        return Ok(false);
    };
    let Some(array) = pinned.as_array() else {
        bail!("Codex 全局状态文件里的 pinned-thread-ids 不是数组");
    };

    Ok(array
        .iter()
        .filter_map(Value::as_str)
        .any(|existing| existing == thread_id))
}

fn ensure_thread_is_pinned(value: &mut Value, thread_id: &str) -> Result<()> {
    let Some(object) = value.as_object_mut() else {
        bail!("Codex 全局状态文件格式无效，顶层不是对象");
    };

    let existing = object
        .entry("pinned-thread-ids".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(array) = existing.as_array_mut() else {
        bail!("Codex 全局状态文件里的 pinned-thread-ids 不是数组");
    };

    if array
        .iter()
        .filter_map(Value::as_str)
        .all(|existing| existing != thread_id)
    {
        array.push(Value::String(thread_id.to_string()));
    }

    Ok(())
}

fn write_global_state(path: &Path, value: &Value) -> Result<()> {
    let serialized = serde_json::to_vec_pretty(value)?;
    fs::write(path, serialized).with_context(|| format!("failed to write {}", path.display()))
}

fn create_backup_dir(codex_home: &Path, thread_id: &str) -> Result<PathBuf> {
    let stamp = OffsetDateTime::now_utc().unix_timestamp();
    let dir = codex_home.join("experimental-backups").join(format!(
        "restore-app-thread-{}-{}",
        stamp,
        sanitize_component(thread_id)
    ));
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    Ok(dir)
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn backup_file_if_exists(path: &Path, backup_dir: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let file_name = path
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_default();
    let backup_path = backup_dir.join(file_name);
    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "failed to backup {} to {}",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(())
}

fn sqlite_sidecar_path(state_db_path: &Path, suffix: &str) -> PathBuf {
    let file_name = state_db_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "state_5.sqlite".to_string());
    state_db_path.with_file_name(format!("{}{}", file_name, suffix))
}

fn collect_warnings(state_db_path: &Path) -> Vec<String> {
    let mut warnings = vec![
        "请优先在 Codex App 已退出的前提下执行写入，避免本地 sqlite 状态被重新覆盖。".to_string(),
    ];

    let wal_path = sqlite_sidecar_path(state_db_path, "-wal");
    let shm_path = sqlite_sidecar_path(state_db_path, "-shm");
    if wal_path.exists() || shm_path.exists() {
        warnings.push(
            "检测到 sqlite sidecar 文件，请在恢复完成后重启 Codex App 再验证线程视图。".to_string(),
        );
    }

    warnings
}

fn upsert_thread(
    conn: &mut Connection,
    thread: &RestoredAppThread,
    defaults: &InsertDefaults,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        r#"
        INSERT INTO threads (
            id,
            rollout_path,
            created_at,
            updated_at,
            source,
            model_provider,
            cwd,
            title,
            sandbox_policy,
            approval_mode,
            archived,
            archived_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
        ON CONFLICT(id) DO UPDATE SET
            rollout_path = excluded.rollout_path,
            updated_at = excluded.updated_at,
            cwd = excluded.cwd,
            title = excluded.title,
            archived = 0,
            archived_at = NULL
        "#,
        params![
            thread.id,
            thread.rollout_path,
            thread.created_at,
            thread.updated_at,
            defaults.source,
            defaults.model_provider,
            thread.cwd,
            thread.title,
            defaults.sandbox_policy,
            defaults.approval_mode,
            0_i64,
        ],
    )?;
    tx.commit()?;
    Ok(())
}
