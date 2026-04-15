mod doctor;
mod experimental;
mod read;
mod search;
mod sync;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Result};

use crate::cli::{Cli, Command, EventsCommand, MessagesCommand, ThreadsCommand};
use crate::experimental::ExperimentalFeatures;
use crate::index::Store;
use crate::output::Rendered;

pub fn run(cli: Cli) -> Result<Rendered> {
    let experimentals = ExperimentalFeatures::parse_csv(cli.enable_experimentals.as_deref())?;
    let sessions_dir = resolve_sessions_dir(cli.sessions_dir.as_deref())?;
    let index_dir = resolve_index_dir(cli.index_dir.as_deref())?;

    match cli.command {
        Command::Sync(args) => run_with_timing(|| {
            let mut store = Store::open(&index_dir)?;
            sync::run(&mut store, &sessions_dir, &index_dir, &args)
        }),
        Command::Status => {
            let store = Store::open(&index_dir)?;
            sync::status(&store)
        }
        Command::Doctor(args) => run_with_timing(|| {
            let store = Store::open(&index_dir)?;
            doctor::run(&store, &args)
        }),
        Command::Experimental { command } => {
            run_with_timing(|| experimental::run(command, &experimentals, &sessions_dir))
        }
        Command::Threads { command } => match command {
            ThreadsCommand::Search(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                search::threads(&store, &args)
            }),
            ThreadsCommand::Read(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                read::thread(&store, &args.session_id, args.limit)
            }),
        },
        Command::Messages { command } => match command {
            MessagesCommand::Search(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                search::messages(&store, &args)
            }),
            MessagesCommand::Read(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                read::messages(&store, &args.session_id, args.limit)
            }),
        },
        Command::Events { command } => match command {
            EventsCommand::Search(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                search::events(&store, &args)
            }),
            EventsCommand::Read(args) => run_with_timing(|| {
                let store = Store::open(&index_dir)?;
                read::events(&store, &args.session_id, args.limit)
            }),
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
