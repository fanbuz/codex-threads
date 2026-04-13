use assert_cmd::Command;
use predicates::prelude::*;
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
