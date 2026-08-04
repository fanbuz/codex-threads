mod common;

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::tempdir;

fn sync_fixture(root: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let _ = common::write_fixture_sessions(root);
    let sessions_dir = root.join("sessions");
    let index_dir = root.join("index");
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
    (sessions_dir, index_dir)
}

fn mark_index_as_legacy(index_dir: &std::path::Path) {
    let conn = Connection::open(index_dir.join("threads.sqlite3")).unwrap();
    conn.execute_batch(
        r#"
        DROP TABLE threads_fts;
        DROP TABLE messages_fts;
        DROP TABLE events_fts;
        CREATE VIRTUAL TABLE threads_fts
        USING fts5(session_id UNINDEXED, title, cwd, path, aggregate_text);
        CREATE VIRTUAL TABLE messages_fts
        USING fts5(session_id UNINDEXED, role UNINDEXED, text);
        CREATE VIRTUAL TABLE events_fts
        USING fts5(session_id UNINDEXED, event_type, summary);
        UPDATE index_meta SET value = '1' WHERE key = 'format_version';
        "#,
    )
    .unwrap();
}

#[test]
fn format_v2_uses_contentless_fts_and_reports_current_state() {
    let tmp = tempdir().unwrap();
    let (_sessions_dir, index_dir) = sync_fixture(tmp.path());
    let conn = Connection::open(index_dir.join("threads.sqlite3")).unwrap();
    let fts_sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'threads_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(fts_sql.contains("contentless_delete=1"));

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"]["index_format_version"], 2);
    assert_eq!(json["status"]["rebuild_required"], false);
}

#[test]
fn legacy_index_is_compacted_and_requires_one_full_sync() {
    let tmp = tempdir().unwrap();
    let (sessions_dir, index_dir) = sync_fixture(tmp.path());
    mark_index_as_legacy(&index_dir);

    let migrated = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let migrated_json: Value = serde_json::from_slice(&migrated).unwrap();
    assert_eq!(migrated_json["status"]["threads"], 0);
    assert_eq!(migrated_json["status"]["rebuild_required"], true);

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

    let rebuilt = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rebuilt_json: Value = serde_json::from_slice(&rebuilt).unwrap();
    assert_eq!(rebuilt_json["status"]["threads"], 2);
    assert_eq!(rebuilt_json["status"]["rebuild_required"], false);
}

#[test]
fn budgeted_legacy_rebuild_clears_rebuild_marker_after_final_batch() {
    let tmp = tempdir().unwrap();
    let (sessions_dir, index_dir) = sync_fixture(tmp.path());
    mark_index_as_legacy(&index_dir);

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success();

    for _ in 0..2 {
        Command::cargo_bin("codex-threads")
            .unwrap()
            .args([
                "--json",
                "--sessions-dir",
                sessions_dir.to_str().unwrap(),
                "--index-dir",
                index_dir.to_str().unwrap(),
                "sync",
                "--budget-files",
                "1",
            ])
            .assert()
            .success();
    }

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"]["threads"], 2);
    assert_eq!(json["status"]["rebuild_required"], false);
    assert!(!index_dir.join("sync.resume.json").exists());
}

#[test]
fn scoped_budgeted_sync_does_not_clear_legacy_rebuild_marker() {
    let tmp = tempdir().unwrap();
    let (sessions_dir, index_dir) = sync_fixture(tmp.path());
    mark_index_as_legacy(&index_dir);

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success();

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "sync",
            "--recent",
            "1",
            "--budget-files",
            "1",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "status",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"]["threads"], 1);
    assert_eq!(json["status"]["rebuild_required"], true);
}
