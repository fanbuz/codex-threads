use std::path::Path;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use anyhow::{Context, Result};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSession {
    pub session_id: String,
    pub cwd: Option<String>,
    pub title: String,
    pub aggregate_text: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub messages: Vec<ParsedMessage>,
    pub events: Vec<ParsedEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub timestamp: String,
    pub role: String,
    pub text: String,
    pub raw_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub timestamp: String,
    pub event_type: String,
    pub summary: String,
    pub raw_json: String,
}

pub fn parse_session_file(path: &Path) -> Result<ParsedSession> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut cwd = None;
    let mut first_user_message = None;
    let mut started_at = None;
    let mut ended_at = None;
    let mut messages = Vec::new();
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !timestamp.is_empty() {
            if started_at.is_none() {
                started_at = Some(timestamp.clone());
            }
            ended_at = Some(timestamp.clone());
        }
        let record_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let payload = value.get("payload").cloned().unwrap_or(Value::Null);

        match record_type {
            "session_meta" => {
                session_id = payload
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                cwd = payload
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            "response_item" => {
                let payload_type = payload
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if payload_type == "message" {
                    let role = payload
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let text = extract_message_text(&payload);
                    if !text.is_empty() {
                        if role == "user" && first_user_message.is_none() {
                            first_user_message = Some(text.clone());
                        }
                        messages.push(ParsedMessage {
                            timestamp,
                            role,
                            text,
                            raw_json: String::new(),
                        });
                    }
                } else {
                    let event_type = if payload_type.is_empty() {
                        "response_item".to_string()
                    } else {
                        payload_type.to_string()
                    };
                    let summary = summarize_response_item(&payload);
                    events.push(ParsedEvent {
                        timestamp,
                        event_type,
                        summary: trim_for_storage(&summary, 1000),
                        raw_json: String::new(),
                    });
                }
            }
            "event_msg" => {
                let event_type = payload
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("event_msg")
                    .to_string();
                let summary = summarize_event_payload(&payload);
                events.push(ParsedEvent {
                    timestamp,
                    event_type,
                    summary: trim_for_storage(&summary, 1000),
                    raw_json: String::new(),
                });
            }
            "turn_context" => {
                let summary = summarize_turn_context(&payload);
                events.push(ParsedEvent {
                    timestamp,
                    event_type: "turn_context".to_string(),
                    summary: trim_for_storage(&summary, 1000),
                    raw_json: String::new(),
                });
            }
            other => {
                events.push(ParsedEvent {
                    timestamp,
                    event_type: other.to_string(),
                    summary: trim_for_storage(&compact_json(&payload), 1000),
                    raw_json: String::new(),
                });
            }
        }
    }

    let session_id = session_id.unwrap_or_else(|| fallback_session_id(path));
    let title = build_title(&session_id, cwd.as_deref(), first_user_message.as_deref());
    let aggregate_text = build_aggregate_text(&title, cwd.as_deref(), &messages, &events);

    Ok(ParsedSession {
        session_id,
        cwd,
        title,
        aggregate_text,
        started_at,
        ended_at,
        messages,
        events,
    })
}

fn extract_message_text(payload: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(content) = payload.get("content").and_then(Value::as_array) {
        for item in content {
            collect_text_fragments(item, &mut parts);
        }
    }

    if parts.is_empty() {
        collect_text_fragments(payload, &mut parts);
    }

    normalize_text(parts.join("\n"))
}

fn collect_text_fragments(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let text = text.trim();
            if !text.is_empty() {
                out.push(text.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_fragments(item, out);
            }
        }
        Value::Object(map) => {
            for key in ["text", "message", "arguments", "output"] {
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() {
                        out.push(text.to_string());
                    }
                }
            }

            if let Some(summary) = map.get("summary") {
                collect_text_fragments(summary, out);
            }
        }
        _ => {}
    }
}

fn summarize_response_item(payload: &Value) -> String {
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("response_item");
    match payload_type {
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool");
            let arguments = payload
                .get("arguments")
                .and_then(Value::as_str)
                .map(|text| normalize_text(text.to_string()))
                .unwrap_or_default();
            if arguments.is_empty() {
                format!("function_call {}", name)
            } else {
                format!("function_call {} {}", name, arguments)
            }
        }
        "function_call_output" => {
            let output = payload
                .get("output")
                .and_then(Value::as_str)
                .map(|text| normalize_text(text.to_string()))
                .unwrap_or_else(|| compact_json(payload));
            format!("function_call_output {}", output)
        }
        "reasoning" => {
            let mut parts = Vec::new();
            if let Some(summary) = payload.get("summary") {
                collect_text_fragments(summary, &mut parts);
            }
            if parts.is_empty() {
                compact_json(payload)
            } else {
                normalize_text(parts.join(" "))
            }
        }
        _ => {
            let text = extract_message_text(payload);
            if text.is_empty() {
                compact_json(payload)
            } else {
                text
            }
        }
    }
}

fn summarize_event_payload(payload: &Value) -> String {
    for key in ["message", "text"] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            return normalize_text(text.to_string());
        }
    }
    compact_json(payload)
}

fn summarize_turn_context(payload: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
        parts.push(format!("cwd={}", cwd));
    }
    if let Some(model) = payload.get("model").and_then(Value::as_str) {
        parts.push(format!("model={}", model));
    }
    if let Some(approval_policy) = payload.get("approval_policy").and_then(Value::as_str) {
        parts.push(format!("approval={}", approval_policy));
    }

    if parts.is_empty() {
        compact_json(payload)
    } else {
        parts.join(" ")
    }
}

fn build_title(session_id: &str, cwd: Option<&str>, first_user_message: Option<&str>) -> String {
    match (
        cwd.and_then(last_path_segment),
        first_user_message.map(shorten),
    ) {
        (Some(folder), Some(summary)) => format!("{}: {}", folder, summary),
        (Some(folder), None) => folder,
        (None, Some(summary)) => summary,
        (None, None) => session_id.to_string(),
    }
}

fn build_aggregate_text(
    title: &str,
    cwd: Option<&str>,
    messages: &[ParsedMessage],
    events: &[ParsedEvent],
) -> String {
    let mut parts = vec![title.to_string()];
    if let Some(cwd) = cwd {
        parts.push(cwd.to_string());
    }
    for message in messages {
        parts.push(trim_for_storage(&message.text, 1200));
    }
    for event in events {
        if !event.summary.is_empty() {
            parts.push(trim_for_storage(&event.summary, 600));
        }
    }
    normalize_text(parts.join("\n"))
}

fn fallback_session_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "unknown-session".to_string())
}

fn last_path_segment(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
}

fn shorten(text: &str) -> String {
    const MAX_LEN: usize = 72;
    let text = normalize_text(text.to_string());
    let mut chars = text.chars();
    let shortened: String = chars.by_ref().take(MAX_LEN).collect();
    if chars.next().is_some() {
        format!("{}...", shortened)
    } else {
        shortened
    }
}

fn normalize_text(text: String) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_json(value: &Value) -> String {
    if value.is_null() {
        String::new()
    } else {
        serde_json::to_string(value).unwrap_or_else(|_| json!({"invalid": true}).to_string())
    }
}

fn trim_for_storage(text: &str, max_chars: usize) -> String {
    let normalized = normalize_text(text.to_string());
    let mut chars = normalized.chars();
    let shortened: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", shortened)
    } else {
        shortened
    }
}
