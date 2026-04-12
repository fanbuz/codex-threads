mod common;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn sync_reports_indexed_files() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "sync");
    assert_eq!(json["stats"]["scanned_files"], 2);
    assert_eq!(json["stats"]["indexed_files"], 2);
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["stats"]["messages"], 4);
}

#[test]
fn sync_text_output_uses_plain_lines() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "sync",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("同步完成"))
        .stdout(predicates::str::contains("会话目录:"))
        .stdout(predicates::str::contains("索引目录:"))
        .stdout(predicates::str::contains("扫描文件:"))
        .stdout(predicates::str::contains("线程总数:"));
}

#[test]
fn sync_is_incremental_for_unchanged_and_changed_files() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());
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

    let second = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let second_json: Value = serde_json::from_slice(&second).unwrap();
    assert_eq!(second_json["stats"]["indexed_files"], 0);
    assert_eq!(second_json["stats"]["skipped_files"], 2);

    common::append_alpha_message(&alpha_path);

    let third = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let third_json: Value = serde_json::from_slice(&third).unwrap();
    assert_eq!(third_json["stats"]["indexed_files"], 1);
    assert_eq!(third_json["stats"]["skipped_files"], 1);
    assert_eq!(third_json["stats"]["messages"], 5);
}

#[test]
fn sync_replaces_existing_session_when_same_session_id_moves_to_new_path() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());
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

    let moved_dir = sessions_dir.join("2026/04/13");
    std::fs::create_dir_all(&moved_dir).unwrap();
    let moved_alpha = moved_dir.join("rollout-2026-04-13T10-00-00-session-alpha.jsonl");
    std::fs::copy(&alpha_path, &moved_alpha).unwrap();
    std::fs::remove_file(&alpha_path).unwrap();

    let output = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["stats"]["removed_files"], 1);
}

#[test]
fn sync_tolerates_invalid_jsonl_and_reports_failed_files() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let invalid_path = common::write_invalid_session(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "sync");
    assert_eq!(json["partial"], true);
    assert_eq!(json["stats"]["scanned_files"], 3);
    assert_eq!(json["stats"]["indexed_files"], 2);
    assert_eq!(json["stats"]["failed_files"], 1);
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["failures"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["failures"][0]["path"],
        invalid_path.to_string_lossy().to_string()
    );
    assert!(json["failures"][0]["error"]
        .as_str()
        .unwrap()
        .contains("invalid JSON"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--index-dir",
            index_dir.to_str().unwrap(),
            "sync",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("同步完成（部分失败）"))
        .stdout(predicates::str::contains("失败文件: 1"))
        .stdout(predicates::str::contains("session-invalid.jsonl"));
}

#[test]
fn sync_keeps_previous_index_when_existing_file_temporarily_breaks() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());
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

    common::overwrite_with_invalid_json(&alpha_path);

    let output = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["partial"], true);
    assert_eq!(json["stats"]["indexed_files"], 0);
    assert_eq!(json["stats"]["skipped_files"], 1);
    assert_eq!(json["stats"]["failed_files"], 1);
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["stats"]["messages"], 4);
    assert_eq!(
        json["failures"][0]["path"],
        alpha_path.to_string_lossy().to_string()
    );
}
