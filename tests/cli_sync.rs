mod common;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

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
    assert!(json.get("duration_ms").and_then(Value::as_u64).is_some());
    assert!(json.get("duration_display").is_none());
    assert_eq!(json["preflight"]["total_files"], 2);
    assert_eq!(json["preflight"]["changed_files"], 2);
    assert_eq!(json["preflight"]["recommended_action"], "sync");
    assert_eq!(json["stats"]["scanned_files"], 2);
    assert_eq!(json["stats"]["indexed_files"], 2);
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["stats"]["messages"], 6);
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
        .stdout(predicates::str::contains("同步预检"))
        .stdout(predicates::str::contains("推荐动作: 执行同步"))
        .stdout(predicates::str::contains("同步完成"))
        .stdout(predicates::str::contains("会话目录:"))
        .stdout(predicates::str::contains("索引目录:"))
        .stdout(predicates::str::contains("同步范围"))
        .stdout(predicates::str::contains("扫描文件:"))
        .stdout(predicates::str::contains("线程总数:"))
        .stdout(predicates::str::contains("耗时:"));
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
    assert_eq!(second_json["preflight"]["recommended_action"], "skip");
    assert_eq!(second_json["preflight"]["changed_files"], 0);
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
    assert_eq!(third_json["preflight"]["recommended_action"], "sync");
    assert_eq!(third_json["preflight"]["changed_files"], 1);
    assert_eq!(third_json["stats"]["indexed_files"], 1);
    assert_eq!(third_json["stats"]["skipped_files"], 1);
    assert_eq!(third_json["stats"]["messages"], 7);
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
    assert_eq!(json["stats"]["messages"], 6);
    assert_eq!(
        json["failures"][0]["path"],
        alpha_path.to_string_lossy().to_string()
    );
}

#[test]
fn sync_respects_time_scope_and_reports_scope() {
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
            "--since",
            "2026-04-12T10:30:00Z",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["scope"]["since"], "2026-04-12T10:30:00Z");
    assert_eq!(json["scope"]["candidate_files"], 1);
    assert_eq!(json["preflight"]["total_files"], 1);
    assert_eq!(json["stats"]["indexed_files"], 1);
    assert_eq!(json["stats"]["threads"], 1);
}

#[test]
fn sync_respects_path_scope_and_reports_scope() {
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
            "--path",
            "session-beta",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["scope"]["path"], "session-beta");
    assert_eq!(json["scope"]["candidate_files"], 1);
    assert_eq!(json["preflight"]["total_files"], 1);
    assert_eq!(json["stats"]["indexed_files"], 1);
    assert_eq!(json["stats"]["threads"], 1);
}

#[test]
fn sync_respects_recent_scope_and_reports_scope() {
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
            "--recent",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["scope"]["recent"], 1);
    assert_eq!(json["scope"]["candidate_files"], 1);
    assert_eq!(json["preflight"]["total_files"], 1);
    assert_eq!(json["stats"]["indexed_files"], 1);
    assert_eq!(json["stats"]["threads"], 1);
}

#[test]
fn sync_text_output_reports_scope_summary() {
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
            "--path",
            "session-beta",
            "--recent",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("同步范围"))
        .stdout(predicates::str::contains("路径过滤: session-beta"))
        .stdout(predicates::str::contains("最近活跃: 最近 1 个文件"))
        .stdout(predicates::str::contains("候选文件: 1"));
}

#[test]
fn scoped_sync_does_not_prune_out_of_scope_history() {
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
            "--path",
            "session-beta",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["scope"]["path"], "session-beta");
    assert_eq!(json["stats"]["removed_files"], 0);
    assert_eq!(json["stats"]["threads"], 2);
}

#[test]
fn sync_fails_when_active_lock_exists() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");
    let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
    common::write_sync_lock(&index_dir, 4242, &now, &now);

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
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["ok"], false);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("已有 sync 正在运行"));
}

#[test]
fn sync_recovers_stale_lock_and_runs_successfully() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");
    let stale = (OffsetDateTime::now_utc() - Duration::hours(1))
        .format(&Rfc3339)
        .unwrap();
    let lock_path = common::write_sync_lock(&index_dir, 4242, &stale, &stale);

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
    assert_eq!(json["stats"]["indexed_files"], 2);
    assert!(!lock_path.exists());
}

#[test]
fn sync_saves_resume_state_when_budget_is_hit() {
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
            "--budget-files",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    let state_path = json["resume"]["state_path"].as_str().unwrap();
    assert_eq!(json["scope"]["budget_files"], 1);
    assert_eq!(json["resume"]["state"], "saved");
    assert_eq!(json["resume"]["resumed_from_checkpoint"], false);
    assert_eq!(json["resume"]["remaining_files"], 1);
    assert_eq!(json["stats"]["threads"], 1);
    assert_eq!(json["partial"], true);
    assert!(std::path::Path::new(state_path).exists());
}

#[test]
fn sync_resumes_from_saved_state_and_clears_it_when_done() {
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
            "--budget-files",
            "1",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-threads")
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
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    let state_path = json["resume"]["state_path"].as_str().unwrap();
    assert_eq!(json["resume"]["state"], "completed");
    assert_eq!(json["resume"]["resumed_from_checkpoint"], true);
    assert_eq!(json["resume"]["remaining_files"], 0);
    assert_eq!(json["stats"]["threads"], 2);
    assert_eq!(json["partial"], false);
    assert!(!std::path::Path::new(state_path).exists());
}

#[test]
fn sync_emits_progress_updates_to_stderr_for_non_tty_runs() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let index_dir = tmp.path().join("index");
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
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
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("阶段: 扫描候选文件"));
    assert!(stderr.contains("阶段: 写入索引"));
    assert!(stderr.contains("阶段: 完成"));
    assert!(stderr.contains("2/2"));
}

#[test]
fn sync_json_response_includes_progress_summary() {
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
    assert_eq!(json["progress"]["mode"], "stderr-lines");
    assert_eq!(json["progress"]["total_files"], 2);
    assert_eq!(json["progress"]["processed_files"], 2);
    assert_eq!(json["progress"]["failed_files"], 0);
    assert_eq!(json["progress"]["phases"][0], "scanning");
    assert_eq!(json["progress"]["phases"][1], "indexing");
    assert_eq!(json["progress"]["phases"][2], "finished");
}

#[test]
fn sync_lock_fixture_writes_valid_json_for_windows_style_paths() {
    let tmp = tempdir().unwrap();
    let index_dir = tmp.path().join(r"C:\runner\threads-index");
    let lock_path = common::write_sync_lock(
        &index_dir,
        4242,
        "2026-04-13T14:00:00Z",
        "2026-04-13T14:00:00Z",
    );

    let raw = std::fs::read_to_string(lock_path).unwrap();
    let json: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["pid"], 4242);
    assert_eq!(
        json["index_path"],
        index_dir
            .join("threads.sqlite3")
            .to_string_lossy()
            .to_string()
    );
}
