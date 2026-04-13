use std::io::{self, IsTerminal, Write};
use std::path::Path;

use crate::cli::SyncArgs;
use crate::index::{
    StatusSummary, Store, SyncCooldown, SyncCooldownPolicy, SyncFailure, SyncPreflight,
    SyncProgressEvent, SyncProgressObserver, SyncRequest, SyncResume, SyncScope, SyncStats,
};
use crate::output::Rendered;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SyncResponse {
    command: &'static str,
    ok: bool,
    partial: bool,
    scope: SyncScope,
    preflight: SyncPreflight,
    cooldown: SyncCooldown,
    progress: SyncProgressSummary,
    resume: SyncResume,
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

#[derive(Debug, Clone, Serialize)]
struct SyncProgressSummary {
    mode: String,
    current_phase: String,
    phases: Vec<String>,
    total_files: usize,
    processed_files: usize,
    indexed_files: usize,
    skipped_files: usize,
    failed_files: usize,
    partial: bool,
}

#[derive(Debug, Clone, Copy)]
enum SyncProgressMode {
    TtyBar,
    StderrLines,
}

impl SyncProgressMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::TtyBar => "tty-bar",
            Self::StderrLines => "stderr-lines",
        }
    }
}

struct SyncProgressReporter {
    mode: SyncProgressMode,
    summary: SyncProgressSummary,
    tty_line_active: bool,
    last_processed_reported: usize,
}

impl SyncProgressReporter {
    fn new() -> Self {
        let mode = if io::stderr().is_terminal() {
            SyncProgressMode::TtyBar
        } else {
            SyncProgressMode::StderrLines
        };
        Self {
            mode,
            summary: SyncProgressSummary {
                mode: mode.as_str().to_string(),
                current_phase: "idle".to_string(),
                phases: Vec::new(),
                total_files: 0,
                processed_files: 0,
                indexed_files: 0,
                skipped_files: 0,
                failed_files: 0,
                partial: false,
            },
            tty_line_active: false,
            last_processed_reported: 0,
        }
    }

    fn into_summary(self) -> SyncProgressSummary {
        self.summary
    }

    fn remember_phase(&mut self, phase_key: &str) {
        self.summary.current_phase = phase_key.to_string();
        if !self.summary.phases.iter().any(|phase| phase == phase_key) {
            self.summary.phases.push(phase_key.to_string());
        }
    }

    fn write_line(&mut self, line: &str) {
        if self.tty_line_active {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr);
            self.tty_line_active = false;
        }
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "{line}");
    }

    fn write_tty_progress(&mut self, phase_label: &str) {
        let line = render_tty_progress_line(
            phase_label,
            self.summary.processed_files,
            self.summary.total_files,
            self.summary.indexed_files,
            self.summary.skipped_files,
            self.summary.failed_files,
        );
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "\r{line}");
        let _ = stderr.flush();
        self.tty_line_active = true;
    }

    fn update_counts(
        &mut self,
        total_files: usize,
        processed_files: usize,
        indexed_files: usize,
        skipped_files: usize,
        failed_files: usize,
    ) {
        self.summary.total_files = total_files;
        self.summary.processed_files = processed_files;
        self.summary.indexed_files = indexed_files;
        self.summary.skipped_files = skipped_files;
        self.summary.failed_files = failed_files;
    }

    fn maybe_write_index_line(&mut self, phase_label: &str) {
        let step = progress_report_step(self.summary.total_files);
        if self.summary.processed_files == self.last_processed_reported
            || (self.summary.processed_files != self.summary.total_files
                && self
                    .summary
                    .processed_files
                    .saturating_sub(self.last_processed_reported)
                    < step)
        {
            return;
        }
        self.last_processed_reported = self.summary.processed_files;
        self.write_line(&format!(
            "阶段: {phase_label} {}/{}（新增/重建 {}，跳过 {}，失败 {}）",
            self.summary.processed_files,
            self.summary.total_files,
            self.summary.indexed_files,
            self.summary.skipped_files,
            self.summary.failed_files,
        ));
    }
}

