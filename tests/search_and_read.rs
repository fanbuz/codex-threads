mod common;

use assert_cmd::Command;
use rusqlite::{params, Connection};
use serde_json::Value;
use tempfile::tempdir;

fn seed_index() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "sync",
        ])
        .assert()
        .success();

    (tmp, sessions_dir, index_dir)
}

fn assert_native_handoff(value: &Value, session_id: &str) {
    assert_eq!(value["source"]["kind"], "local_codex_session");
    assert!(value["source"]["path"]
        .as_str()
        .unwrap()
        .ends_with(".jsonl"));
    assert_eq!(value["handoff"]["target"], "codex_native_threads");
    assert_eq!(value["handoff"]["candidate_thread_id"], session_id);
    assert_eq!(value["handoff"]["verification"], "required");
    assert_eq!(value["handoff"]["recommended_action"], "confirm_then_read");
    assert!(value["handoff"]["local_fallback"]
        .as_str()
        .unwrap()
        .contains(session_id));
}

fn insert_search_rows(
    index_dir: &std::path::Path,
    count: usize,
    search_text: impl Fn(usize) -> String,
) {
    let mut conn = Connection::open(index_dir.join("threads.sqlite3")).unwrap();
    let tx = conn.transaction().unwrap();
    for index in 0..count {
        let session_id = format!("bulk-session-{index:03}");
        let path = format!("/tmp/{session_id}.jsonl");
        let timestamp = format!("2026-05-01T00:{:02}:{:02}Z", index / 60, index % 60);
        let text = search_text(index);

        tx.execute(
            r#"
            INSERT INTO threads(
                session_id, path, file_name, folder, cwd, title, started_at, ended_at,
                message_count, event_count, aggregate_text
            ) VALUES (?1, ?2, ?3, NULL, '/workspace/bulk', ?4, ?5, ?5, 1, 1, ?6)
            "#,
            params![
                session_id,
                path,
                format!("{session_id}.jsonl"),
                format!("Bulk search thread {index}"),
                timestamp,
                text,
            ],
        )
        .unwrap();
        let thread_row_id = tx.last_insert_rowid();
        tx.execute(
            r#"
            INSERT INTO threads_fts(rowid, session_id, title, cwd, path, aggregate_text)
            VALUES (?1, ?2, ?3, '/workspace/bulk', ?4, ?5)
            "#,
            params![
                thread_row_id,
                session_id,
                format!("Bulk search thread {index}"),
                path,
                text,
            ],
        )
        .unwrap();

        tx.execute(
            "INSERT INTO messages(session_id, idx, timestamp, role, text, raw_json) VALUES (?1, 0, ?2, 'assistant', ?3, '{}')",
            params![session_id, timestamp, text],
        )
        .unwrap();
        let message_row_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO messages_fts(rowid, session_id, role, text) VALUES (?1, ?2, 'assistant', ?3)",
            params![message_row_id, session_id, text],
        )
        .unwrap();

        tx.execute(
            "INSERT INTO events(session_id, idx, timestamp, event_type, summary, raw_json) VALUES (?1, 0, ?2, 'agent_reasoning', ?3, '{}')",
            params![session_id, timestamp, text],
        )
        .unwrap();
        let event_row_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO events_fts(rowid, session_id, event_type, summary) VALUES (?1, ?2, 'agent_reasoning', ?3)",
            params![event_row_id, session_id, text],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}

#[test]
fn messages_search_returns_matching_snippets() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "Rust and SQLite",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "messages.search");
    assert!(json.get("duration_ms").and_then(Value::as_u64).is_some());
    assert!(json.get("duration_display").is_none());
    assert_eq!(json["count"], 1);
    assert_eq!(json["search"]["backend"], "fts");
    assert_eq!(json["search"]["query_mode"], "literal");
    assert_eq!(json["search"]["ranking"], "bm25");
    assert_eq!(
        json["search"]["normalized_terms"],
        serde_json::json!(["Rust", "and", "SQLite"])
    );
    assert_eq!(json["results"][0]["session_id"], "session-alpha");
    assert_eq!(json["results"][0]["role"], "assistant");
    assert_eq!(json["results"][0]["explain"]["rank"], 1);
    assert_eq!(
        json["results"][0]["explain"]["matched_fields"],
        serde_json::json!(["text"])
    );
    assert_eq!(json["results"][0]["explain"]["matched_terms"], 3);
    assert_eq!(json["results"][0]["explain"]["literal_match"], true);
    assert_native_handoff(&json["results"][0], "session-alpha");
}

