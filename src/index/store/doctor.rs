use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;

use super::super::types::{DoctorIssue, DoctorRepairAction, DoctorReport};
use super::refresh::SyncRefreshState;
use super::resume::SyncResumeState;
use super::Store;

#[derive(Debug)]
struct DoctorScan {
    issues: Vec<DoctorIssue>,
    repair_actions: Vec<RepairAction>,
}

#[derive(Debug, Clone)]
enum RepairAction {
    RemoveFile {
        code: &'static str,
        summary: String,
        path: PathBuf,
    },
}

impl RepairAction {
    fn apply(&self) -> Result<()> {
        match self {
            Self::RemoveFile { path, .. } => remove_file_if_exists(path),
        }
    }

    fn to_report(&self) -> DoctorRepairAction {
        match self {
            Self::RemoveFile {
                code,
                summary,
                path,
            } => DoctorRepairAction {
                code: (*code).to_string(),
                summary: summary.clone(),
                path: Some(path.to_string_lossy().into_owned()),
            },
        }
    }
}

impl Store {
    pub fn doctor(&self, repair: bool) -> Result<DoctorReport> {
        let initial_scan = self.collect_doctor_scan()?;
        let mut repaired_actions = Vec::new();

        if repair {
            // Re-run the checks after repair so the report reflects the current state,
            // not the problems that were already cleaned up during this invocation.
            for action in &initial_scan.repair_actions {
                action.apply()?;
                repaired_actions.push(action.to_report());
            }
        }

        let issues = if repair {
            self.collect_doctor_scan()?.issues
        } else {
            initial_scan.issues
        };
        let status_summary = self.status()?;
        let status = if issues.is_empty() {
            "healthy"
        } else {
            "attention"
        };

        Ok(DoctorReport {
            status: status.to_string(),
            recommendation: build_recommendation(&issues),
            issues,
            repaired_actions,
            status_summary,
        })
    }

    fn collect_doctor_scan(&self) -> Result<DoctorScan> {
        let mut issues = Vec::new();
        let mut repair_actions = Vec::new();

        self.scan_sync_lock(&mut issues, &mut repair_actions)?;
        self.scan_resume_state(&mut issues, &mut repair_actions)?;
        self.scan_refresh_state(&mut issues, &mut repair_actions)?;
        self.scan_thread_count_drift(&mut issues)?;

        Ok(DoctorScan {
            issues,
            repair_actions,
        })
    }

