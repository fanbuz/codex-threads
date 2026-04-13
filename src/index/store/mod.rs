mod read;
mod search;
mod sync;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::schema::init_schema;
use super::types::StatusSummary;

#[derive(Debug)]
pub struct Store {
    conn: Connection,
    index_path: PathBuf,
    fts_available: bool,
}

impl Store {
    pub fn open(index_dir: &Path) -> Result<Self> {
        fs::create_dir_all(index_dir)
            .with_context(|| format!("failed to create {}", index_dir.display()))?;
        let index_path = index_dir.join("threads.sqlite3");
        let conn = Connection::open(&index_path)
            .with_context(|| format!("failed to open {}", index_path.display()))?;
        let fts_available = init_schema(&conn)?;
        Ok(Self {
            conn,
            index_path,
            fts_available,
        })
    }

    pub fn status(&self) -> Result<StatusSummary> {
        let counts = self.count_totals()?;
        let files = self.count_rows("files")?;
        Ok(StatusSummary {
            index_path: self.index_path.to_string_lossy().into_owned(),
            fts_available: self.fts_available,
            files,
            threads: counts.0,
            messages: counts.1,
            events: counts.2,
        })
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
