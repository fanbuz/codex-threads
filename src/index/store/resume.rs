use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::super::types::{SyncRequest, SyncResume};
use super::Store;

const SYNC_RESUME_FILE_NAME: &str = "sync.resume.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SyncResumeState {
    pub request: SyncRequest,
    pub pending_paths: Vec<String>,
    pub saved_at: String,
}

impl SyncResumeState {
    pub(crate) fn new(request: SyncRequest, pending_paths: Vec<String>) -> Result<Self> {
        Ok(Self {
            request,
            pending_paths,
            saved_at: now_rfc3339()?,
        })
    }
}

impl Store {
    pub(crate) fn load_sync_resume_state(&self) -> Result<Option<SyncResumeState>> {
        let path = self.sync_resume_state_path();
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read sync resume state {}", path.display()))?;
        let state = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse sync resume state {}", path.display()))?;
        Ok(Some(state))
    }

    pub(crate) fn save_sync_resume_state(&self, state: &SyncResumeState) -> Result<()> {
        let path = self.sync_resume_state_path();
        let payload =
            serde_json::to_vec_pretty(state).context("failed to serialize sync resume state")?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write sync resume state {}", path.display()))
    }

    pub(crate) fn clear_sync_resume_state(&self) -> Result<()> {
        let path = self.sync_resume_state_path();
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove sync resume state {}", path.display()))
    }

    pub(crate) fn sync_resume_state_path(&self) -> PathBuf {
        self.index_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(SYNC_RESUME_FILE_NAME)
    }
}

pub(crate) fn build_sync_resume(
    path: PathBuf,
    state: &str,
    budget_files: Option<usize>,
    resumed_from_checkpoint: bool,
    processed_files: usize,
    remaining_files: usize,
    reason: Option<String>,
) -> SyncResume {
    SyncResume {
        state: state.to_string(),
        state_path: path.to_string_lossy().into_owned(),
        budget_files,
        resumed_from_checkpoint,
        processed_files,
        remaining_files,
        reason,
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| anyhow!("failed to format current time: {}", error))
}
