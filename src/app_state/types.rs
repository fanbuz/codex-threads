use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RestoredAppThread {
    pub id: String,
    pub rollout_path: String,
    pub title: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreAppThreadReport {
    pub feature: String,
    pub dry_run: bool,
    pub thread_id: String,
    pub thread_action: String,
    pub pin_action: String,
    pub state_db_path: String,
    pub global_state_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,
    pub warnings: Vec<String>,
    pub thread: RestoredAppThread,
}
