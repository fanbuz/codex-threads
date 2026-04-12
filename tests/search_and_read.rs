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
    assert_eq!(json["results"][0]["session_id"], "session-alpha");
    assert_eq!(json["results"][0]["role"], "assistant");
}

#[test]
fn human_readable_search_and_read_outputs_use_plain_layout() {
    let (_tmp, sessions_dir, index_dir) = seed_index();

    Command::cargo_bin("codex-threads")
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
        .stdout(predicates::str::contains("消息搜索: Rust and SQLite"))
        .stdout(predicates::str::contains("命中条数: 1"))
        .stdout(predicates::str::contains("session-alpha"))
        .stdout(predicates::str::contains("assistant"))
        .stdout(predicates::str::contains("耗时:"));

    Command::cargo_bin("codex-threads")
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
        .stdout(predicates::str::contains("线程: session-alpha"))
        .stdout(predicates::str::contains("标题:"))
        .stdout(predicates::str::contains("消息数:"))
        .stdout(predicates::str::contains("assistant"))
        .stdout(predicates::str::contains("耗时:"));
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
