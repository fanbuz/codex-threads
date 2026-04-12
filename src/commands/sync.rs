use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::index::{StatusSummary, Store, SyncFailure, SyncStats};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct SyncResponse {
    command: &'static str,
    ok: bool,
    partial: bool,
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
    let report = store.sync_sessions(sessions_dir)?;
    let response = SyncResponse {
        command: "sync",
        ok: true,
        partial: report.partial,
        sessions_dir: sessions_dir.to_string_lossy().into_owned(),
        index_dir: index_dir.to_string_lossy().into_owned(),
        stats: report.stats.clone(),
        failures: report.failures.clone(),
    };

    let mut lines = vec![
        if report.partial {
            "同步完成（部分失败）".to_string()
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
