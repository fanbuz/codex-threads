mod common;

use tempfile::tempdir;

#[test]
fn parser_extracts_thread_messages_and_events() {
    let tmp = tempdir().unwrap();
    let (alpha_path, _) = common::write_fixture_sessions(tmp.path());

    let parsed = codex_threads::parser::parse_session_file(&alpha_path).unwrap();

    assert_eq!(parsed.session_id, "session-alpha");
    assert_eq!(parsed.cwd.as_deref(), Some("/workspace/alpha-repo"));
    assert_eq!(parsed.messages.len(), 3);
    assert_eq!(parsed.events.len(), 4);
    assert!(parsed.title.contains("alpha-repo"));
    assert!(parsed.aggregate_text.contains("Rust and SQLite"));
    assert!(parsed.aggregate_text.contains("C++"));
}

#[test]
fn parser_ignores_developer_context_for_default_thread_text() {
    let tmp = tempdir().unwrap();
    let session_path = tmp.path().join("latest-codex-session.jsonl");
    std::fs::write(
        &session_path,
        [
            r#"{"timestamp":"2026-06-17T02:44:05Z","type":"session_meta","payload":{"id":"session-latest","timestamp":"2026-06-17T02:44:05Z","cwd":"/workspace/code-threads","originator":"codex_cli_rs","cli_version":"0.140.0-alpha.19"}}"#,
            r##"{"timestamp":"2026-06-17T02:44:06Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions\n<INSTRUCTIONS>\nGlobal issue writing memory\n</INSTRUCTIONS>\n<environment_context>\nDo not use this as a user-facing thread title."}]}}"##,
            r#"{"timestamp":"2026-06-17T02:44:07Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<permissions instructions>\nFilesystem sandboxing defines which files can be read or written."}]}}"#,
            r#"{"timestamp":"2026-06-17T02:44:08Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"请 review 一下最新 Codex 兼容性"}]}}"#,
            r#"{"timestamp":"2026-06-17T02:44:09Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"我会先检查 parser 和真实会话样本。"}]}}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let parsed = codex_threads::parser::parse_session_file(&session_path).unwrap();

    assert_eq!(parsed.messages.len(), 2);
    assert!(parsed
        .messages
        .iter()
        .all(|message| message.role != "developer"));
    assert!(parsed.title.contains("请 review 一下最新 Codex 兼容性"));
    assert!(!parsed.title.contains("AGENTS.md"));
    assert!(!parsed.aggregate_text.contains("environment_context"));
    assert!(!parsed.aggregate_text.contains("Filesystem sandboxing"));
    assert!(!parsed.aggregate_text.contains("Do not use this"));
}

#[test]
fn parser_does_not_index_encrypted_reasoning_as_searchable_text() {
    let tmp = tempdir().unwrap();
    let session_path = tmp.path().join("latest-codex-events.jsonl");
    std::fs::write(
        &session_path,
        [
            r#"{"timestamp":"2026-06-17T03:20:00Z","type":"session_meta","payload":{"id":"session-events","timestamp":"2026-06-17T03:20:00Z","cwd":"/workspace/code-threads"}}"#,
            r#"{"timestamp":"2026-06-17T03:20:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"验证事件证据保留"}]}}"#,
            r#"{"timestamp":"2026-06-17T03:20:02Z","type":"response_item","payload":{"type":"reasoning","encrypted_content":"ciphertext-should-not-be-indexed"}}"#,
            r#"{"timestamp":"2026-06-17T03:20:03Z","type":"response_item","payload":{"type":"function_call_output","output":"cargo test passed: 64 tests"}}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let parsed = codex_threads::parser::parse_session_file(&session_path).unwrap();

    assert!(parsed.events.iter().any(|event| {
        event.event_type == "function_call_output"
            && event.summary.contains("cargo test passed: 64 tests")
    }));
    assert!(parsed
        .events
        .iter()
        .any(|event| { event.event_type == "reasoning" && event.summary.is_empty() }));
    assert!(!parsed.aggregate_text.contains("encrypted_content"));
    assert!(!parsed
        .aggregate_text
        .contains("ciphertext-should-not-be-indexed"));
}
