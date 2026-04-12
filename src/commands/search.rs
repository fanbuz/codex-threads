use anyhow::Result;
use serde::Serialize;

use crate::cli::{EventSearchArgs, MessageSearchArgs, ThreadSearchArgs};
use crate::index::{
    EventSearchFilters, EventSearchHit, MessageSearchFilters, MessageSearchHit, Store,
    ThreadSearchFilters, ThreadSearchHit,
};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct MessageSearchResponse {
    command: &'static str,
    ok: bool,
    query: String,
    count: usize,
    filters: MessageSearchFilters,
    results: Vec<MessageSearchHit>,
}

#[derive(Debug, Serialize)]
struct ThreadSearchResponse {
    command: &'static str,
    ok: bool,
    query: String,
    count: usize,
    filters: ThreadSearchFilters,
    results: Vec<ThreadSearchHit>,
}

#[derive(Debug, Serialize)]
struct EventSearchResponse {
    command: &'static str,
    ok: bool,
    query: String,
    count: usize,
    filters: EventSearchFilters,
    results: Vec<EventSearchHit>,
}

pub fn messages(store: &Store, args: &MessageSearchArgs) -> Result<Rendered> {
    let filters = MessageSearchFilters {
        since: args.common.since.clone(),
        until: args.common.until.clone(),
        session: args.common.session.clone(),
        role: args.role.clone(),
    };
    let results = store.search_messages(&args.common.query, args.common.limit, &filters)?;
    let response = MessageSearchResponse {
        command: "messages.search",
        ok: true,
        query: args.common.query.clone(),
        count: results.len(),
        filters: filters.clone(),
        results: results.clone(),
    };

    let mut lines = vec![
        format!("消息搜索: {}", args.common.query),
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

pub fn threads(store: &Store, args: &ThreadSearchArgs) -> Result<Rendered> {
    let filters = ThreadSearchFilters {
        since: args.common.since.clone(),
        until: args.common.until.clone(),
        session: args.common.session.clone(),
        cwd: args.cwd.clone(),
        path: args.path.clone(),
    };
    let results = store.search_threads(&args.common.query, args.common.limit, &filters)?;
    let response = ThreadSearchResponse {
        command: "threads.search",
        ok: true,
        query: args.common.query.clone(),
        count: results.len(),
        filters: filters.clone(),
        results: results.clone(),
    };

    let mut lines = vec![
        format!("线程搜索: {}", args.common.query),
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

pub fn events(store: &Store, args: &EventSearchArgs) -> Result<Rendered> {
    let filters = EventSearchFilters {
        since: args.common.since.clone(),
        until: args.common.until.clone(),
        session: args.common.session.clone(),
        event_type: args.event_type.clone(),
    };
    let results = store.search_events(&args.common.query, args.common.limit, &filters)?;
    let response = EventSearchResponse {
        command: "events.search",
        ok: true,
        query: args.common.query.clone(),
        count: results.len(),
        filters: filters.clone(),
        results: results.clone(),
    };

    let mut lines = vec![
        format!("事件搜索: {}", args.common.query),
        format!("命中条数: {}", response.count),
    ];
    for item in results {
        lines.push(format!(
            "- [{}] {} {}",
            item.session_id, item.event_type, item.snippet
        ));
    }

    Rendered::new(lines.join("\n"), &response).map(|rendered| rendered.with_duration_after_line(1))
}