#[test]
fn literal_fts_search_honors_limits_above_fallback_candidate_cap() {
    let (_tmp, sessions_dir, index_dir) = seed_index();
    insert_search_rows(&index_dir, 300, |index| {
        format!("limitneedle result {index}")
    });

    for command in ["threads", "messages", "events"] {
        let output = Command::cargo_bin("codex-threads")
            .unwrap()
            .args([
                "--json",
                "--sessions-dir",
                sessions_dir.to_str().unwrap(),
                "--index-dir",
                index_dir.to_str().unwrap(),
                command,
                "search",
                "limitneedle",
                "--limit",
                "300",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json["search"]["backend"], "fts", "{command}");
        assert_eq!(json["search"]["query_mode"], "literal", "{command}");
        assert_eq!(json["count"], 300, "{command}");
    }
}

#[test]
fn literal_search_supplements_saturated_fts_candidates_after_filtering() {
    let (_tmp, sessions_dir, index_dir) = seed_index();
    insert_search_rows(&index_dir, 300, |index| {
        if index == 0 {
            format!("target-phrase literal {index}")
        } else if index >= 296 {
            format!("target-phrase {} literal {index}", "padding ".repeat(100))
        } else {
            format!("target phrase token match {index}")
        }
    });

    for command in ["threads", "messages", "events"] {
        let output = Command::cargo_bin("codex-threads")
            .unwrap()
            .args([
                "--json",
                "--sessions-dir",
                sessions_dir.to_str().unwrap(),
                "--index-dir",
                index_dir.to_str().unwrap(),
                command,
                "search",
                "target-phrase",
                "--limit",
                "100",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json["search"]["query_mode"], "literal", "{command}");
        assert_eq!(json["count"], 5, "{command}");
    }
}

#[test]
fn human_readable_search_and_read_outputs_use_plain_layout() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let search_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "Rust and SQLite",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let search_text = String::from_utf8(search_output).unwrap();
    assert!(search_text.contains("消息搜索: Rust and SQLite"));
    assert!(search_text.contains("命中条数: 1"));
    assert!(search_text.contains("session-alpha"));
    assert!(search_text.contains("assistant"));

    let search_lines = search_text.lines().collect::<Vec<_>>();
    assert_eq!(search_lines[0], "消息搜索: Rust and SQLite");
    assert_eq!(search_lines[1], "命中条数: 1");
    assert!(search_lines[2].starts_with("耗时: "));
    assert!(search_lines[3].starts_with("检索范围: "));
    assert!(search_lines[4].contains("session-alpha"));
    assert!(search_lines[5].starts_with("  来源: "));

    let thread_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "read",
            "session-alpha",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let thread_text = String::from_utf8(thread_output).unwrap();
    assert!(thread_text.contains("线程: session-alpha"));
    assert!(thread_text.contains("标题:"));
    assert!(thread_text.contains("消息数:"));
    assert!(thread_text.contains("assistant"));
    assert!(thread_text.contains("耗时:"));

    let thread_lines = thread_text.lines().collect::<Vec<_>>();
    assert_eq!(thread_lines[0], "线程: session-alpha");
    assert!(thread_lines[1].starts_with("标题: "));
    assert!(thread_lines[2].starts_with("消息数: "));
    assert!(thread_lines[3].starts_with("事件数: "));
    assert!(thread_lines[4].starts_with("- "));
    assert!(thread_lines.last().unwrap().starts_with("耗时: "));
}

#[test]
fn human_readable_messages_and_events_read_place_duration_after_count() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let messages_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "read",
            "session-alpha",
            "--limit",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let messages_text = String::from_utf8(messages_output).unwrap();
    let message_lines = messages_text.lines().collect::<Vec<_>>();
    assert_eq!(message_lines[0], "消息线程: session-alpha");
    assert_eq!(message_lines[1], "返回条数: 2");
    assert!(message_lines[2].starts_with("耗时: "));
    assert!(message_lines[3].starts_with("- "));

    let events_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "events",
            "read",
            "session-alpha",
            "--limit",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let events_text = String::from_utf8(events_output).unwrap();
    let event_lines = events_text.lines().collect::<Vec<_>>();
    assert_eq!(event_lines[0], "事件线程: session-alpha");
    assert_eq!(event_lines[1], "返回条数: 2");
    assert!(event_lines[2].starts_with("耗时: "));
    assert!(event_lines[3].starts_with("- "));
}

#[test]
fn threads_search_uses_aggregate_content() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "websocket reconnect",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "threads.search");
    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["session_id"], "session-beta");
    assert_native_handoff(&json["results"][0], "session-beta");
}

#[test]
fn punctuation_query_stays_on_fts_instead_of_scanning_like() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "alpha-repo",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["count"], 1);
    assert_eq!(json["search"]["backend"], "fts");
    assert_eq!(json["results"][0]["session_id"], "session-alpha");
}

