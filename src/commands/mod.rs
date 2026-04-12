mod read;
mod search;
mod sync;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Result};

use crate::cli::{Cli, Command, EventsCommand, MessagesCommand, ThreadsCommand};
use crate::index::Store;
use crate::output::Rendered;

pub fn run(cli: Cli) -> Result<Rendered> {
    let sessions_dir = resolve_sessions_dir(cli.sessions_dir.as_deref())?;
    let index_dir = resolve_index_dir(cli.index_dir.as_deref())?;
    let mut store = Store::open(&index_dir)?;

    match cli.command {
        Command::Sync => run_with_timing(|| sync::run(&mut store, &sessions_dir, &index_dir)),
        Command::Status => sync::status(&store),
        Command::Threads { command } => match command {
            ThreadsCommand::Search(args) => {
                run_with_timing(|| search::threads(&store, &args.query, args.limit))
            }
            ThreadsCommand::Read(args) => {
                run_with_timing(|| read::thread(&store, &args.session_id, args.limit))
            }
        },
        Command::Messages { command } => match command {
            MessagesCommand::Search(args) => {
                run_with_timing(|| search::messages(&store, &args.query, args.limit))
            }
            MessagesCommand::Read(args) => {
                run_with_timing(|| read::messages(&store, &args.session_id, args.limit))
            }
        },
        Command::Events { command } => match command {
            EventsCommand::Read(args) => {
                run_with_timing(|| read::events(&store, &args.session_id, args.limit))
            }
        },
    }
}

fn run_with_timing(operation: impl FnOnce() -> Result<Rendered>) -> Result<Rendered> {
    let started_at = Instant::now();
    let rendered = operation()?;
    Ok(rendered.with_duration(started_at.elapsed()))
}

fn resolve_sessions_dir(value: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = value {
        return Ok(path.to_path_buf());
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法确定 home 目录"))?;
    Ok(home.join(".codex").join("sessions"))
}

fn resolve_index_dir(value: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = value {
        return Ok(path.to_path_buf());
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法确定 home 目录"))?;
    Ok(home.join(".codex").join("threads-index"))
}

#[allow(dead_code)]
fn ensure_exists(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        Ok(())
    } else {
        bail!("{}不存在: {}", label, path.display())
    }
}
