use std::fs;
use std::path::{Path, PathBuf};

pub fn write_fixture_sessions(root: &Path) -> (PathBuf, PathBuf) {
    let session_dir = root.join("sessions").join("2026").join("04").join("12");
    fs::create_dir_all(&session_dir).unwrap();

    let alpha = session_dir.join("rollout-2026-04-12T10-00-00-session-alpha.jsonl");
    let beta = session_dir.join("rollout-2026-04-12T11-00-00-session-beta.jsonl");

    fs::write(&alpha, alpha_session()).unwrap();
    fs::write(&beta, beta_session()).unwrap();

    (alpha, beta)
}

#[allow(dead_code)]
pub fn write_invalid_session(root: &Path) -> PathBuf {
    let session_dir = root.join("sessions").join("2026").join("04").join("12");
    fs::create_dir_all(&session_dir).unwrap();

    let invalid = session_dir.join("rollout-2026-04-12T12-00-00-session-invalid.jsonl");
    fs::write(
        &invalid,
        "{\"timestamp\":\"2026-04-12T12:00:00Z\",\"type\":\"session_meta\"\nnot-json\n",
    )
    .unwrap();

    invalid
}

#[allow(dead_code)]
pub fn append_alpha_message(alpha_path: &Path) {
    let extra = r#"{"timestamp":"2026-04-12T10:00:08Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The CLI now supports incremental sync."}]}}"#;
    let mut content = fs::read_to_string(alpha_path).unwrap();
    content.push('\n');
    content.push_str(extra);
    content.push('\n');
    fs::write(alpha_path, content).unwrap();
}

#[allow(dead_code)]
pub fn overwrite_with_invalid_json(path: &Path) {
    fs::write(
        path,
        "{\"timestamp\":\"2026-04-12T10:00:00Z\",\"type\":\"session_meta\"\nnot-json\n",
    )
    .unwrap();
}

fn alpha_session() -> String {
    [
        r#"{"timestamp":"2026-04-12T10:00:00Z","type":"session_meta","payload":{"id":"session-alpha","timestamp":"2026-04-12T10:00:00Z","cwd":"/workspace/alpha-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Please build a CLI for thread search"}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I will build a CLI using Rust and SQLite."}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The C++ parser still needs better symbol-aware search."}]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"rg\",\"CLI\"]}"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"Please build a CLI for thread search","images":[]}}"#,
        r#"{"timestamp":"2026-04-12T10:00:05Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"Planning CLI surface and indexing layout"}}"#,
        r#"{"timestamp":"2026-04-12T10:00:06Z","type":"turn_context","payload":{"cwd":"/workspace/alpha-repo","approval_policy":"on-request","model":"gpt-5-codex"}}"#,
    ]
    .join("\n")
}

fn beta_session() -> String {
    [
        r#"{"timestamp":"2026-04-12T11:00:00Z","type":"session_meta","payload":{"id":"session-beta","timestamp":"2026-04-12T11:00:00Z","cwd":"/workspace/beta-repo","originator":"codex_cli_rs","cli_version":"0.53.0"}}"#,
        r#"{"timestamp":"2026-04-12T11:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Investigate websocket reconnect masking"}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The reconnect overlay should only appear after policy rejection."}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The plain C fallback is still too broad."}]}}"#,
        r#"{"timestamp":"2026-04-12T11:00:03Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"Checking websocket status handling"}}"#,
    ]
    .join("\n")
}
