use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn top_level_help_shows_command_descriptions() {
    Command::cargo_bin("codex-threads")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI 版本: 0.0.3"))
        .stdout(predicate::str::contains(
            "sync      增量扫描会话文件并更新索引",
        ))
        .stdout(predicate::str::contains("threads   搜索和读取线程"))
        .stdout(predicate::str::contains("messages  搜索和读取消息"))
        .stdout(predicate::str::contains("events    搜索和读取事件记录"));
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
        .stdout(predicate::str::contains("--budget-files <BUDGET_FILES>"));
}
