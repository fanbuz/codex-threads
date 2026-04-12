mod common;

use assert_cmd::Command;
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
    assert!(search_lines[3].contains("session-alpha"));

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
    assert_eq!(event_lines[1], "返回条数: 3");
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
    assert!(lines[3].contains("session-alpha"));
    assert!(lines[3].contains("agent_reasoning"));
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
    assert_eq!(events_json["count"], 3);
}