#[test]
fn events_search_returns_matching_results() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "events",
            "search",
            "agent_reasoning",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "events.search");
    assert_eq!(json["count"], 2);
    assert_eq!(json["search"]["backend"], "fts");
    assert_eq!(json["search"]["query_mode"], "literal");
    assert_eq!(json["search"]["ranking"], "bm25");
    assert_eq!(json["results"][0]["session_id"], "session-beta");
    assert_eq!(json["results"][0]["event_type"], "agent_reasoning");
    assert_native_handoff(&json["results"][0], "session-beta");
    assert_eq!(
        json["results"][0]["explain"]["matched_fields"],
        serde_json::json!(["event_type"])
    );
}

#[test]
fn human_readable_events_search_uses_plain_layout() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "events",
            "search",
            "Planning CLI surface",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], "事件搜索: Planning CLI surface");
    assert_eq!(lines[1], "命中条数: 1");
    assert!(lines[2].starts_with("耗时: "));
    assert!(lines[3].starts_with("检索范围: "));
    assert!(lines[4].contains("session-alpha"));
    assert!(lines[4].contains("agent_reasoning"));
    assert!(lines[5].starts_with("  来源: "));
}

#[test]
fn messages_search_supports_role_and_session_filters() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "CLI",
            "--limit",
            "5",
            "--role",
            "user",
            "--session",
            "session-alpha",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["session_id"], "session-alpha");
    assert_eq!(json["results"][0]["role"], "user");
    assert_eq!(json["filters"]["role"], "user");
    assert_eq!(json["filters"]["session"], "session-alpha");
}

#[test]
fn threads_search_supports_cwd_path_and_time_filters() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "The",
            "--limit",
            "5",
            "--cwd",
            "alpha-repo",
            "--path",
            "session-alpha",
            "--since",
            "2026-04-12T09:00:00Z",
            "--until",
            "2026-04-12T10:30:00Z",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["session_id"], "session-alpha");
    assert_eq!(json["filters"]["cwd"], "alpha-repo");
    assert_eq!(json["filters"]["path"], "session-alpha");
    assert_eq!(json["filters"]["since"], "2026-04-12T09:00:00Z");
    assert_eq!(json["filters"]["until"], "2026-04-12T10:30:00Z");
}

#[test]
fn events_search_supports_event_type_session_and_time_filters() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "events",
            "search",
            "agent",
            "--limit",
            "5",
            "--event-type",
            "agent_reasoning",
            "--session",
            "session-beta",
            "--since",
            "2026-04-12T10:30:00Z",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["session_id"], "session-beta");
    assert_eq!(json["results"][0]["event_type"], "agent_reasoning");
    assert_eq!(json["filters"]["event_type"], "agent_reasoning");
    assert_eq!(json["filters"]["session"], "session-beta");
    assert_eq!(json["filters"]["since"], "2026-04-12T10:30:00Z");
}

#[test]
fn search_normalizes_punctuation_in_message_and_thread_queries() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let messages_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "Rust, SQLite",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let messages_json: Value = serde_json::from_slice(&messages_output).unwrap();
    assert_eq!(messages_json["command"], "messages.search");
    assert_eq!(messages_json["count"], 1);
    assert_eq!(messages_json["results"][0]["session_id"], "session-alpha");

    let threads_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "CLI, search",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let threads_json: Value = serde_json::from_slice(&threads_output).unwrap();
    assert_eq!(threads_json["command"], "threads.search");
    assert_eq!(threads_json["count"], 1);
    assert_eq!(threads_json["results"][0]["session_id"], "session-alpha");
}

#[test]
fn search_preserves_symbol_bearing_queries() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let messages_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "C++",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let messages_json: Value = serde_json::from_slice(&messages_output).unwrap();
    assert_eq!(messages_json["count"], 1);
    assert_eq!(messages_json["results"][0]["session_id"], "session-alpha");
    assert!(messages_json["results"][0]["snippet"]
        .as_str()
        .unwrap()
        .contains("C++"));

    let threads_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "C++",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let threads_json: Value = serde_json::from_slice(&threads_output).unwrap();
    assert_eq!(threads_json["count"], 1);
    assert_eq!(threads_json["results"][0]["session_id"], "session-alpha");
}

