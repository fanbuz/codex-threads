use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use super::super::types::SyncLockStatus;
use super::Store;

const SYNC_LOCK_FILE_NAME: &str = "sync.lock.json";
const STALE_LOCK_WINDOW: Duration = Duration::minutes(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncLockFile {
    pid: u32,
    command: String,
    index_path: String,
    started_at: String,
    heartbeat_at: String,
}

#[derive(Debug)]
pub(crate) struct SyncLockGuard {
    path: PathBuf,
    lock: SyncLockFile,
}

impl SyncLockGuard {
    pub(crate) fn heartbeat(&mut self) -> Result<()> {
        self.lock.heartbeat_at = now_rfc3339()?;
        write_lock_file(&self.path, &self.lock)
    }
}

impl Drop for SyncLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Store {
    pub(crate) fn acquire_sync_lock(&self) -> Result<SyncLockGuard> {
        let path = self.sync_lock_path();
        let now = now_rfc3339()?;
        let lock = SyncLockFile {
            pid: std::process::id(),
            command: "sync".to_string(),
            index_path: self.index_path.to_string_lossy().into_owned(),
            started_at: now.clone(),
            heartbeat_at: now,
        };

        for _ in 0..2 {
            match try_create_lock_file(&path, &lock) {
                Ok(()) => {
                    return Ok(SyncLockGuard {
                        path: path.clone(),
                        lock,
                    });
                }
                Err(CreateLockError::AlreadyExists) => {
                    let status = read_sync_lock_status(&path)?;
                    // A stale lock should not block future sync runs forever.
                    // Reclaim it eagerly once the heartbeat is outside the allowed window.
                    if status.state == "stale" {
                        fs::remove_file(&path).with_context(|| {
                            format!("failed to remove stale sync lock {}", path.display())
                        })?;
                        continue;
                    }
                    bail!("{}", describe_lock_conflict(&status));
                }
                Err(CreateLockError::Io(error)) => return Err(error),
            }
        }

        bail!("无法接管过期的 sync 锁: {}", path.display())
    }

    pub(crate) fn sync_lock_status(&self) -> Result<SyncLockStatus> {
        read_sync_lock_status(&self.sync_lock_path())
    }

    fn sync_lock_path(&self) -> PathBuf {
        self.index_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(SYNC_LOCK_FILE_NAME)
    }
}

enum CreateLockError {
    AlreadyExists,
    Io(anyhow::Error),
}

fn try_create_lock_file(
    path: &Path,
    lock: &SyncLockFile,
) -> std::result::Result<(), CreateLockError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            ErrorKind::AlreadyExists => CreateLockError::AlreadyExists,
            _ => CreateLockError::Io(
                anyhow!(error).context(format!("failed to create sync lock {}", path.display())),
            ),
        })?;
    let payload = serde_json::to_vec(lock).map_err(|error| {
        CreateLockError::Io(anyhow!(error).context("failed to serialize sync lock"))
    })?;
    file.write_all(&payload).map_err(|error| {
        CreateLockError::Io(
            anyhow!(error).context(format!("failed to write sync lock {}", path.display())),
        )
    })
}

fn write_lock_file(path: &Path, lock: &SyncLockFile) -> Result<()> {
    let payload = serde_json::to_vec(lock).context("failed to serialize sync lock")?;
    fs::write(path, payload)
        .with_context(|| format!("failed to update sync lock {}", path.display()))
}

fn read_sync_lock_status(path: &Path) -> Result<SyncLockStatus> {
    if !path.exists() {
        return Ok(SyncLockStatus {
            state: "idle".to_string(),
            lock_path: path.to_string_lossy().into_owned(),
            pid: None,
            started_at: None,
            heartbeat_at: None,
            reason: None,
        });
    }

    let lock_path = path.to_string_lossy().into_owned();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read sync lock {}", path.display()))?;

    match serde_json::from_str::<SyncLockFile>(&raw) {
        Ok(lock) => {
            let stale = is_stale(&lock.heartbeat_at).unwrap_or(true);
            Ok(SyncLockStatus {
                state: if stale { "stale" } else { "running" }.to_string(),
                lock_path,
                pid: Some(lock.pid),
                started_at: Some(lock.started_at),
                heartbeat_at: Some(lock.heartbeat_at),
                reason: if stale {
                    Some("最近心跳已过期".to_string())
                } else {
                    None
                },
            })
        }
        Err(error) => {
            let stale = is_stale_by_mtime(path).unwrap_or(true);
            Ok(SyncLockStatus {
                state: if stale { "stale" } else { "running" }.to_string(),
                lock_path,
                pid: None,
                started_at: None,
                heartbeat_at: None,
                reason: Some(format!("锁文件不可解析: {}", error)),
            })
        }
    }
}

fn describe_lock_conflict(status: &SyncLockStatus) -> String {
    let mut details = Vec::new();
    if let Some(pid) = status.pid {
        details.push(format!("pid={}", pid));
    }
    if let Some(heartbeat_at) = &status.heartbeat_at {
        details.push(format!("最近心跳 {}", heartbeat_at));
    }
    if let Some(reason) = &status.reason {
        details.push(reason.clone());
    }

    if details.is_empty() {
        "已有 sync 正在运行，请稍后重试".to_string()
    } else {
        format!("已有 sync 正在运行（{}），请稍后重试", details.join("，"))
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| anyhow!("failed to format current time: {}", error))
}

fn is_stale(heartbeat_at: &str) -> Result<bool> {
    let heartbeat = OffsetDateTime::parse(heartbeat_at, &Rfc3339)
        .with_context(|| format!("invalid sync lock heartbeat {}", heartbeat_at))?;
    Ok(OffsetDateTime::now_utc() - heartbeat > STALE_LOCK_WINDOW)
}

fn is_stale_by_mtime(path: &Path) -> Result<bool> {
    let modified_at = fs::metadata(path)
        .with_context(|| format!("failed to stat sync lock {}", path.display()))?
        .modified()
        .with_context(|| format!("failed to read sync lock mtime {}", path.display()))?;
    let modified_at = OffsetDateTime::from(modified_at);
    Ok(OffsetDateTime::now_utc() - modified_at > STALE_LOCK_WINDOW)
}
