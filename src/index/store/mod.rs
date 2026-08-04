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
use super::types::StatusSummary;

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
