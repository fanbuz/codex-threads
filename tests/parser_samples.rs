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
