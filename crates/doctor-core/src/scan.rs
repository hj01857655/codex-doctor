use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};
use crate::{
    read_root_config_snapshot, read_threads, CodexLayout, RolloutRecord, RootConfigSnapshot,
    SqliteThreadRecord, ThreadLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    pub config_present: bool,
    pub sessions_present: bool,
    pub sqlite_present: bool,
    pub sqlite_readable: bool,
    pub sqlite_locked: bool,
    pub logs_present: bool,
    pub logs_readable: bool,
    pub history_present: bool,
    pub history_readable: bool,
    pub active_rollout_count: usize,
    pub archived_rollout_count: usize,
    pub locked_rollout_count: usize,
    pub root_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderDistribution {
    pub rollout: BTreeMap<String, usize>,
    pub sqlite: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub summary: ScanSummary,
    pub providers: ProviderDistribution,
    pub root_config: Option<RootConfigSnapshot>,
    pub rollout_records: Vec<RolloutRecord>,
    pub locked_rollout_paths: Vec<PathBuf>,
    pub sqlite_threads: Vec<SqliteThreadRecord>,
}

pub fn scan_codex_home(codex_home: &Path) -> Result<ScanReport, String> {
    scan_codex_home_with_sqlite_home(codex_home, None)
}

pub fn scan_codex_home_with_sqlite_home(
    codex_home: &Path,
    sqlite_home_override: Option<&Path>,
) -> Result<ScanReport, String> {
    let layout =
        CodexLayout::from_codex_home_and_env(codex_home, sqlite_home_override.map(PathBuf::from));

    let config_present = layout.config_toml.exists();
    let sessions_present = layout.sessions_dir.exists();
    let sqlite_present = layout.state_db.exists();

    let root_config = if config_present {
        Some(read_root_config_snapshot(&layout.config_toml)?)
    } else {
        None
    };
    let root_provider = root_config
        .as_ref()
        .and_then(|config| config.model_provider.clone());

    let sqlite_locked = if sqlite_present {
        probe_sqlite_lock(&layout.state_db)
    } else {
        false
    };

    let logs_present = layout.logs_db.exists();
    let logs_readable = if logs_present {
        fs::metadata(&layout.logs_db).is_ok()
    } else {
        false
    };

    let history_present = layout.history_jsonl.exists();
    let history_readable = if history_present {
        fs::read_to_string(&layout.history_jsonl).is_ok()
    } else {
        false
    };

    let active_rollouts = read_rollouts_in_dir(&layout.sessions_dir, ThreadLocation::Active)?;
    let archived_rollouts =
        read_rollouts_in_dir(&layout.archived_sessions_dir, ThreadLocation::Archived)?;
    let mut locked_rollout_paths = active_rollouts.locked_paths;
    locked_rollout_paths.extend(archived_rollouts.locked_paths);

    let mut rollout_distribution = BTreeMap::new();
    for record in active_rollouts
        .records
        .iter()
        .chain(archived_rollouts.records.iter())
    {
        if let Some(provider) = record.session_meta.provider.as_ref() {
            *rollout_distribution.entry(provider.clone()).or_insert(0) += 1;
        }
    }

    let (sqlite_readable, sqlite_distribution, sqlite_threads) = if sqlite_present {
        match read_threads(&layout.state_db) {
            Ok(rows) => {
                let mut distribution = BTreeMap::new();
                for row in &rows {
                    *distribution.entry(row.model_provider.clone()).or_insert(0) += 1;
                }
                (true, distribution, rows)
            }
            Err(_) => (false, BTreeMap::new(), Vec::new()),
        }
    } else {
        (false, BTreeMap::new(), Vec::new())
    };

    Ok(ScanReport {
        summary: ScanSummary {
            config_present,
            sessions_present,
            sqlite_present,
            sqlite_readable,
            sqlite_locked,
            logs_present,
            logs_readable,
            history_present,
            history_readable,
            active_rollout_count: active_rollouts.records.len(),
            archived_rollout_count: archived_rollouts.records.len(),
            locked_rollout_count: locked_rollout_paths.len(),
            root_provider,
        },
        providers: ProviderDistribution {
            rollout: rollout_distribution,
            sqlite: sqlite_distribution,
        },
        root_config,
        rollout_records: active_rollouts
            .records
            .into_iter()
            .chain(archived_rollouts.records)
            .collect(),
        locked_rollout_paths,
        sqlite_threads,
    })
}

#[derive(Debug, Default)]
struct RolloutDirScan {
    records: Vec<RolloutRecord>,
    locked_paths: Vec<PathBuf>,
}

fn read_rollouts_in_dir(
    dir: &Path,
    location: ThreadLocation,
) -> Result<RolloutDirScan, String> {
    if !dir.exists() {
        return Ok(RolloutDirScan::default());
    }

    let mut result = RolloutDirScan::default();
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            if probe_rollout_lock(&path)? {
                result.locked_paths.push(path);
                continue;
            }
            result
                .records
                .push(RolloutRecord::from_path(&path, location.clone())?);
        }
    }

    Ok(result)
}

fn probe_rollout_lock(path: &Path) -> Result<bool, String> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|err| err.to_string())?;

    match file.try_lock_shared() {
        Ok(()) => {
            let _ = file.unlock();
            Ok(false)
        }
        Err(err) if is_lock_try_error(&err) => Ok(true),
        Err(err) => Err(err.to_string()),
    }
}

fn probe_sqlite_lock(path: &Path) -> bool {
    let connection = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE) {
        Ok(connection) => connection,
        Err(err) => return is_lock_sqlite_error(&err),
    };

    let _ = connection.busy_timeout(Duration::from_millis(0));
    connection
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .err()
        .is_some_and(|err| is_lock_sqlite_error(&err))
}

fn is_lock_sqlite_error(err: &SqliteError) -> bool {
    match err {
        SqliteError::SqliteFailure(inner, _) => {
            matches!(inner.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
        }
        _ => {
            let text = err.to_string().to_ascii_lowercase();
            text.contains("database is locked") || text.contains("database is busy")
        }
    }
}

fn is_lock_io_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
    ) || {
        let text = err.to_string().to_ascii_lowercase();
        text.contains("used by another process")
            || text.contains("cannot access the file")
            || text.contains("resource temporarily unavailable")
            || text.contains("would block")
    }
}

fn is_lock_try_error(err: &std::fs::TryLockError) -> bool {
    match err {
        std::fs::TryLockError::WouldBlock => true,
        std::fs::TryLockError::Error(err) => is_lock_io_error(err),
    }
}
