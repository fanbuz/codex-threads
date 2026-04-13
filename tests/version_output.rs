use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::tempdir;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

mod common;

#[test]
fn version_flag_reports_0_0_3() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("codex-threads 0.0.3"));
}

#[test]
fn status_reports_cli_version_in_text_and_json_without_timing() {
    let tmp = tempdir().unwrap();
    let index_dir = tmp.path().join("index");

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
        .stdout(predicate::str::contains("\"cli_version\":\"0.0.3\""))
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert!(json.get("duration_ms").is_none());
    assert_eq!(json["cli_version"], "0.0.3");
    assert_eq!(json["status"]["sync_lock"]["state"], "idle");

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["--index-dir", index_dir.to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("索引状态"))
        .stdout(predicate::str::contains("CLI 版本: 0.0.3"))
        .stdout(predicate::str::contains("索引文件:"))
        .stdout(predicate::str::contains("FTS5 可用:"))
        .stdout(predicate::str::contains("同步锁: 空闲"))
        .stdout(predicate::str::contains("耗时:").not());
}

#[test]
fn status_reports_running_sync_lock_in_text_and_json() {
    let tmp = tempdir().unwrap();
    let index_dir = tmp.path().join("index");
    let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
    common::write_sync_lock(&index_dir, 4242, &now, &now);

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
    assert_eq!(json["status"]["sync_lock"]["state"], "running");
    assert_eq!(json["status"]["sync_lock"]["pid"], 4242);

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["--index-dir", index_dir.to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("同步锁: 运行中"))
        .stdout(predicate::str::contains("锁 PID: 4242"));
}

#[test]
fn doctor_reports_healthy_index_in_text_and_json() {
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

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "doctor");
    assert_eq!(json["doctor"]["status"], "healthy");
    assert_eq!(json["doctor"]["issues"].as_array().unwrap().len(), 0);
    assert_eq!(
        json["doctor"]["repaired_actions"].as_array().unwrap().len(),
        0
    );
    assert_eq!(
        json["doctor"]["status_summary"]["sync_lock"]["state"],
        "idle"
    );

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["--index-dir", index_dir.to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("索引健康检查"))
        .stdout(predicate::str::contains("健康状态: 正常"))
        .stdout(predicate::str::contains("发现问题: 0"))
        .stdout(predicate::str::contains("修复动作: 0"));
}

#[test]
fn doctor_detects_repairable_state_files_and_can_clean_them() {
    let tmp = tempdir().unwrap();
    let index_dir = tmp.path().join("index");
    let stale = (OffsetDateTime::now_utc() - time::Duration::hours(1))
        .format(&Rfc3339)
        .unwrap();
    let lock_path = common::write_sync_lock(&index_dir, 4242, &stale, &stale);
    let resume_path = common::write_invalid_resume_state(&index_dir);
    let refresh_path = common::write_invalid_refresh_state(&index_dir);

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["doctor"]["status"], "attention");
    assert_eq!(json["doctor"]["issues"].as_array().unwrap().len(), 3);
    assert!(json["doctor"]["recommendation"]
        .as_str()
        .unwrap()
        .contains("--repair"));

    let repaired = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "doctor",
            "--repair",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let repaired_json: Value = serde_json::from_slice(&repaired).unwrap();
    assert_eq!(repaired_json["doctor"]["status"], "healthy");
    assert_eq!(
        repaired_json["doctor"]["issues"].as_array().unwrap().len(),
        0
    );
    assert_eq!(
        repaired_json["doctor"]["repaired_actions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!lock_path.exists());
    assert!(!resume_path.exists());
    assert!(!refresh_path.exists());
}

#[test]
fn doctor_detects_index_count_drift_without_trying_to_auto_repair_it() {
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

    let db_path = index_dir.join("threads.sqlite3");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "UPDATE threads SET message_count = 999 WHERE session_id = 'session-alpha'",
        [],
    )
    .unwrap();

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--index-dir",
            index_dir.to_str().unwrap(),
            "doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["doctor"]["status"], "attention");
    assert!(json["doctor"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "thread_message_count_mismatch"));
    assert_eq!(
        json["doctor"]["repaired_actions"].as_array().unwrap().len(),
        0
    );
    assert!(json["doctor"]["recommendation"]
        .as_str()
        .unwrap()
        .contains("重新同步"));
}
