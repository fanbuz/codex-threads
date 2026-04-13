use anyhow::{anyhow, bail, Result};
use rusqlite::{params, OptionalExtension};

use super::super::types::{EventRecord, MessageRecord, ThreadRead, ThreadRecord};
use super::Store;

impl Store {
    pub fn read_thread(&self, identifier: &str, limit: Option<usize>) -> Result<ThreadRead> {
        let session_id = self.resolve_session_id(identifier)?;
        let thread = self
            .load_thread(&session_id)?
            .ok_or_else(|| anyhow!("未找到线程: {}", identifier))?;
        let messages = self.read_messages_by_session(&session_id, limit)?;
        Ok(ThreadRead { thread, messages })
    }

    pub fn read_messages(
        &self,
        identifier: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MessageRecord>> {
        let session_id = self.resolve_session_id(identifier)?;
        self.read_messages_by_session(&session_id, limit)
    }

    pub fn read_events(&self, identifier: &str, limit: Option<usize>) -> Result<Vec<EventRecord>> {
        let session_id = self.resolve_session_id(identifier)?;
        self.read_events_by_session(&session_id, limit)
    }

    fn resolve_session_id(&self, identifier: &str) -> Result<String> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id
            FROM threads
            WHERE session_id = ?1
                OR file_name = ?1
                OR replace(file_name, '.jsonl', '') = ?1
                OR file_name LIKE '%' || ?1 || '%'
            ORDER BY session_id
            LIMIT 5
            "#,
        )?;

        let mut rows = stmt.query(params![identifier])?;
        let mut matches = Vec::new();
        while let Some(row) = rows.next()? {
            matches.push(row.get::<_, String>(0)?);
        }

        matches.sort();
        matches.dedup();

        match matches.len() {
            0 => bail!("未找到线程: {}", identifier),
            1 => Ok(matches.remove(0)),
            _ => bail!("线程标识不唯一，请使用更精确的 session_id: {}", identifier),
        }
    }

    fn load_thread(&self, session_id: &str) -> Result<Option<ThreadRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT session_id, title, cwd, path, file_name, folder, started_at, ended_at,
                       message_count, event_count
                FROM threads
                WHERE session_id = ?1
                "#,
                params![session_id],
                |row| {
                    Ok(ThreadRecord {
                        session_id: row.get(0)?,
                        title: row.get(1)?,
                        cwd: row.get(2)?,
                        path: row.get(3)?,
                        file_name: row.get(4)?,
                        folder: row.get(5)?,
                        started_at: row.get(6)?,
                        ended_at: row.get(7)?,
                        message_count: row.get::<_, i64>(8)? as usize,
                        event_count: row.get::<_, i64>(9)? as usize,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn read_messages_by_session(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MessageRecord>> {
        if let Some(limit) = limit {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, role, text
                FROM (
                    SELECT session_id, timestamp, role, text, idx
                    FROM messages
                    WHERE session_id = ?1
                    ORDER BY idx DESC
                    LIMIT ?2
                )
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |row| {
                Ok(MessageRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    role: row.get(2)?,
                    text: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, role, text
                FROM messages
                WHERE session_id = ?1
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(MessageRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    role: row.get(2)?,
                    text: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        }
    }

    fn read_events_by_session(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<EventRecord>> {
        if let Some(limit) = limit {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, event_type, summary
                FROM (
                    SELECT session_id, timestamp, event_type, summary, idx
                    FROM events
                    WHERE session_id = ?1
                    ORDER BY idx DESC
                    LIMIT ?2
                )
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |row| {
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    summary: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT session_id, timestamp, event_type, summary
                FROM events
                WHERE session_id = ?1
                ORDER BY idx ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    summary: row.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        }
    }
}
