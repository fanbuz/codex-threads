use serde::Serialize;

use super::search_meta::SearchExplain;

#[derive(Debug, Clone, Serialize)]
pub struct SyncStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusSummary {
    pub index_path: String,
    pub fts_available: bool,
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
