use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

mod common;

#[test]
fn experimental_command_requires_explicit_enable() {
    let tmp = tempdir().unwrap();
    let _ = common::write_fixture_sessions(tmp.path());
    let codex_home = tmp.path().join("codex-home");
    let _ = common::write_codex_app_state(&codex_home);
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "experimental",
            "restore-app-thread",
            "session-alpha",
            "--codex-home",
            codex_home.to_str().unwrap(),
            "--dry-run",
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
        .contains("--enable-experimentals restore-app-thread"));
}

#[test]
fn enable_experimentals_rejects_unknown_feature_names() {
    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--enable-experimentals",
            "restore-app-thread,unknown-feature",
            "status",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("unknown-feature"));
}

#[test]
fn enable_experimentals_rejects_empty_feature_entries() {
    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--enable-experimentals",
            "restore-app-thread,,aaa",
            "status",
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("空"));
}

#[test]
fn restore_app_thread_dry_run_reports_changes_without_writing() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());
    let codex_home = tmp.path().join("codex-home");
    let (state_db_path, global_state_path) = common::write_codex_app_state(&codex_home);
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--enable-experimentals",
            "restore-app-thread",
            "experimental",
            "restore-app-thread",
            "session-alpha",
            "--codex-home",
            codex_home.to_str().unwrap(),
            "--pin",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["command"], "experimental.restore-app-thread");
    assert_eq!(json["experimental"]["feature"], "restore-app-thread");
    assert_eq!(json["experimental"]["dry_run"], true);
    assert_eq!(json["experimental"]["thread_action"], "would_insert");
    assert_eq!(json["experimental"]["pin_action"], "would_pin");
    assert_eq!(
        json["experimental"]["thread"]["rollout_path"],
        alpha_path.to_string_lossy().to_string()
    );
    assert!(json["experimental"]["backup_dir"].is_null());
    assert!(common::read_app_thread_row(&state_db_path, "session-alpha").is_none());
    assert_eq!(
        common::read_pinned_thread_ids(&global_state_path),
        vec!["already-pinned".to_string()]
    );
}

#[test]
fn restore_app_thread_writes_thread_and_pin_with_backup() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());
    let codex_home = tmp.path().join("codex-home");
    let (state_db_path, global_state_path) = common::write_codex_app_state(&codex_home);
    let sessions_dir = tmp.path().join("sessions");

    let output = Command::cargo_bin("codex-threads")
        .unwrap()
        .args([
            "--json",
            "--sessions-dir",
            sessions_dir.to_str().unwrap(),
            "--enable-experimentals",
            "restore-app-thread",
            "experimental",
            "restore-app-thread",
            "session-alpha",
            "--codex-home",
            codex_home.to_str().unwrap(),
            "--pin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["experimental"]["dry_run"], false);
    assert_eq!(json["experimental"]["thread_action"], "inserted");
    assert_eq!(json["experimental"]["pin_action"], "pinned");

    let backup_dir = json["experimental"]["backup_dir"].as_str().unwrap();
    assert!(Path::new(backup_dir).exists());
    assert!(Path::new(backup_dir).join("state_5.sqlite").exists());
    assert!(Path::new(backup_dir)
        .join(".codex-global-state.json")
        .exists());

    let row = common::read_app_thread_row(&state_db_path, "session-alpha").unwrap();
    assert_eq!(row.id, "session-alpha");
    assert_eq!(row.rollout_path, alpha_path.to_string_lossy().to_string());
    assert_eq!(
        row.title,
        "alpha-repo: Please build a CLI for thread search"
    );
    assert_eq!(row.archived, 0);

    let pinned = common::read_pinned_thread_ids(&global_state_path);
    assert!(pinned.contains(&"already-pinned".to_string()));
    assert!(pinned.contains(&"session-alpha".to_string()));
}
