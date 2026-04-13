use anyhow::Result;
use rusqlite::types::Value;

use crate::output::excerpt;
use crate::query::normalize_query_terms;

use super::super::search_meta::{
    analyze_match, build_search_meta, SearchBackend, SearchExplain, SearchField, SearchQueryMode,
    SearchReport,
};
use super::super::types::{
    EventSearchFilters, EventSearchHit, MessageSearchFilters, MessageSearchHit,
    ThreadSearchFilters, ThreadSearchHit,
};
use super::Store;

impl Store {
    pub fn search_threads(
        &self,
        query: &str,
        limit: usize,
        filters: &ThreadSearchFilters,
    ) -> Result<SearchReport<ThreadSearchHit>> {
        let original_query = query.trim();
        if original_query.is_empty() {
            return Ok(SearchReport {
                search: build_search_meta(
                    original_query,
                    &[],
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                results: Vec::new(),
            });
        }
        let query_terms = normalize_query_terms(original_query);
        let normalized_query = if query_terms.is_empty() {
            original_query.to_string()
        } else {
            query_terms.join(" ")
        };
        let fallback_limit = fallback_candidate_limit(limit);

        if self.fts_available {
            if let Ok(results) = self.search_threads_fts(original_query, limit, filters) {
                if !results.is_empty() {
                    return Ok(finalize_thread_search_report(
                        results,
                        build_search_meta(
                            original_query,
                            &query_terms,
                            SearchBackend::Fts,
                            SearchQueryMode::Literal,
                            "bm25",
                        ),
                        original_query,
                        &query_terms,
                        limit,
                    ));
                }
            }
        }

        let literal_results =
            self.search_threads_like_literal(original_query, fallback_limit, filters)?;
        if !literal_results.is_empty() {
            return Ok(finalize_thread_search_report(
                literal_results,
                build_search_meta(
                    original_query,
                    &query_terms,
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                original_query,
                &query_terms,
                limit,
            ));
        }

        if self.fts_available && should_expand_query_terms(original_query, &query_terms) {
            if let Some(fts_query) = expanded_fts_query(original_query, &normalized_query) {
                if let Ok(results) = self.search_threads_fts(&fts_query, limit, filters) {
                    if !results.is_empty() {
                        return Ok(finalize_thread_search_report(
                            results,
                            build_search_meta(
                                original_query,
                                &query_terms,
                                SearchBackend::Fts,
                                SearchQueryMode::Expanded,
                                "bm25",
                            ),
                            original_query,
                            &query_terms,
                            limit,
                        ));
                    }
                }
            }
        }

        self.search_threads_like(&query_terms, &normalized_query, fallback_limit, filters)
            .map(|results| {
                finalize_thread_search_report(
                    results,
                    build_search_meta(
                        original_query,
                        &query_terms,
                        SearchBackend::Like,
                        SearchQueryMode::Expanded,
                        "field_match_then_recency",
                    ),
                    original_query,
                    &query_terms,
                    limit,
                )
            })
    }

    pub fn search_messages(
        &self,
        query: &str,
        limit: usize,
        filters: &MessageSearchFilters,
    ) -> Result<SearchReport<MessageSearchHit>> {
        let original_query = query.trim();
        if original_query.is_empty() {
            return Ok(SearchReport {
                search: build_search_meta(
                    original_query,
                    &[],
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                results: Vec::new(),
            });
        }
        let query_terms = normalize_query_terms(original_query);
        let normalized_query = if query_terms.is_empty() {
            original_query.to_string()
        } else {
            query_terms.join(" ")
        };
        let fallback_limit = fallback_candidate_limit(limit);

        if self.fts_available {
            if let Ok(results) = self.search_messages_fts(original_query, limit, filters) {
                if !results.is_empty() {
                    return Ok(finalize_message_search_report(
                        results,
                        build_search_meta(
                            original_query,
                            &query_terms,
                            SearchBackend::Fts,
                            SearchQueryMode::Literal,
                            "bm25",
                        ),
                        original_query,
                        &query_terms,
                        limit,
                    ));
                }
            }
        }

        let literal_results =
            self.search_messages_like_literal(original_query, fallback_limit, filters)?;
        if !literal_results.is_empty() {
            return Ok(finalize_message_search_report(
                literal_results,
                build_search_meta(
                    original_query,
                    &query_terms,
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                original_query,
                &query_terms,
                limit,
            ));
        }

        if self.fts_available && should_expand_query_terms(original_query, &query_terms) {
            if let Some(fts_query) = expanded_fts_query(original_query, &normalized_query) {
                if let Ok(results) = self.search_messages_fts(&fts_query, limit, filters) {
                    if !results.is_empty() {
                        return Ok(finalize_message_search_report(
                            results,
                            build_search_meta(
                                original_query,
                                &query_terms,
                                SearchBackend::Fts,
                                SearchQueryMode::Expanded,
                                "bm25",
                            ),
                            original_query,
                            &query_terms,
                            limit,
                        ));
                    }
                }
            }
        }

        self.search_messages_like(&query_terms, &normalized_query, fallback_limit, filters)
            .map(|results| {
                finalize_message_search_report(
                    results,
                    build_search_meta(
                        original_query,
                        &query_terms,
                        SearchBackend::Like,
                        SearchQueryMode::Expanded,
                        "field_match_then_recency",
                    ),
                    original_query,
                    &query_terms,
                    limit,
                )
            })
    }

    pub fn search_events(
        &self,
        query: &str,
        limit: usize,
        filters: &EventSearchFilters,
    ) -> Result<SearchReport<EventSearchHit>> {
        let original_query = query.trim();
        if original_query.is_empty() {
            return Ok(SearchReport {
                search: build_search_meta(
                    original_query,
                    &[],
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                results: Vec::new(),
            });
        }
        let query_terms = normalize_query_terms(original_query);
        let normalized_query = if query_terms.is_empty() {
            original_query.to_string()
        } else {
            query_terms.join(" ")
        };
        let fallback_limit = fallback_candidate_limit(limit);

        if self.fts_available {
            if let Ok(results) = self.search_events_fts(original_query, limit, filters) {
                if !results.is_empty() {
                    return Ok(finalize_event_search_report(
                        results,
                        build_search_meta(
                            original_query,
                            &query_terms,
                            SearchBackend::Fts,
                            SearchQueryMode::Literal,
                            "bm25",
                        ),
                        original_query,
                        &query_terms,
                        limit,
                    ));
                }
            }
        }

        let literal_results =
            self.search_events_like_literal(original_query, fallback_limit, filters)?;
        if !literal_results.is_empty() {
            return Ok(finalize_event_search_report(
                literal_results,
                build_search_meta(
                    original_query,
                    &query_terms,
                    SearchBackend::Like,
                    SearchQueryMode::Literal,
                    "field_match_then_recency",
                ),
                original_query,
                &query_terms,
                limit,
            ));
        }

        if self.fts_available && should_expand_query_terms(original_query, &query_terms) {
            if let Some(fts_query) = expanded_fts_query(original_query, &normalized_query) {
                if let Ok(results) = self.search_events_fts(&fts_query, limit, filters) {
                    if !results.is_empty() {
                        return Ok(finalize_event_search_report(
                            results,
                            build_search_meta(
                                original_query,
                                &query_terms,
                                SearchBackend::Fts,
                                SearchQueryMode::Expanded,
                                "bm25",
                            ),
                            original_query,
                            &query_terms,
                            limit,
                        ));
                    }
                }
            }
        }

        self.search_events_like(&query_terms, &normalized_query, fallback_limit, filters)
            .map(|results| {
                finalize_event_search_report(
                    results,
                    build_search_meta(
                        original_query,
                        &query_terms,
                        SearchBackend::Like,
                        SearchQueryMode::Expanded,
                        "field_match_then_recency",
                    ),
                    original_query,
                    &query_terms,
                    limit,
                )
            })
    }

    fn search_threads_fts(
        &self,
        query: &str,
        limit: usize,
        filters: &ThreadSearchFilters,
    ) -> Result<Vec<ThreadSearchHit>> {
        let mut conditions = vec!["threads_fts MATCH ?".to_string()];
        let mut params = vec![Value::Text(query.to_string())];
        push_thread_filter_conditions(&mut conditions, &mut params, filters, "t");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT
                t.session_id,
                t.title,
                t.cwd,
                t.path,
                t.message_count,
                t.event_count,
                t.aggregate_text,
                t.started_at,
                snippet(threads_fts, 4, '[', ']', '…', 12)
            FROM threads_fts
            JOIN threads t ON t.id = threads_fts.rowid
            WHERE {}
            ORDER BY bm25(threads_fts)
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let aggregate_text: String = row.get(6)?;
            let started_at: Option<String> = row.get(7)?;
            let snippet: Option<String> = row.get(8)?;
            Ok(ThreadSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                path: row.get(3)?,
                message_count: row.get::<_, i64>(4)? as usize,
                event_count: row.get::<_, i64>(5)? as usize,
                snippet: snippet.unwrap_or_else(|| excerpt(&aggregate_text, query, 120)),
                explain: SearchExplain::default(),
                aggregate_text,
                started_at,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_threads_like(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &ThreadSearchFilters,
    ) -> Result<Vec<ThreadSearchHit>> {
        if should_expand_query_terms(normalized_query, query_terms) {
            return self.search_threads_like_terms(query_terms, normalized_query, limit, filters);
        }

        Ok(Vec::new())
    }

    fn search_threads_like_literal(
        &self,
        original_query: &str,
        limit: usize,
        filters: &ThreadSearchFilters,
    ) -> Result<Vec<ThreadSearchHit>> {
        let mut conditions = vec![r#"
                (
                    lower(title) LIKE lower(?) ESCAPE '\'
                    OR lower(ifnull(cwd, '')) LIKE lower(?) ESCAPE '\'
                    OR lower(path) LIKE lower(?) ESCAPE '\'
                    OR lower(aggregate_text) LIKE lower(?) ESCAPE '\'
                )
            "#
        .trim()
        .to_string()];
        let pattern = like_pattern(original_query);
        let mut params = vec![
            Value::Text(pattern.clone()),
            Value::Text(pattern.clone()),
            Value::Text(pattern.clone()),
            Value::Text(pattern),
        ];
        push_thread_filter_conditions(&mut conditions, &mut params, filters, "");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT session_id, title, cwd, path, message_count, event_count, aggregate_text, started_at
            FROM threads
            WHERE {}
            ORDER BY started_at DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let aggregate_text: String = row.get(6)?;
            Ok(ThreadSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                path: row.get(3)?,
                message_count: row.get::<_, i64>(4)? as usize,
                event_count: row.get::<_, i64>(5)? as usize,
                snippet: excerpt(&aggregate_text, original_query, 120),
                explain: SearchExplain::default(),
                aggregate_text,
                started_at: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_threads_like_terms(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &ThreadSearchFilters,
    ) -> Result<Vec<ThreadSearchHit>> {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        for term in query_terms {
            conditions.push(
                r#"
                (
                    lower(title) LIKE lower(?) ESCAPE '\'
                    OR lower(ifnull(cwd, '')) LIKE lower(?) ESCAPE '\'
                    OR lower(path) LIKE lower(?) ESCAPE '\'
                    OR lower(aggregate_text) LIKE lower(?) ESCAPE '\'
                )
                "#
                .trim()
                .to_string(),
            );
            let pattern = like_pattern(term);
            params.push(Value::Text(pattern.clone()));
            params.push(Value::Text(pattern.clone()));
            params.push(Value::Text(pattern.clone()));
            params.push(Value::Text(pattern));
        }
        push_thread_filter_conditions(&mut conditions, &mut params, filters, "");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT session_id, title, cwd, path, message_count, event_count, aggregate_text, started_at
            FROM threads
            WHERE {}
            ORDER BY started_at DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let aggregate_text: String = row.get(6)?;
            Ok(ThreadSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                path: row.get(3)?,
                message_count: row.get::<_, i64>(4)? as usize,
                event_count: row.get::<_, i64>(5)? as usize,
                snippet: excerpt(&aggregate_text, normalized_query, 120),
                explain: SearchExplain::default(),
                aggregate_text,
                started_at: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_messages_fts(
        &self,
        query: &str,
        limit: usize,
        filters: &MessageSearchFilters,
    ) -> Result<Vec<MessageSearchHit>> {
        let mut conditions = vec!["messages_fts MATCH ?".to_string()];
        let mut params = vec![Value::Text(query.to_string())];
        push_message_filter_conditions(&mut conditions, &mut params, filters, "m");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT
                m.session_id,
                t.title,
                m.timestamp,
                m.role,
                m.text,
                snippet(messages_fts, 2, '[', ']', '…', 12)
            FROM messages_fts
            JOIN messages m ON m.id = messages_fts.rowid
            LEFT JOIN threads t ON t.session_id = m.session_id
            WHERE {}
            ORDER BY bm25(messages_fts)
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let text: String = row.get(4)?;
            let snippet: Option<String> = row.get(5)?;
            Ok(MessageSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                role: row.get(3)?,
                text: text.clone(),
                snippet: snippet.unwrap_or_else(|| excerpt(&text, query, 120)),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_events_fts(
        &self,
        query: &str,
        limit: usize,
        filters: &EventSearchFilters,
    ) -> Result<Vec<EventSearchHit>> {
        let mut conditions = vec!["events_fts MATCH ?".to_string()];
        let mut params = vec![Value::Text(query.to_string())];
        push_event_filter_conditions(&mut conditions, &mut params, filters, "e");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT
                e.session_id,
                t.title,
                e.timestamp,
                e.event_type,
                e.summary,
                snippet(events_fts, 2, '[', ']', '…', 12)
            FROM events_fts
            JOIN events e ON e.id = events_fts.rowid
            LEFT JOIN threads t ON t.session_id = e.session_id
            WHERE {}
            ORDER BY bm25(events_fts), e.timestamp DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let event_type: String = row.get(3)?;
            let summary: String = row.get(4)?;
            let snippet: Option<String> = row.get(5)?;
            let snippet_source = format!("{} {}", event_type, summary);
            Ok(EventSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                event_type,
                summary: summary.clone(),
                snippet: snippet.unwrap_or_else(|| excerpt(&snippet_source, query, 120)),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_messages_like(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &MessageSearchFilters,
    ) -> Result<Vec<MessageSearchHit>> {
        if should_expand_query_terms(normalized_query, query_terms) {
            return self.search_messages_like_terms(query_terms, normalized_query, limit, filters);
        }

        Ok(Vec::new())
    }

    fn search_events_like(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &EventSearchFilters,
    ) -> Result<Vec<EventSearchHit>> {
        if should_expand_query_terms(normalized_query, query_terms) {
            return self.search_events_like_terms(query_terms, normalized_query, limit, filters);
        }

        Ok(Vec::new())
    }

    fn search_messages_like_literal(
        &self,
        original_query: &str,
        limit: usize,
        filters: &MessageSearchFilters,
    ) -> Result<Vec<MessageSearchHit>> {
        let mut conditions = vec!["lower(m.text) LIKE lower(?) ESCAPE '\\'".to_string()];
        let mut params = vec![Value::Text(like_pattern(original_query))];
        push_message_filter_conditions(&mut conditions, &mut params, filters, "m");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT m.session_id, t.title, m.timestamp, m.role, m.text
            FROM messages m
            LEFT JOIN threads t ON t.session_id = m.session_id
            WHERE {}
            ORDER BY m.timestamp DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let text: String = row.get(4)?;
            Ok(MessageSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                role: row.get(3)?,
                text: text.clone(),
                snippet: excerpt(&text, original_query, 120),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_messages_like_terms(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &MessageSearchFilters,
    ) -> Result<Vec<MessageSearchHit>> {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        for term in query_terms {
            conditions.push("lower(m.text) LIKE lower(?) ESCAPE '\\'".to_string());
            params.push(Value::Text(like_pattern(term)));
        }
        push_message_filter_conditions(&mut conditions, &mut params, filters, "m");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT m.session_id, t.title, m.timestamp, m.role, m.text
            FROM messages m
            LEFT JOIN threads t ON t.session_id = m.session_id
            WHERE {}
            ORDER BY m.timestamp DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let text: String = row.get(4)?;
            Ok(MessageSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                role: row.get(3)?,
                text: text.clone(),
                snippet: excerpt(&text, normalized_query, 120),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_events_like_literal(
        &self,
        original_query: &str,
        limit: usize,
        filters: &EventSearchFilters,
    ) -> Result<Vec<EventSearchHit>> {
        let mut conditions = vec![
            "(lower(e.event_type) LIKE lower(?) ESCAPE '\\' OR lower(e.summary) LIKE lower(?) ESCAPE '\\')"
                .to_string(),
        ];
        let pattern = like_pattern(original_query);
        let mut params = vec![Value::Text(pattern.clone()), Value::Text(pattern)];
        push_event_filter_conditions(&mut conditions, &mut params, filters, "e");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT e.session_id, t.title, e.timestamp, e.event_type, e.summary
            FROM events e
            LEFT JOIN threads t ON t.session_id = e.session_id
            WHERE {}
            ORDER BY e.timestamp DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let event_type: String = row.get(3)?;
            let summary: String = row.get(4)?;
            let snippet_source = format!("{} {}", event_type, summary);
            Ok(EventSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                event_type,
                summary: summary.clone(),
                snippet: excerpt(&snippet_source, original_query, 120),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_events_like_terms(
        &self,
        query_terms: &[String],
        normalized_query: &str,
        limit: usize,
        filters: &EventSearchFilters,
    ) -> Result<Vec<EventSearchHit>> {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        for term in query_terms {
            conditions.push(
                "(lower(e.event_type) LIKE lower(?) ESCAPE '\\' OR lower(e.summary) LIKE lower(?) ESCAPE '\\')"
                    .to_string(),
            );
            let pattern = like_pattern(term);
            params.push(Value::Text(pattern.clone()));
            params.push(Value::Text(pattern));
        }
        push_event_filter_conditions(&mut conditions, &mut params, filters, "e");
        params.push(Value::Integer(limit as i64));

        let sql = format!(
            r#"
            SELECT e.session_id, t.title, e.timestamp, e.event_type, e.summary
            FROM events e
            LEFT JOIN threads t ON t.session_id = e.session_id
            WHERE {}
            ORDER BY e.timestamp DESC
            LIMIT ?
            "#,
            conditions.join("\n                AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let event_type: String = row.get(3)?;
            let summary: String = row.get(4)?;
            let snippet_source = format!("{} {}", event_type, summary);
            Ok(EventSearchHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                timestamp: row.get(2)?,
                event_type,
                summary: summary.clone(),
                snippet: excerpt(&snippet_source, normalized_query, 120),
                explain: SearchExplain::default(),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

fn expanded_fts_query(original_query: &str, normalized_query: &str) -> Option<String> {
    if !normalized_query.is_empty() && normalized_query != original_query {
        Some(normalized_query.to_string())
    } else {
        None
    }
}

fn should_expand_query_terms(normalized_query: &str, query_terms: &[String]) -> bool {
    !normalized_query.is_empty() && !query_terms.is_empty()
}

fn like_pattern(value: &str) -> String {
    format!("%{}%", escape_like(value))
}

fn escape_like(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '%' | '_' | '\\' => ['\\', ch].into_iter().collect::<Vec<_>>(),
            _ => [ch].into_iter().collect::<Vec<_>>(),
        })
        .collect()
}

fn finalize_thread_search_report(
    mut results: Vec<ThreadSearchHit>,
    search: super::super::search_meta::SearchMeta,
    query: &str,
    query_terms: &[String],
    limit: usize,
) -> SearchReport<ThreadSearchHit> {
    if search.backend == SearchBackend::Like {
        results.sort_by(|left, right| {
            let left_analysis = analyze_match(query, query_terms, &thread_search_fields(left), 0);
            let right_analysis = analyze_match(query, query_terms, &thread_search_fields(right), 0);

            right_analysis
                .fallback_score
                .cmp(&left_analysis.fallback_score)
                .then_with(|| right.started_at.cmp(&left.started_at))
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
    }

    if results.len() > limit {
        results.truncate(limit);
    }

    for (index, result) in results.iter_mut().enumerate() {
        result.explain =
            analyze_match(query, query_terms, &thread_search_fields(result), index + 1).explain;
    }

    SearchReport { search, results }
}

fn finalize_message_search_report(
    mut results: Vec<MessageSearchHit>,
    search: super::super::search_meta::SearchMeta,
    query: &str,
    query_terms: &[String],
    limit: usize,
) -> SearchReport<MessageSearchHit> {
    if search.backend == SearchBackend::Like {
        results.sort_by(|left, right| {
            let left_analysis = analyze_match(query, query_terms, &message_search_fields(left), 0);
            let right_analysis =
                analyze_match(query, query_terms, &message_search_fields(right), 0);

            right_analysis
                .fallback_score
                .cmp(&left_analysis.fallback_score)
                .then_with(|| right.timestamp.cmp(&left.timestamp))
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
    }

    if results.len() > limit {
        results.truncate(limit);
    }

    for (index, result) in results.iter_mut().enumerate() {
        result.explain = analyze_match(
            query,
            query_terms,
            &message_search_fields(result),
            index + 1,
        )
        .explain;
    }

    SearchReport { search, results }
}

fn finalize_event_search_report(
    mut results: Vec<EventSearchHit>,
    search: super::super::search_meta::SearchMeta,
    query: &str,
    query_terms: &[String],
    limit: usize,
) -> SearchReport<EventSearchHit> {
    if search.backend == SearchBackend::Like {
        results.sort_by(|left, right| {
            let left_analysis = analyze_match(query, query_terms, &event_search_fields(left), 0);
            let right_analysis = analyze_match(query, query_terms, &event_search_fields(right), 0);

            right_analysis
                .fallback_score
                .cmp(&left_analysis.fallback_score)
                .then_with(|| right.timestamp.cmp(&left.timestamp))
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
    }

    if results.len() > limit {
        results.truncate(limit);
    }

    for (index, result) in results.iter_mut().enumerate() {
        result.explain =
            analyze_match(query, query_terms, &event_search_fields(result), index + 1).explain;
    }

    SearchReport { search, results }
}

fn thread_search_fields(hit: &ThreadSearchHit) -> [SearchField<'_>; 4] {
    [
        SearchField {
            name: "title",
            text: Some(hit.title.as_str()),
            weight: 60,
        },
        SearchField {
            name: "cwd",
            text: hit.cwd.as_deref(),
            weight: 20,
        },
        SearchField {
            name: "path",
            text: Some(hit.path.as_str()),
            weight: 25,
        },
        SearchField {
            name: "aggregate_text",
            text: Some(hit.aggregate_text.as_str()),
            weight: 40,
        },
    ]
}

fn message_search_fields(hit: &MessageSearchHit) -> [SearchField<'_>; 1] {
    [SearchField {
        name: "text",
        text: Some(hit.text.as_str()),
        weight: 40,
    }]
}

fn event_search_fields(hit: &EventSearchHit) -> [SearchField<'_>; 2] {
    [
        SearchField {
            name: "event_type",
            text: Some(hit.event_type.as_str()),
            weight: 50,
        },
        SearchField {
            name: "summary",
            text: Some(hit.summary.as_str()),
            weight: 30,
        },
    ]
}

fn push_thread_filter_conditions(
    conditions: &mut Vec<String>,
    params: &mut Vec<Value>,
    filters: &ThreadSearchFilters,
    alias: &str,
) {
    push_common_search_filter_conditions(
        conditions,
        params,
        filters.session.as_deref(),
        filters.since.as_deref(),
        filters.until.as_deref(),
        &qualified_column(alias, "session_id"),
        &qualified_column(alias, "started_at"),
    );

    if let Some(cwd) = filters.cwd.as_deref() {
        conditions.push(format!(
            "lower(ifnull({}, '')) LIKE lower(?) ESCAPE '\\'",
            qualified_column(alias, "cwd")
        ));
        params.push(Value::Text(like_pattern(cwd)));
    }

    if let Some(path) = filters.path.as_deref() {
        conditions.push(format!(
            "lower({}) LIKE lower(?) ESCAPE '\\'",
            qualified_column(alias, "path")
        ));
        params.push(Value::Text(like_pattern(path)));
    }
}

fn push_message_filter_conditions(
    conditions: &mut Vec<String>,
    params: &mut Vec<Value>,
    filters: &MessageSearchFilters,
    alias: &str,
) {
    push_common_search_filter_conditions(
        conditions,
        params,
        filters.session.as_deref(),
        filters.since.as_deref(),
        filters.until.as_deref(),
        &qualified_column(alias, "session_id"),
        &qualified_column(alias, "timestamp"),
    );

    if let Some(role) = filters.role.as_deref() {
        conditions.push(format!(
            "lower({}) = lower(?)",
            qualified_column(alias, "role")
        ));
        params.push(Value::Text(role.to_string()));
    }
}

fn push_event_filter_conditions(
    conditions: &mut Vec<String>,
    params: &mut Vec<Value>,
    filters: &EventSearchFilters,
    alias: &str,
) {
    push_common_search_filter_conditions(
        conditions,
        params,
        filters.session.as_deref(),
        filters.since.as_deref(),
        filters.until.as_deref(),
        &qualified_column(alias, "session_id"),
        &qualified_column(alias, "timestamp"),
    );

    if let Some(event_type) = filters.event_type.as_deref() {
        conditions.push(format!(
            "lower({}) = lower(?)",
            qualified_column(alias, "event_type")
        ));
        params.push(Value::Text(event_type.to_string()));
    }
}

fn push_common_search_filter_conditions(
    conditions: &mut Vec<String>,
    params: &mut Vec<Value>,
    session: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    session_column: &str,
    timestamp_column: &str,
) {
    if let Some(session) = session {
        conditions.push(format!("{session_column} = ?"));
        params.push(Value::Text(session.to_string()));
    }

    if let Some(since) = since {
        conditions.push(format!("{timestamp_column} >= ?"));
        params.push(Value::Text(since.to_string()));
    }

    if let Some(until) = until {
        conditions.push(format!("{timestamp_column} <= ?"));
        params.push(Value::Text(until.to_string()));
    }
}

fn qualified_column(alias: &str, column: &str) -> String {
    if alias.is_empty() {
        column.to_string()
    } else {
        format!("{alias}.{column}")
    }
}

fn fallback_candidate_limit(limit: usize) -> usize {
    limit.saturating_mul(5).max(50).min(250)
}