    fn scan_sync_lock(
        &self,
        issues: &mut Vec<DoctorIssue>,
        repair_actions: &mut Vec<RepairAction>,
    ) -> Result<()> {
        let status = self.sync_lock_status()?;
        match status.state.as_str() {
            "stale" => {
                let summary = match &status.reason {
                    Some(reason) => {
                        format!("检测到过期的同步锁，可安全清理后恢复后续同步: {reason}")
                    }
                    None => "检测到过期的同步锁，可安全清理后恢复后续同步".to_string(),
                };
                issues.push(DoctorIssue {
                    code: "stale_lock".to_string(),
                    summary,
                    repairable: true,
                    path: Some(status.lock_path.clone()),
                });
                repair_actions.push(RepairAction::RemoveFile {
                    code: "stale_lock",
                    summary: "已清理过期的同步锁文件".to_string(),
                    path: PathBuf::from(status.lock_path),
                });
            }
            "running" => {
                issues.push(DoctorIssue {
                    code: "sync_in_progress".to_string(),
                    summary: "检测到同步仍在进行，请等待当前同步完成后再重新检查".to_string(),
                    repairable: false,
                    path: Some(status.lock_path),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn scan_resume_state(
        &self,
        issues: &mut Vec<DoctorIssue>,
        repair_actions: &mut Vec<RepairAction>,
    ) -> Result<()> {
        let path = self.sync_resume_state_path();
        if !path.exists() {
            return Ok(());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read sync resume state {}", path.display()))?;
        match serde_json::from_str::<SyncResumeState>(&raw) {
            Ok(state) if state.pending_paths.is_empty() => {
                issues.push(DoctorIssue {
                    code: "empty_resume_state".to_string(),
                    summary: "续跑状态文件里没有待处理文件，说明本地检查点已经失效".to_string(),
                    repairable: true,
                    path: Some(path.to_string_lossy().into_owned()),
                });
                repair_actions.push(RepairAction::RemoveFile {
                    code: "empty_resume_state",
                    summary: "已清理空的续跑状态文件".to_string(),
                    path,
                });
            }
            Ok(_) => {}
            Err(_) => {
                issues.push(DoctorIssue {
                    code: "invalid_resume_state".to_string(),
                    summary: "续跑状态文件已损坏，无法继续安全恢复增量同步".to_string(),
                    repairable: true,
                    path: Some(path.to_string_lossy().into_owned()),
                });
                repair_actions.push(RepairAction::RemoveFile {
                    code: "invalid_resume_state",
                    summary: "已清理损坏的续跑状态文件".to_string(),
                    path,
                });
            }
        }

        Ok(())
    }

    fn scan_refresh_state(
        &self,
        issues: &mut Vec<DoctorIssue>,
        repair_actions: &mut Vec<RepairAction>,
    ) -> Result<()> {
        let path = self.sync_refresh_state_path();
        if !path.exists() {
            return Ok(());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read sync refresh state {}", path.display()))?;
        if serde_json::from_str::<SyncRefreshState>(&raw).is_err() {
            issues.push(DoctorIssue {
                code: "invalid_refresh_state".to_string(),
                summary: "冷却状态文件已损坏，可能导致后续同步判断失真".to_string(),
                repairable: true,
                path: Some(path.to_string_lossy().into_owned()),
            });
            repair_actions.push(RepairAction::RemoveFile {
                code: "invalid_refresh_state",
                summary: "已清理损坏的冷却状态文件".to_string(),
                path,
            });
        }

        Ok(())
    }

    fn scan_thread_count_drift(&self, issues: &mut Vec<DoctorIssue>) -> Result<()> {
        let message_mismatches = self.count_thread_message_mismatches()?;
        if message_mismatches > 0 {
            issues.push(DoctorIssue {
                code: "thread_message_count_mismatch".to_string(),
                summary: format!(
                    "有 {message_mismatches} 个线程的消息计数与明细表不一致，建议重新同步重建统计"
                ),
                repairable: false,
                path: None,
            });
        }

        let event_mismatches = self.count_thread_event_mismatches()?;
        if event_mismatches > 0 {
            issues.push(DoctorIssue {
                code: "thread_event_count_mismatch".to_string(),
                summary: format!(
                    "有 {event_mismatches} 个线程的事件计数与明细表不一致，建议重新同步重建统计"
                ),
                repairable: false,
                path: None,
            });
        }

        Ok(())
    }

    fn count_thread_message_mismatches(&self) -> Result<usize> {
        count_mismatches(
            &self.conn,
            r#"
            SELECT COUNT(*) FROM (
                SELECT t.session_id
                FROM threads t
                LEFT JOIN (
                    SELECT session_id, COUNT(*) AS actual_count
                    FROM messages
                    GROUP BY session_id
                ) m ON m.session_id = t.session_id
                WHERE t.message_count != COALESCE(m.actual_count, 0)
            )
            "#,
        )
    }

    fn count_thread_event_mismatches(&self) -> Result<usize> {
        count_mismatches(
            &self.conn,
            r#"
            SELECT COUNT(*) FROM (
                SELECT t.session_id
                FROM threads t
                LEFT JOIN (
                    SELECT session_id, COUNT(*) AS actual_count
                    FROM events
                    GROUP BY session_id
                ) e ON e.session_id = t.session_id
                WHERE t.event_count != COALESCE(e.actual_count, 0)
            )
            "#,
        )
    }
}

fn count_mismatches(conn: &rusqlite::Connection, sql: &str) -> Result<usize> {
    let count = conn
        .query_row(sql, [], |row| row.get::<_, Option<i64>>(0))
        .optional()?
        .flatten()
        .unwrap_or_default();
    Ok(count as usize)
}

fn build_recommendation(issues: &[DoctorIssue]) -> String {
    if issues.is_empty() {
        return "可以继续使用当前索引".to_string();
    }

    let has_repairable = issues.iter().any(|issue| issue.repairable);
    let has_drift = issues.iter().any(|issue| {
        matches!(
            issue.code.as_str(),
            "thread_message_count_mismatch" | "thread_event_count_mismatch"
        )
    });
    let has_running_sync = issues.iter().any(|issue| issue.code == "sync_in_progress");

    match (has_repairable, has_drift, has_running_sync) {
        (true, true, _) => {
            "可先运行 codex-threads doctor --repair 清理本地状态，再执行重新同步处理索引漂移"
                .to_string()
        }
        (true, false, _) => "可运行 codex-threads doctor --repair 清理本地状态问题".to_string(),
        (false, true, _) => "建议重新同步以重建受影响的索引统计".to_string(),
        (false, false, true) => "请等待当前同步完成后，再重新执行 doctor 检查".to_string(),
        _ => "请根据问题描述继续处理".to_string(),
    }
}

fn remove_file_if_exists(path: &PathBuf) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove doctor repair target {}", path.display())),
    }
}
