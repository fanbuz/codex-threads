use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::app_state::{restore_app_thread, RestoreAppThreadReport};
use crate::cli::{ExperimentalCommand, RestoreAppThreadArgs};
use crate::experimental::{ExperimentalFeature, ExperimentalFeatures};
use crate::output::Rendered;

#[derive(Debug, Serialize)]
struct ExperimentalResponse {
    command: &'static str,
    ok: bool,
    cli_version: &'static str,
    experimental: RestoreAppThreadReport,
}

pub fn run(
    command: ExperimentalCommand,
    experimentals: &ExperimentalFeatures,
    sessions_dir: &Path,
) -> Result<Rendered> {
    match command {
        ExperimentalCommand::RestoreAppThread(args) => {
            experimentals.ensure_enabled(ExperimentalFeature::RestoreAppThread)?;
            restore_thread_view(args, sessions_dir)
        }
    }
}

fn restore_thread_view(args: RestoreAppThreadArgs, sessions_dir: &Path) -> Result<Rendered> {
    let codex_home = resolve_codex_home(args.codex_home.as_deref())?;
    let report = restore_app_thread(
        &args.thread_id,
        sessions_dir,
        &codex_home,
        args.pin,
        args.dry_run,
    )?;

    let response = ExperimentalResponse {
        command: "experimental.restore-app-thread",
        ok: true,
        cli_version: env!("CARGO_PKG_VERSION"),
        experimental: report.clone(),
    };

    let mut lines = vec![
        "实验能力：restore-app-thread".to_string(),
        format!("线程 ID: {}", report.thread_id),
        format!(
            "执行模式: {}",
            if report.dry_run { "dry-run" } else { "apply" }
        ),
        format!("线程处理: {}", render_thread_action(&report.thread_action)),
        format!("Pin 处理: {}", render_pin_action(&report.pin_action)),
        format!("状态库: {}", report.state_db_path),
        format!("全局状态: {}", report.global_state_path),
        format!("线程标题: {}", report.thread.title),
        format!("线程路径: {}", report.thread.rollout_path),
    ];

    if let Some(backup_dir) = &report.backup_dir {
        lines.push(format!("备份目录: {}", backup_dir));
    }

    if !report.warnings.is_empty() {
        lines.push(String::new());
        lines.push("提示".to_string());
        lines.extend(
            report
                .warnings
                .iter()
                .map(|warning| format!("- {}", warning)),
        );
    }

    Rendered::new(lines.join("\n"), &response).map(|rendered| rendered.with_duration_after_line(3))
}

fn resolve_codex_home(value: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = value {
        return Ok(path.to_path_buf());
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow!("无法确定 home 目录"))?;
    Ok(home.join(".codex"))
}

fn render_thread_action(action: &str) -> &'static str {
    match action {
        "would_insert" => "将新增线程记录",
        "inserted" => "已新增线程记录",
        "would_update_existing" => "将更新现有线程记录",
        "updated_existing" => "已更新现有线程记录",
        _ => "未变更",
    }
}

fn render_pin_action(action: &str) -> &'static str {
    match action {
        "would_pin" => "将加入 pinned-thread-ids",
        "pinned" => "已加入 pinned-thread-ids",
        "already_pinned" => "已经在 pinned-thread-ids 中",
        _ => "未请求",
    }
}
