use anyhow::Result;
use serde::Serialize;

use crate::index::{MessageSearchHit, Store, ThreadSearchHit};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct MessageSearchResponse {
    command: &'static str,
    ok: bool,
    query: String,
    count: usize,
    results: Vec<MessageSearchHit>,
}

#[derive(Debug, Serialize)]
struct ThreadSearchResponse {
    command: &'static str,
    ok: bool,
    query: String,
    count: usize,
    results: Vec<ThreadSearchHit>,
}

pub fn messages(store: &Store, query: &str, limit: usize) -> Result<Rendered> {
    let results = store.search_messages(query, limit)?;
    let response = MessageSearchResponse {
        command: "messages.search",
        ok: true,
        query: query.to_string(),
        count: results.len(),
        results: results.clone(),
    };

    let mut lines = vec![
        format!("消息搜索: {}", query),
        format!("命中条数: {}", response.count),
    ];
    for item in results {
        lines.push(format!(
            "- [{}] {} {}",
            item.session_id, item.role, item.snippet
        ));
    }

    Rendered::new(lines.join("\n"), &response).map(|rendered| rendered.with_duration_after_line(1))
}

pub fn threads(store: &Store, query: &str, limit: usize) -> Result<Rendered> {
    let results = store.search_threads(query, limit)?;
    let response = ThreadSearchResponse {
        command: "threads.search",
        ok: true,
        query: query.to_string(),
        count: results.len(),
        results: results.clone(),
    };

    let mut lines = vec![
        format!("线程搜索: {}", query),
        format!("命中条数: {}", response.count),
    ];
    for item in results {
        lines.push(format!(
            "- [{}] {} {}",
            item.session_id, item.title, item.snippet
        ));
    }

    Rendered::new(lines.join("\n"), &response).map(|rendered| rendered.with_duration_after_line(1))
}
