#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::json;

pub fn write_fixture_sessions(root: &Path) -> (PathBuf, PathBuf) {
    let session_dir = root.join("sessions").join("2026").join("04").join("12");
    fs::create_dir_all(&session_dir).unwrap();

    let alpha = session_dir.join("rollout-2026-04-12T10-00-00-session-alpha.jsonl");
    let beta = session_dir.join("rollout-2026-04-12T11-00-00-session-beta.jsonl");

    fs::write(&alpha, alpha_session()).unwrap();
    fs::write(&beta, beta_session()).unwrap();

    (alpha, beta)
}

#[allow(dead_code)]
pub fn write_invalid_session(root: &Path) -> PathBuf {
    let session_dir = root.join("sessions").join("2026").join("04").join("12");
    fs::create_dir_all(&session_dir).unwrap();

    let invalid = session_dir.join("rollout-2026-04-12T12-00-00-session-invalid.jsonl");
    fs::write(
        &invalid,
        "{\"timestamp\":\"2026-04-12T12:00:00Z\",\"type\":\"session_meta\"\nnot-json\n",
    )
    .unwrap();

    invalid
}

#[allow(dead_code)]
pub fn append_alpha_message(alpha_path: &Path) {
    let extra = r#"{"timestamp":"2026-04-12T10:00:08Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The CLI now supports incremental sync."}]}}"#;
    let mut content = fs::read_to_string(alpha_path).unwrap();
    content.push('\n');
    content.push_str(extra);
    content.push('\n');
    fs::write(alpha_path, content).unwrap();
}

#[allow(dead_code)]
pub fn overwrite_with_invalid_json(path: &Path) {
    fs::write(
        path,
        "{\"timestamp\":\"2026-04-12T10:00:00Z\",\"type\":\"session_meta\"\nnot-json\n",
    )
    .unwrap();
}

#[allow(dead_code)]
pub fn rewrite_alpha_session_with_extra_message(path: &Path) {
    fs::write(
        path,
        [
            r#"{"timestamp":"2026-04-12T10:00:00Z","type":"session_meta","payload":{"id":"session-alpha","timestamp":"2026-04-12T10:00:00Z","cwd":"/workspace/alpha-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Please build a CLI for thread search"}]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I will rebuild this CLI with an append-aware sync path."}]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The parser should avoid reprocessing unchanged prefixes."}]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"rg\",\"append\"]}"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"Please build a CLI for thread search","images":[]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:05Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"Re-evaluating append-tail sync strategy"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:06Z","type":"turn_context","payload":{"cwd":"/workspace/alpha-repo","approval_policy":"on-request","model":"gpt-5-codex"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:07Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"This rewritten session should trigger a rebuild fallback."}]}}"#,
        ]
        .join("\n"),
    )
    .unwrap();
}

#[allow(dead_code)]
pub fn truncate_alpha_session(path: &Path) {
    fs::write(
        path,
        [
            r#"{"timestamp":"2026-04-12T10:00:00Z","type":"session_meta","payload":{"id":"session-alpha","timestamp":"2026-04-12T10:00:00Z","cwd":"/workspace/alpha-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Please build a CLI for thread search"}]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I will build a CLI using Rust and SQLite."}]}}"#,
            r#"{"timestamp":"2026-04-12T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"rg\",\"CLI\"]}"}}"#,
            r#"{"timestamp":"2026-04-12T10:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"Please build a CLI for thread search","images":[]}}"#,
        ]
        .join("\n"),
    )
    .unwrap();
}

#[allow(dead_code)]
pub fn write_sync_lock(
    index_dir: &Path,
    pid: u32,
    started_at: &str,
    heartbeat_at: &str,
) -> PathBuf {
    fs::create_dir_all(index_dir).unwrap();
    let lock_path = index_dir.join("sync.lock.json");
    let payload = json!({
        "pid": pid,
        "command": "sync",
        "index_path": index_dir.join("threads.sqlite3").to_string_lossy().to_string(),
        "started_at": started_at,
        "heartbeat_at": heartbeat_at,
    });
    fs::write(&lock_path, serde_json::to_vec(&payload).unwrap()).unwrap();
    lock_path
}

