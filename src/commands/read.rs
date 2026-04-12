use anyhow::Result;
use serde::Serialize;

use crate::index::{EventRecord, MessageRecord, Store, ThreadRead};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct ThreadReadResponse {
    command: &'static str,
    ok: bool,
    thread: crate::index::ThreadRecord,
    messages: Vec<MessageRecord>,
}

#[derive(Debug, Serialize)]
struct MessageReadResponse {
    command: &'static str,
    ok: bool,
    session_id: String,
    count: usize,
    messages: Vec<MessageRecord>,
}

#[derive(Debug, Serialize)]
struct EventReadResponse {
    command: &'static str,
    ok: bool,
    session_id: String,
    count: usize,
    events: Vec<EventRecord>,
}

pub fn thread(store: &Store, identifier: &str, limit: Option<usize>) -> Result<Rendered> {
    let ThreadRead { thread, messages } = store.read_thread(identifier, limit)?;
    let response = ThreadReadResponse {
        command: "threads.read",
        ok: true,
        thread: thread.clone(),
        messages: messages.clone(),
    };

    let mut lines = vec![
        format!("线程: {}", thread.session_id),
        format!("标题: {}", thread.title),
        format!("消息数: {}", thread.message_count),
        format!("事件数: {}", thread.event_count),
    ];
    for message in messages {
        lines.push(format!(
            "- {} {} {}",
            message.timestamp.unwrap_or_default(),
            message.role,
            message.text
        ));
    }

    Rendered::new(lines.join("\n"), &response)
}

pub fn messages(store: &Store, identifier: &str, limit: Option<usize>) -> Result<Rendered> {
    let messages = store.read_messages(identifier, limit)?;
    let session_id = messages
        .first()
        .map(|value| value.session_id.clone())
        .unwrap_or_else(|| identifier.to_string());
    let response = MessageReadResponse {
        command: "messages.read",
        ok: true,
        session_id: session_id.clone(),
        count: messages.len(),
        messages: messages.clone(),
    };

    let mut lines = vec![
        format!("消息线程: {}", session_id),
        format!("返回条数: {}", response.count),
    ];
    for message in messages {
        lines.push(format!(
            "- {} {} {}",
            message.timestamp.unwrap_or_default(),
            message.role,
            message.text
        ));
    }

    Rendered::new(lines.join("\n"), &response)
}

pub fn events(store: &Store, identifier: &str, limit: Option<usize>) -> Result<Rendered> {
    let events = store.read_events(identifier, limit)?;
    let session_id = events
        .first()
        .map(|value| value.session_id.clone())
        .unwrap_or_else(|| identifier.to_string());
    let response = EventReadResponse {
        command: "events.read",
        ok: true,
        session_id: session_id.clone(),
        count: events.len(),
        events: events.clone(),
    };

    let mut lines = vec![
        format!("事件线程: {}", session_id),
        format!("返回条数: {}", response.count),
    ];
    for event in events {
        lines.push(format!(
            "- {} {} {}",
            event.timestamp.unwrap_or_default(),
            event.event_type,
            event.summary
        ));
    }

    Rendered::new(lines.join("\n"), &response)
}
