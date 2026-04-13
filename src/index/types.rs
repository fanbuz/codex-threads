use serde::{Deserialize, Serialize};

use super::search_meta::SearchExplain;

#[derive(Debug, Clone, Serialize)]
pub struct SyncStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub appended_files: usize,
    pub rebuilt_files: usize,
    pub fallback_rebuilt_files: usize,
    pub skipped_files: usize,
    pub failed_files: usize,
    pub removed_files: usize,
    pub threads: usize,
    pub messages: usize,
    pub events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub stats: SyncStats,
    pub partial: bool,
    pub failures: Vec<SyncFailure>,
    pub cooldown: SyncCooldown,
    pub resume: SyncResume,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncCooldown {
    pub state: String,
    pub interval: String,
    pub interval_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_allowed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyncCooldownPolicy {
    pub interval: String,
    pub interval_seconds: u64,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorIssue {
    pub code: String,
    pub summary: String,
    pub repairable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorRepairAction {
    pub code: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub status: String,
    pub recommendation: String,
    pub issues: Vec<DoctorIssue>,
    pub repaired_actions: Vec<DoctorRepairAction>,
    pub status_summary: StatusSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPreflight {
    pub total_files: usize,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub total_bytes: u64,
    pub largest_file_bytes: u64,
    pub recommended_action: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncLockStatus {
    pub state: String,
    pub lock_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heartbeat_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResume {
    pub state: String,
    pub state_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_files: Option<usize>,
    pub resumed_from_checkpoint: bool,
    pub processed_files: usize,
    pub remaining_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRequest {
    pub since: Option<String>,
    pub until: Option<String>,
    pub path: Option<String>,
    pub recent: Option<usize>,
    pub budget_files: Option<usize>,
}

impl SyncRequest {
    pub fn is_scoped(&self) -> bool {
        self.since.is_some()
            || self.until.is_some()
            || self.path.is_some()
            || self.recent.is_some()
            || self.budget_files.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncScope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_files: Option<usize>,
    pub candidate_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPlan {
    pub scope: SyncScope,
    pub preflight: SyncPreflight,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusSummary {
    pub index_path: String,
    pub fts_available: bool,
    pub sync_lock: SyncLockStatus,
    pub files: usize,
    pub threads: usize,
    pub messages: usize,
    pub events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadRecord {
    pub session_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub path: String,
    pub file_name: String,
    pub folder: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub message_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadSearchHit {
    pub session_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub path: String,
    pub message_count: usize,
    pub event_count: usize,
    pub snippet: String,
    pub explain: SearchExplain,
    #[serde(skip_serializing)]
    pub aggregate_text: String,
    #[serde(skip_serializing)]
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ThreadSearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub session_id: String,
    pub timestamp: Option<String>,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageSearchHit {
    pub session_id: String,
    pub title: Option<String>,
    pub timestamp: Option<String>,
    pub role: String,
    pub text: String,
    pub snippet: String,
    pub explain: SearchExplain,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MessageSearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub session_id: String,
    pub timestamp: Option<String>,
    pub event_type: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSearchHit {
    pub session_id: String,
    pub title: Option<String>,
    pub timestamp: Option<String>,
    pub event_type: String,
    pub summary: String,
    pub snippet: String,
    pub explain: SearchExplain,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EventSearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadRead {
    pub thread: ThreadRecord,
    pub messages: Vec<MessageRecord>,
}