#[allow(dead_code)]
pub fn write_invalid_resume_state(index_dir: &Path) -> PathBuf {
    fs::create_dir_all(index_dir).unwrap();
    let path = index_dir.join("sync.resume.json");
    fs::write(&path, "{not-json").unwrap();
    path
}

#[allow(dead_code)]
pub fn write_invalid_refresh_state(index_dir: &Path) -> PathBuf {
    fs::create_dir_all(index_dir).unwrap();
    let path = index_dir.join("sync.refresh.json");
    fs::write(&path, "{not-json").unwrap();
    path
}

#[derive(Debug)]
pub struct AppThreadRow {
    pub id: String,
    pub rollout_path: String,
    pub title: String,
    pub archived: i64,
}

#[allow(dead_code)]
pub fn write_codex_app_state(codex_home: &Path) -> (PathBuf, PathBuf) {
    fs::create_dir_all(codex_home).unwrap();

    let state_db_path = codex_home.join("state_5.sqlite");
    let conn = Connection::open(&state_db_path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            model_provider TEXT NOT NULL,
            cwd TEXT NOT NULL,
            title TEXT NOT NULL,
            sandbox_policy TEXT NOT NULL,
            approval_mode TEXT NOT NULL,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            has_user_event INTEGER NOT NULL DEFAULT 0,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_at INTEGER,
            git_sha TEXT,
            git_branch TEXT,
            git_origin_url TEXT,
            cli_version TEXT NOT NULL DEFAULT '',
            first_user_message TEXT NOT NULL DEFAULT '',
            agent_nickname TEXT,
            agent_role TEXT,
            memory_mode TEXT NOT NULL DEFAULT 'enabled',
            model TEXT,
            reasoning_effort TEXT,
            agent_path TEXT
        );
        "#,
    )
    .unwrap();

    let global_state_path = codex_home.join(".codex-global-state.json");
    fs::write(
        &global_state_path,
        serde_json::to_vec_pretty(&json!({
            "pinned-thread-ids": ["already-pinned"]
        }))
        .unwrap(),
    )
    .unwrap();

    (state_db_path, global_state_path)
}

#[allow(dead_code)]
pub fn read_app_thread_row(state_db_path: &Path, id: &str) -> Option<AppThreadRow> {
    let conn = Connection::open(state_db_path).unwrap();
    conn.query_row(
        "SELECT id, rollout_path, title, archived FROM threads WHERE id = ?1",
        [id],
        |row| {
            Ok(AppThreadRow {
                id: row.get(0)?,
                rollout_path: row.get(1)?,
                title: row.get(2)?,
                archived: row.get(3)?,
            })
        },
    )
    .ok()
}

#[allow(dead_code)]
pub fn read_pinned_thread_ids(global_state_path: &Path) -> Vec<String> {
    let raw = fs::read_to_string(global_state_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    value
        .get("pinned-thread-ids")
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn alpha_session() -> String {
    [
        r#"{"timestamp":"2026-04-12T10:00:00Z","type":"session_meta","payload":{"id":"session-alpha","timestamp":"2026-04-12T10:00:00Z","cwd":"/workspace/alpha-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Please build a CLI for thread search"}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I will build a CLI using Rust and SQLite."}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The C++ parser still needs better symbol-aware search."}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"rg\",\"CLI\"]}"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"Please build a CLI for thread search","images":[]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:05Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"Planning CLI surface and indexing layout"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:06Z","type":"turn_context","payload":{"cwd":"/workspace/alpha-repo","approval_policy":"on-request","model":"gpt-5-codex"}}"#,
    ]
    .join("\n")
}

fn beta_session() -> String {
    [
        r#"{"timestamp":"2026-04-12T11:00:00Z","type":"session_meta","payload":{"id":"session-beta","timestamp":"2026-04-12T11:00:00Z","cwd":"/workspace/beta-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
        r#"{"timestamp":"2026-04-12T11:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Investigate websocket reconnect masking"}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The reconnect overlay should only appear after policy rejection."}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The plain C fallback is still too broad."}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:03Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"Checking websocket status handling"}}"#,
    ]
    .join("\n")
}
