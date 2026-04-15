use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn top_level_help_shows_command_descriptions() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI 版本: 0.0.4"))
        .stdout(predicate::str::contains(
            "--enable-experimentals <FEATURES>",
        ))
        .stdout(predicate::str::is_match(r"(?m)^\s*sync\s+增量扫描会话文件并更新索引$").unwrap())
        .stdout(
            predicate::str::is_match(r"(?m)^\s*doctor\s+检查索引健康状态，并可选择修复安全问题$")
                .unwrap(),
        )
        .stdout(
            predicate::str::is_match(r"(?m)^\s*experimental\s+实验性能力，默认关闭，需显式开启$")
                .unwrap(),
        )
        .stdout(predicate::str::is_match(r"(?m)^\s*threads\s+搜索和读取线程$").unwrap())
        .stdout(predicate::str::is_match(r"(?m)^\s*messages\s+搜索和读取消息$").unwrap())
        .stdout(predicate::str::is_match(r"(?m)^\s*events\s+搜索和读取事件记录$").unwrap());
}

#[test]
fn nested_help_shows_subcommand_descriptions() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["threads", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "search  按标题、路径和聚合内容搜索线程",
        ))
        .stdout(predicate::str::contains("read    读取指定线程"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["messages", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "search  在所有历史消息中搜索关键词",
        ))
        .stdout(predicate::str::contains("read    读取指定线程里的消息"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["events", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "search  在所有历史事件中搜索关键词",
        ))
        .stdout(predicate::str::contains("read    读取指定线程里的事件记录"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["experimental", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "restore-app-thread  将指定线程恢复到 Codex App 本地线程视图",
        ));
}

#[test]
fn search_help_shows_filter_options() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["messages", "search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--since <SINCE>"))
        .stdout(predicate::str::contains("--until <UNTIL>"))
        .stdout(predicate::str::contains("--session <SESSION>"))
        .stdout(predicate::str::contains("--role <ROLE>"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["threads", "search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--cwd <CWD>"))
        .stdout(predicate::str::contains("--path <PATH>"))
        .stdout(predicate::str::contains("--since <SINCE>"))
        .stdout(predicate::str::contains("--until <UNTIL>"))
        .stdout(predicate::str::contains("--session <SESSION>"));

    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["events", "search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--event-type <EVENT_TYPE>"))
        .stdout(predicate::str::contains("--since <SINCE>"))
        .stdout(predicate::str::contains("--until <UNTIL>"))
        .stdout(predicate::str::contains("--session <SESSION>"));
}

#[test]
fn sync_help_shows_scope_options() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["sync", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--since <SINCE>"))
        .stdout(predicate::str::contains("--until <UNTIL>"))
        .stdout(predicate::str::contains("--path <PATH>"))
        .stdout(predicate::str::contains("--recent <RECENT>"))
        .stdout(predicate::str::contains("--budget-files <BUDGET_FILES>"))
        .stdout(predicate::str::contains("--cooldown <COOLDOWN>"))
        .stdout(predicate::str::contains("--force"));
}

#[test]
fn doctor_help_shows_repair_option() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--repair"));
}

#[test]
fn experimental_restore_help_shows_safety_options() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .args(["experimental", "restore-app-thread", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--codex-home <PATH>"))
        .stdout(predicate::str::contains("--pin"))
        .stdout(predicate::str::contains("--dry-run"));
}
