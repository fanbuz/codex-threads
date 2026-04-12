use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn version_flag_reports_0_0_1() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("codex-threads 0.0.1"));
}

#[test]
fn status_reports_cli_version_in_text_and_json() {
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
        .stdout(predicate::str::contains("\"cli_version\":\"0.0.1\""))
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["cli_version"], "0.0.1");

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["--index-dir", index_dir.to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("索引状态"))
        .stdout(predicate::str::contains("CLI 版本: 0.0.1"))
        .stdout(predicate::str::contains("索引文件:"))
        .stdout(predicate::str::contains("FTS5 可用:"));
}