impl SyncProgressObserver for SyncProgressReporter {
    fn on_event(&mut self, event: SyncProgressEvent) {
        match event {
            SyncProgressEvent::ScanStarted => {
                self.remember_phase("scanning");
                self.write_line("阶段: 扫描候选文件");
            }
            SyncProgressEvent::ScanProgress {
                visited_entries,
                discovered_files,
            } => {
                if matches!(self.mode, SyncProgressMode::StderrLines)
                    && discovered_files > 0
                    && visited_entries > discovered_files
                {
                    self.write_line(&format!(
                        "阶段: 扫描候选文件 已遍历 {} 项，命中 {} 个候选",
                        visited_entries, discovered_files
                    ));
                }
            }
            SyncProgressEvent::IndexStarted {
                total_files,
                processed_files,
            } => {
                self.remember_phase("indexing");
                self.update_counts(total_files, processed_files, 0, processed_files, 0);
                self.last_processed_reported = processed_files;
                match self.mode {
                    SyncProgressMode::TtyBar => self.write_tty_progress("写入索引"),
                    SyncProgressMode::StderrLines => {
                        self.write_line(&format!("阶段: 写入索引 {processed_files}/{total_files}"))
                    }
                }
            }
            SyncProgressEvent::IndexProgress {
                processed_files,
                total_files,
                indexed_files,
                skipped_files,
                failed_files,
            } => {
                self.update_counts(
                    total_files,
                    processed_files,
                    indexed_files,
                    skipped_files,
                    failed_files,
                );
                match self.mode {
                    SyncProgressMode::TtyBar => self.write_tty_progress("写入索引"),
                    SyncProgressMode::StderrLines => self.maybe_write_index_line("写入索引"),
                }
            }
            SyncProgressEvent::Finished {
                total_files,
                processed_files,
                indexed_files,
                skipped_files,
                failed_files,
                partial,
            } => {
                self.remember_phase("finished");
                self.summary.partial = partial;
                self.update_counts(
                    total_files,
                    processed_files,
                    indexed_files,
                    skipped_files,
                    failed_files,
                );
                self.write_line(&format!(
                    "阶段: 完成 {processed_files}/{total_files}（新增/重建 {}，跳过 {}，失败 {}）",
                    indexed_files, skipped_files, failed_files
                ));
            }
        }
    }
}

pub fn run(
    store: &mut Store,
    sessions_dir: &Path,
    index_dir: &Path,
    args: &SyncArgs,
) -> Result<Rendered> {
    let request = build_request(args);
    let cooldown = parse_cooldown_policy(args)?;
    let mut sync_lock = store.acquire_sync_lock()?;
    sync_lock.heartbeat()?;
    let mut progress = SyncProgressReporter::new();
    let mut progress_observer: Option<&mut dyn SyncProgressObserver> = Some(&mut progress);
    let (plan, report) = store.run_sync(
        sessions_dir,
        &request,
        &cooldown,
        Some(&mut sync_lock),
        &mut progress_observer,
    )?;
    drop(progress_observer);
    let progress = progress.into_summary();
    let response = SyncResponse {
        command: "sync",
        ok: true,
        partial: report.partial,
        scope: plan.scope.clone(),
        preflight: plan.preflight.clone(),
        cooldown: report.cooldown.clone(),
        progress: progress.clone(),
        resume: report.resume.clone(),
        sessions_dir: sessions_dir.to_string_lossy().into_owned(),
        index_dir: index_dir.to_string_lossy().into_owned(),
        stats: report.stats.clone(),
        failures: report.failures.clone(),
    };

    let mut lines = vec![
        "同步范围".to_string(),
        format!(
            "时间起点: {}",
            render_scope_bound(plan.scope.since.as_deref())
        ),
        format!(
            "时间终点: {}",
            render_scope_bound(plan.scope.until.as_deref())
        ),
        format!(
            "路径过滤: {}",
            render_scope_path(plan.scope.path.as_deref())
        ),
        format!("最近活跃: {}", render_scope_recent(plan.scope.recent)),
        format!("预算文件: {}", render_scope_budget(plan.scope.budget_files)),
        format!("候选文件: {}", plan.scope.candidate_files),
        String::new(),
        "同步冷却".to_string(),
        format!("冷却时间: {}", report.cooldown.interval),
        format!("冷却状态: {}", render_cooldown_state(&report.cooldown)),
        format!(
            "最近刷新: {}",
            render_optional_time(report.cooldown.last_completed_at.as_deref())
        ),
        format!(
            "下次允许: {}",
            render_optional_time(report.cooldown.next_allowed_at.as_deref())
        ),
    ];
    if let Some(reason) = &report.cooldown.reason {
        lines.push(format!("冷却说明: {}", reason));
    }
    lines.extend([
        String::new(),
        "同步预检".to_string(),
        format!("会话文件: {}", plan.preflight.total_files),
        format!("变更文件: {}", plan.preflight.changed_files),
        format!("未变更文件: {}", plan.preflight.unchanged_files),
        format!("总大小: {}", format_bytes(plan.preflight.total_bytes)),
        format!(
            "最大文件: {}",
            format_bytes(plan.preflight.largest_file_bytes)
        ),
        format!("推荐动作: {}", render_action(&plan.preflight)),
        format!("原因: {}", plan.preflight.reason),
        String::new(),
        if report.cooldown.state == "active" {
            "同步完成（命中冷却时间）".to_string()
        } else if !report.failures.is_empty() {
            "同步完成（部分失败）".to_string()
        } else if report.resume.state == "saved" {
            "同步完成（部分完成）".to_string()
        } else if plan.preflight.recommended_action == "skip" {
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
        format!("续跑状态: {}", render_resume_state(&report.resume)),
        format!(
            "从续跑恢复: {}",
            render_bool(report.resume.resumed_from_checkpoint)
        ),
        format!("剩余文件: {}", report.resume.remaining_files),
    ]);
    if report.resume.state != "idle" {
        lines.push(format!("续跑状态文件: {}", report.resume.state_path));
    }
    if let Some(reason) = &report.resume.reason {
        lines.push(format!("续跑说明: {}", reason));
    }
    for failure in &report.failures {
        lines.push(format!("- {} {}", failure.path, failure.error));
    }
    let text = lines.join("\n");

    Rendered::new(text, &response)
}

fn build_request(args: &SyncArgs) -> SyncRequest {
    SyncRequest {
        since: args.since.clone(),
        until: args.until.clone(),
        path: args.path.clone(),
        recent: args.recent,
        budget_files: args.budget_files,
    }
}

fn parse_cooldown_policy(args: &SyncArgs) -> Result<SyncCooldownPolicy> {
    let interval = args.cooldown.clone().unwrap_or_else(|| "30m".to_string());
    let interval_seconds = parse_cooldown_interval(&interval)?;
    Ok(SyncCooldownPolicy {
        interval,
        interval_seconds,
        force: args.force,
    })
}

fn parse_cooldown_interval(raw: &str) -> Result<u64> {
    let value = raw.trim();
    if value.len() < 2 {
        bail!("--cooldown 需要使用类似 30m、45s 或 2h 的格式");
    }

    let (digits, unit) = value.split_at(value.len() - 1);
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        bail!("--cooldown 需要使用类似 30m、45s 或 2h 的格式");
    }

    let amount = digits
        .parse::<u64>()
        .map_err(|error| anyhow!("failed to parse cooldown interval: {}", error))?;
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        _ => bail!("--cooldown 仅支持 s、m、h 单位"),
    };
    amount
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow!("--cooldown 超出可支持的时间范围"))
}

