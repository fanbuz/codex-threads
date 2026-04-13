use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::index::{StatusSummary, Store, SyncFailure, SyncPreflight, SyncStats};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct SyncResponse {
    command: &'static str,
    ok: bool,
    partial: bool,
    preflight: SyncPreflight,
    sessions_dir: String,
    index_dir: String,
    stats: SyncStats,
    failures: Vec<SyncFailure>,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    command: &'static str,
    ok: bool,
    cli_version: &'static str,
    status: StatusSummary,
}

pub fn run(store: &mut Store, sessions_dir: &Path, index_dir: &Path) -> Result<Rendered> {
    let preflight = store.preflight_sync(sessions_dir)?;
    let report = if preflight.recommended_action == "skip" {
        store.skip_sync_report(sessions_dir, &preflight)?
    } else {
        store.sync_sessions(sessions_dir)?
    };
    let response = SyncResponse {
        command: "sync",
        ok: true,
        partial: report.partial,
        preflight: preflight.clone(),
        sessions_dir: sessions_dir.to_string_lossy().into_owned(),
        index_dir: index_dir.to_string_lossy().into_owned(),
        stats: report.stats.clone(),
        failures: report.failures.clone(),
    };

    let mut lines = vec![
        "同步预检".to_string(),
        format!("会话文件: {}", preflight.total_files),
        format!("变更文件: {}", preflight.changed_files),
        format!("未变更文件: {}", preflight.unchanged_files),
        format!("总大小: {}", format_bytes(preflight.total_bytes)),
        format!("最大文件: {}", format_bytes(preflight.largest_file_bytes)),
        format!("推荐动作: {}", render_action(&preflight)),
        format!("原因: {}", preflight.reason),
        String::new(),
        if report.partial {
            "同步完成（部分失败）".to_string()
        } else if preflight.recommended_action == "skip" {
            "同步完成（无需更新）".to_string()
        } else {
            "同步完成".to_string()
        },
        format!("会话目录: {}", response.sessions_dir),
        format!("索引目录: {}", response.index_dir),
        format!("扫描文件: {}", report.stats.scanned_files),
        format!("新增/重建: {}", report.stats.indexed_files),
        format!("跳过未变更: {}", report.stats.skipped_files),
        format!("失败文件: {}", report.stats.failed_files),
        format!("移除失效文件: {}", report.stats.removed_files),
        format!("线程总数: {}", report.stats.threads),
        format!("消息总数: {}", report.stats.messages),
        format!("事件总数: {}", report.stats.events),
    ];
    for failure in &report.failures {
        lines.push(format!("- {} {}", failure.path, failure.error));
    }
    let text = lines.join("\n");

    Rendered::new(text, &response)
}

fn render_action(preflight: &SyncPreflight) -> &'static str {
    match preflight.recommended_action.as_str() {
        "skip" => "跳过同步",
        "heavy-sync" => "执行同步（预计较重）",
        "rebuild-needed" => "建议重建",
        _ => "执行同步",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1}GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes / KB)
    } else {
        format!("{}B", bytes as u64)
    }
}

pub fn status(store: &Store) -> Result<Rendered> {
    let status = store.status()?;
    let response = StatusResponse {
        command: "status",
        ok: true,
        cli_version: env!("CARGO_PKG_VERSION"),
        status: status.clone(),
    };

    let text = [
        "索引状态".to_string(),
        format!("CLI 版本: {}", response.cli_version),
        format!("索引文件: {}", status.index_path),
        format!("FTS5 可用: {}", status.fts_available),
        format!("文件数: {}", status.files),
        format!("线程数: {}", status.threads),
        format!("消息数: {}", status.messages),
        format!("事件数: {}", status.events),
    ]
    .join("\n");

    Rendered::new(text, &response)
}