#[test]
fn search_expands_slash_queries_without_breaking_literal_symbols() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let threads_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "search",
            "CLI/search",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let threads_json: Value = serde_json::from_slice(&threads_output).unwrap();
    assert_eq!(threads_json["count"], 1);
    assert_eq!(threads_json["results"][0]["session_id"], "session-alpha");
    assert_eq!(threads_json["search"]["query_mode"], "expanded");
    assert_eq!(threads_json["search"]["normalized_query"], "CLI search");
    assert_eq!(
        threads_json["search"]["normalized_terms"],
        serde_json::json!(["CLI", "search"])
    );
    assert_eq!(
        threads_json["results"][0]["explain"]["matched_fields"],
        serde_json::json!(["title", "aggregate_text"])
    );
    assert_eq!(threads_json["results"][0]["explain"]["matched_terms"], 2);
    assert_eq!(
        threads_json["results"][0]["explain"]["literal_match"],
        false
    );
}

#[test]
fn search_escapes_like_wildcards_in_literal_queries() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let messages_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "search",
            "%",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let messages_json: Value = serde_json::from_slice(&messages_output).unwrap();
    assert_eq!(messages_json["count"], 0);
}

#[test]
fn thread_message_and_event_reads_honor_limits() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let thread_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "read",
            "session-alpha",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let thread_json: Value = serde_json::from_slice(&thread_output).unwrap();
    assert_eq!(thread_json["thread"]["session_id"], "session-alpha");
    assert!(thread_json
        .get("duration_ms")
        .and_then(Value::as_u64)
        .is_some());
    assert_eq!(thread_json["messages"].as_array().unwrap().len(), 1);
    assert_eq!(thread_json["messages"][0]["role"], "assistant");

    let messages_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "messages",
            "read",
            "session-alpha",
            "--limit",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let messages_json: Value = serde_json::from_slice(&messages_output).unwrap();
    assert!(messages_json
        .get("duration_ms")
        .and_then(Value::as_u64)
        .is_some());
    assert_eq!(messages_json["count"], 2);

    let events_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "events",
            "read",
            "session-alpha",
            "--limit",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let events_json: Value = serde_json::from_slice(&events_output).unwrap();
    assert_eq!(events_json["command"], "events.read");
    assert!(events_json
        .get("duration_ms")
        .and_then(Value::as_u64)
        .is_some());
    assert_eq!(events_json["count"], 2);
}

#[test]
fn threads_context_outputs_budgeted_handoff() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "context",
            "session-alpha",
            "--budget",
            "900",
            "--messages",
            "3",
            "--events",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.len() <= 1100);
    assert!(text.contains("# Codex Thread Context"));
    assert!(text.contains("session-alpha"));
    assert!(text.contains("## Recent Messages"));
    assert!(text.contains("Please build a CLI for thread search"));
    assert!(text.contains("## Execution Evidence"));
    assert!(text.contains("function_call"));
    assert!(text.contains("agent_reasoning"));
    assert!(text.contains("## Resume Pointers"));

    let json_output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "context",
            "session-alpha",
            "--budget",
            "900",
            "--messages",
            "3",
            "--events",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&json_output).unwrap();
    assert_eq!(json["command"], "threads.context");
    assert_eq!(json["ok"], true);
    assert_eq!(json["thread"]["session_id"], "session-alpha");
    assert_native_handoff(&json, "session-alpha");
    assert!(json["budget"]["used"].as_u64().unwrap() <= json["budget"]["limit"].as_u64().unwrap());
    assert!(!json["messages"].as_array().unwrap().is_empty());
    assert!(!json["events"].as_array().unwrap().is_empty());
}

#[test]
fn threads_context_can_omit_events() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "context",
            "session-alpha",
            "--budget",
            "900",
            "--messages",
            "2",
            "--events",
            "3",
            "--no-events",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "threads.context");
    assert!(json["events"].as_array().unwrap().is_empty());
    assert!(!json["text"]
        .as_str()
        .unwrap()
        .contains("## Execution Evidence"));
    assert!(json["text"]
        .as_str()
        .unwrap()
        .contains("## Recent Messages"));
}

#[test]
fn threads_context_keeps_resume_pointers_with_tight_budget() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "context",
            "session-alpha",
            "--budget",
            "650",
            "--messages",
            "3",
            "--events",
            "20",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.len() <= 750);
    assert!(text.contains("## Resume Pointers"));
    assert!(text.contains("codex-threads threads read session-alpha --limit 20"));
    assert!(text.contains("codex-threads events read session-alpha --limit 20"));
}

#[test]
fn threads_context_rejects_budget_smaller_than_resume_pointers() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "threads",
            "context",
            "session-alpha",
            "--budget",
            "200",
        ])
        .assert()
        .failure()
        .get_output()
        .clone();

    let rendered = format!(
        "{}{}",
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap()
    );
    assert!(rendered.contains("--budget"));
    assert!(rendered.contains("Resume Pointers"));
}