fn render_action(preflight: &SyncPreflight) -> &'static str {
    match preflight.recommended_action.as_str() {
        "skip" => "跳过同步",
        "heavy-sync" => "执行同步（预计较重）",
        "rebuild-needed" => "建议重建",
        _ => "执行同步",
    }
}

fn render_scope_bound(value: Option<&str>) -> &str {
    value.unwrap_or("全部")
}

fn render_scope_path(value: Option<&str>) -> &str {
    value.unwrap_or("全部")
}

fn render_scope_recent(value: Option<usize>) -> String {
    match value {
        Some(limit) => format!("最近 {} 个文件", limit),
        None => "不限制".to_string(),
    }
}

fn render_scope_budget(value: Option<usize>) -> String {
    match value {
        Some(limit) => format!("每次最多 {} 个文件", limit),
        None => "不限制".to_string(),
    }
}

fn render_cooldown_state(cooldown: &SyncCooldown) -> &'static str {
    match cooldown.state.as_str() {
        "active" => "命中冷却时间",
        "bypassed" => "已通过 --force 跳过",
        _ => "可继续同步",
    }
}

fn render_optional_time(value: Option<&str>) -> &str {
    value.unwrap_or("无")
}

fn render_resume_state(resume: &SyncResume) -> &'static str {
    match resume.state.as_str() {
        "saved" => "已保存续跑状态",
        "completed" => "已完成续跑并清理状态",
        _ => "未启用",
    }
}

fn render_bool(value: bool) -> &'static str {
    if value {
        "是"
    } else {
        "否"
    }
}

fn render_tty_progress_line(
    phase_label: &str,
    processed_files: usize,
    total_files: usize,
    indexed_files: usize,
    skipped_files: usize,
    failed_files: usize,
) -> String {
    let width = 20usize;
    let filled = if total_files == 0 {
        0
    } else {
        ((processed_files.min(total_files) * width) + total_files - 1) / total_files
    };
    let bar = format!(
        "{}{}",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled.min(width)))
    );
    format!(
        "[{}] {} {}/{} 新增/重建 {} 跳过 {} 失败 {}",
        bar, phase_label, processed_files, total_files, indexed_files, skipped_files, failed_files
    )
}

fn progress_report_step(total_files: usize) -> usize {
    if total_files <= 10 {
        1
    } else {
        (total_files / 10).max(1)
    }
}

fn render_lock_state(lock: &crate::index::SyncLockStatus) -> &'static str {
    match lock.state.as_str() {
        "running" => "运行中",
        "stale" => "过期",
        _ => "空闲",
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

    let mut lines = vec![
        "索引状态".to_string(),
        format!("CLI 版本: {}", response.cli_version),
        format!("索引文件: {}", status.index_path),
        format!("FTS5 可用: {}", status.fts_available),
        format!("同步锁: {}", render_lock_state(&status.sync_lock)),
        format!("文件数: {}", status.files),
        format!("线程数: {}", status.threads),
        format!("消息数: {}", status.messages),
        format!("事件数: {}", status.events),
    ];
    if let Some(pid) = status.sync_lock.pid {
        lines.push(format!("锁 PID: {}", pid));
    }
    if let Some(started_at) = &status.sync_lock.started_at {
        lines.push(format!("开始时间: {}", started_at));
    }
    if let Some(heartbeat_at) = &status.sync_lock.heartbeat_at {
        lines.push(format!("最近心跳: {}", heartbeat_at));
    }
    if let Some(reason) = &status.sync_lock.reason {
        lines.push(format!("锁说明: {}", reason));
    }
    let text = lines.join("\n");

    Rendered::new(text, &response)
}
