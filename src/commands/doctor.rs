use anyhow::Result;
use serde::Serialize;

use crate::cli::DoctorArgs;
use crate::index::{DoctorReport, Store, SyncLockStatus};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct DoctorResponse {
    command: &'static str,
    ok: bool,
    cli_version: &'static str,
    doctor: DoctorReport,
}

pub fn run(store: &Store, args: &DoctorArgs) -> Result<Rendered> {
    let report = store.doctor(args.repair)?;
    let response = DoctorResponse {
        command: "doctor",
        ok: true,
        cli_version: env!("CARGO_PKG_VERSION"),
        doctor: report.clone(),
    };

    let mut lines = vec![
        "索引健康检查".to_string(),
        format!("健康状态: {}", render_status(&report.status)),
        format!("发现问题: {}", report.issues.len()),
        format!("修复动作: {}", report.repaired_actions.len()),
        format!("建议: {}", report.recommendation),
        String::new(),
        "当前索引".to_string(),
        format!("索引文件: {}", report.status_summary.index_path),
        format!("FTS5 可用: {}", report.status_summary.fts_available),
        format!(
            "同步锁: {}",
            render_lock_state(&report.status_summary.sync_lock)
        ),
        format!("文件数: {}", report.status_summary.files),
        format!("线程数: {}", report.status_summary.threads),
        format!("消息数: {}", report.status_summary.messages),
        format!("事件数: {}", report.status_summary.events),
    ];

    if let Some(pid) = report.status_summary.sync_lock.pid {
        lines.push(format!("锁 PID: {}", pid));
    }
    if let Some(reason) = &report.status_summary.sync_lock.reason {
        lines.push(format!("锁说明: {}", reason));
    }

    if !report.issues.is_empty() {
        lines.push(String::new());
        lines.push("问题列表".to_string());
        lines.extend(report.issues.iter().map(render_issue_line));
    }

    if !report.repaired_actions.is_empty() {
        lines.push(String::new());
        lines.push("修复记录".to_string());
        lines.extend(report.repaired_actions.iter().map(render_repair_line));
    }

    Rendered::new(lines.join("\n"), &response).map(|rendered| rendered.with_duration_after_line(3))
}

fn render_status(status: &str) -> &'static str {
    match status {
        "healthy" => "正常",
        _ => "需关注",
    }
}

fn render_lock_state(lock: &SyncLockStatus) -> &'static str {
    match lock.state.as_str() {
        "running" => "运行中",
        "stale" => "过期",
        _ => "空闲",
    }
}

fn render_issue_line(issue: &crate::index::DoctorIssue) -> String {
    match &issue.path {
        Some(path) => format!("- [{}] {} ({})", issue.code, issue.summary, path),
        None => format!("- [{}] {}", issue.code, issue.summary),
    }
}

fn render_repair_line(action: &crate::index::DoctorRepairAction) -> String {
    match &action.path {
        Some(path) => format!("- [{}] {} ({})", action.code, action.summary, path),
        None => format!("- [{}] {}", action.code, action.summary),
    }
}
