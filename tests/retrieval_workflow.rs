use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::{tempdir, TempDir};

fn fixture() -> TempDir {
    let tmp = tempdir().unwrap();
    let sessions = tmp.path().join("sessions");
    fs::create_dir(&sessions).unwrap();
    for name in ["retrieval-zh", "retrieval-en", "retrieval-noise"] {
        fs::copy(
            format!("tests/fixtures/retrieval/{name}.jsonl"),
            sessions.join(format!("{name}.jsonl")),
        )
        .unwrap();
    }
    tmp
}

fn command(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("codex-threads").unwrap();
    cmd.arg("--sessions-dir")
        .arg(root.join("sessions"))
        .arg("--index-dir")
        .arg(root.join("index"));
    cmd
}

fn json(root: &Path, args: &[&str]) -> Value {
    let output = command(root)
        .arg("--json")
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn keyword_discovery_and_offline_evidence_are_reproducible() {
    let tmp = fixture();
    let root = tmp.path();
    json(root, &["sync"]);
    for (domain, query, session, field, expected) in [
        (
            "messages",
            "断线重连",
            "retrieval-zh",
            "text",
            "请排查中文检索中的断线重连问题",
        ),
        (
            "messages",
            "websocket reconnect",
            "retrieval-en",
            "text",
            "Investigate websocket reconnect policy",
        ),
        (
            "messages",
            "reconnect_policy",
            "retrieval-zh",
            "text",
            "修复 reconnect_policy 并保留精确关键词。",
        ),
        (
            "events",
            "Error: ECONNRESET",
            "retrieval-zh",
            "summary",
            "Error: ECONNRESET at reconnect_policy",
        ),
        (
            "events",
            "Error: retry_budget exceeded",
            "retrieval-en",
            "summary",
            "Error: retry_budget exceeded",
        ),
    ] {
        let hits = json(root, &[domain, "search", query]);
        assert_eq!(hits["count"], 1, "{query}: {hits}");
        let hit = &hits["results"][0];
        assert_eq!(hit["session_id"], session);
        assert!(hit[field].as_str().unwrap().contains(expected));
        assert_eq!(hit["explain"]["literal_match"], true);
        assert_eq!(hit["handoff"]["verification"], "required");
        let source = fs::read_to_string(hit["source"]["path"].as_str().unwrap()).unwrap();
        assert!(source.contains(expected));
        let read = json(root, &[domain, "read", session]);
        assert!(read[domain].as_array().unwrap().iter().any(|record| {
            record[field]
                .as_str()
                .is_some_and(|text| text.contains(expected))
        }));
        let text = command(root)
            .args([domain, "search", query])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(String::from_utf8(text)
            .unwrap()
            .contains(hit["source"]["path"].as_str().unwrap()));
    }
    // Indexed evidence remains readable when the source directory is unavailable.
    fs::rename(root.join("sessions"), root.join("sessions-offline")).unwrap();
    assert_eq!(
        json(root, &["threads", "read", "retrieval-zh"])["thread"]["session_id"],
        "retrieval-zh"
    );
    assert_eq!(json(root, &["events", "search", "ECONNRESET"])["count"], 1);
}

#[test]
fn fixed_sample_filters_exclude_unrelated_records() {
    let tmp = fixture();
    let root = tmp.path();
    json(root, &["sync"]);
    for (args, count) in [
        (vec!["messages", "search", "断线重连", "--role", "user"], 1),
        (
            vec!["messages", "search", "断线重连", "--role", "assistant"],
            0,
        ),
        (
            vec![
                "messages",
                "search",
                "reconnect",
                "--session",
                "retrieval-en",
            ],
            1,
        ),
        (
            vec![
                "messages",
                "search",
                "reconnect",
                "--since",
                "2026-09-02T00:00:00Z",
                "--until",
                "2026-09-02T23:59:59Z",
            ],
            1,
        ),
        (
            vec![
                "events",
                "search",
                "ECONNRESET",
                "--event-type",
                "function_call_output",
            ],
            1,
        ),
        (
            vec![
                "events",
                "search",
                "ECONNRESET",
                "--event-type",
                "function_call",
            ],
            0,
        ),
        (
            vec![
                "events",
                "search",
                "ECONNRESET",
                "--session",
                "retrieval-en",
            ],
            0,
        ),
    ] {
        let result = json(root, &args);
        assert_eq!(result["count"], count, "{args:?}: {result}");
    }
}

#[test]
fn search_distinguishes_index_state_without_claiming_corpus_coverage() {
    let tmp = fixture();
    let root = tmp.path();
    for domain in ["messages", "events", "threads"] {
        let result = json(root, &[domain, "search", "missing"]);
        assert_eq!(result["coverage"]["state"], "empty");
        assert_eq!(result["coverage"]["corpus_verified"], false);
        let text = command(root)
            .args([domain, "search", "missing"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(String::from_utf8(text).unwrap().contains("索引中没有会话"));
    }
    json(root, &["sync", "--path", "retrieval-zh"]);
    for domain in ["messages", "events", "threads"] {
        let partial = json(root, &[domain, "search", "missing"]);
        assert_eq!(partial["coverage"]["state"], "indexed");
        assert_eq!(partial["coverage"]["indexed_threads"], 1);
        assert_eq!(partial["coverage"]["corpus_verified"], false);
        assert_eq!(partial["count"], 0);
        let absent = json(
            root,
            &[domain, "search", "missing", "--session", "retrieval-en"],
        );
        assert_eq!(absent["coverage"]["state"], "session_not_indexed");
        assert_eq!(absent["coverage"]["requested_session_indexed"], false);
        let present = json(
            root,
            &[domain, "search", "missing", "--session", "retrieval-zh"],
        );
        assert_eq!(present["coverage"]["state"], "indexed");
        assert_eq!(present["coverage"]["requested_session_indexed"], true);
    }
    let conn = Connection::open(root.join("index/threads.sqlite3")).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('rebuild_required', '1')",
        [],
    )
    .unwrap();
    for domain in ["messages", "events", "threads"] {
        assert_eq!(
            json(root, &[domain, "search", "missing"])["coverage"]["state"],
            "rebuild_required"
        );
    }
    json(root, &["sync", "--force"]);
    let result = json(root, &["messages", "search", "missing"]);
    assert_eq!(result["coverage"]["state"], "indexed");
    assert_eq!(result["coverage"]["indexed_threads"], 3);
    assert_eq!(result["coverage"]["corpus_verified"], false);
}

#[test]
fn context_budget_describes_and_limits_utf8_text_only() {
    let tmp = fixture();
    let root = tmp.path();
    json(root, &["sync"]);
    let result = json(
        root,
        &["threads", "context", "retrieval-zh", "--budget", "650"],
    );
    assert_eq!(result["budget"]["unit"], "utf8_bytes");
    assert_eq!(result["budget"]["applies_to"], "text");
    let text = result["text"].as_str().unwrap();
    assert_eq!(
        result["budget"]["used"].as_u64().unwrap() as usize,
        text.len()
    );
    assert!(text.len() <= 650);
    assert!(text.contains("Resume Pointers"));
    assert!(result["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["text"].as_str().unwrap().contains("断线重连")));
}
