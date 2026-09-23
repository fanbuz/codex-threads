mod doctor;
mod lock;
mod read;
mod refresh;
mod resume;
mod search;
mod sync;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::schema::{init_schema, read_rebuild_required, write_rebuild_required};
use super::types::{SearchCoverage, StatusSummary};

#[derive(Debug)]
pub struct Store {
    conn: Connection,
    index_path: PathBuf,
    fts_available: bool,
    index_format_version: i64,
}

impl Store {
    pub fn open(index_dir: &Path) -> Result<Self> {
        fs::create_dir_all(index_dir)
            .with_context(|| format!("failed to create {}", index_dir.display()))?;
        let index_path = index_dir.join("threads.sqlite3");
        let conn = Connection::open(&index_path)
            .with_context(|| format!("failed to open {}", index_path.display()))?;
        let schema = init_schema(&conn)?;
        Ok(Self {
            conn,
            index_path,
            fts_available: schema.fts_available,
            index_format_version: schema.format_version,
        })
    }

    pub fn search_coverage(&self, session: Option<&str>) -> Result<SearchCoverage> {
        let indexed_threads = self.count_rows("threads")?;
        let requested_session_indexed = session
            .map(|id| {
                self.conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM threads WHERE session_id = ?1)",
                    [id],
                    |row| row.get::<_, bool>(0),
                )
            })
            .transpose()?;
        let (state, notice) = if self.rebuild_required()? {
            (
                "rebuild_required",
                "索引需要重建；请沿用相同目录参数运行 sync --force，不能据此判断历史是否存在。",
            )
        } else if indexed_threads == 0 {
            (
                "empty",
                "索引中没有会话；请确认目录并运行 sync，不能据此判断历史是否存在。",
            )
        } else if requested_session_indexed == Some(false) {
            (
                "session_not_indexed",
                "指定会话不在当前索引中；请确认会话 ID 和同步范围，不能据此判断历史是否存在。",
            )
        } else {
            ("indexed", "仅查询当前已索引内容；未核验全部会话文件及其最新变化，未命中不代表全部历史中不存在。")
        };
        Ok(SearchCoverage {
            state,
            index_path: self.index_path.to_string_lossy().into_owned(),
            indexed_threads,
            corpus_verified: false,
            requested_session_indexed,
            notice,
        })
    }

    pub fn status(&self) -> Result<StatusSummary> {
        let counts = self.count_totals()?;
        let files = self.count_rows("files")?;
        Ok(StatusSummary {
            index_path: self.index_path.to_string_lossy().into_owned(),
            fts_available: self.fts_available,
            index_format_version: self.index_format_version,
            rebuild_required: self.rebuild_required()?,
            sync_lock: self.sync_lock_status()?,
            files,
            threads: counts.0,
            messages: counts.1,
            events: counts.2,
        })
    }

    pub(crate) fn rebuild_required(&self) -> Result<bool> {
        read_rebuild_required(&self.conn).map_err(Into::into)
    }

    pub(crate) fn mark_rebuild_complete(&self) -> Result<()> {
        write_rebuild_required(&self.conn, false).map_err(Into::into)
    }

    fn count_totals(&self) -> Result<(usize, usize, usize)> {
        Ok((
            self.count_rows("threads")?,
            self.count_rows("messages")?,
            self.count_rows("events")?,
        ))
    }

    fn count_rows(&self, table: &str) -> Result<usize> {
        let sql = format!("SELECT COUNT(*) FROM {}", table);
        let count = self.conn.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
    }
}
