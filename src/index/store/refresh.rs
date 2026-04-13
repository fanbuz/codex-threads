use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::super::types::SyncRequest;
use super::Store;

const SYNC_REFRESH_FILE_NAME: &str = "sync.refresh.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SyncRefreshState {
    pub request: SyncRequest,
    pub completed_at: String,
}

impl SyncRefreshState {
    pub(crate) fn new(request: SyncRequest) -> Result<Self> {
        Ok(Self {
            request,
            completed_at: now_rfc3339()?,
        })
    }
}

impl Store {
    pub(crate) fn load_sync_refresh_state(&self) -> Result<Option<SyncRefreshState>> {
        let path = self.sync_refresh_state_path();
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read sync refresh state {}", path.display()))?;
        let state = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse sync refresh state {}", path.display()))?;
        Ok(Some(state))
    }

    pub(crate) fn save_sync_refresh_state(&self, state: &SyncRefreshState) -> Result<()> {
        let path = self.sync_refresh_state_path();
        let payload =
            serde_json::to_vec_pretty(state).context("failed to serialize sync refresh state")?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write sync refresh state {}", path.display()))
    }

    pub(crate) fn sync_refresh_state_path(&self) -> PathBuf {
        self.index_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(SYNC_REFRESH_FILE_NAME)
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| anyhow!("failed to format current time: {}", error))
}
